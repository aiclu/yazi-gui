use anyhow::{Context, Result, anyhow};
use semver::Version;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::mpsc::UnboundedSender;
use url::Url;

pub const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");
const LATEST_RELEASE_URL: &str = "https://api.github.com/repos/aiclu/yazi-gui/releases/latest";
const CHECKSUM_ASSET: &str = "SHA256SUMS.txt";
const BUNDLED_YAZI_VERSION: &str = "26.8.15";

#[derive(Clone, Debug)]
pub struct ReleaseInfo {
    pub tag_name: String,
    pub version: Version,
    pub page_url: String,
    pub asset_name: String,
    pub asset_url: String,
    pub asset_size: u64,
    checksum_url: String,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct DownloadProgress {
    pub downloaded: u64,
    pub total: u64,
    pub bytes_per_second: u64,
}

pub enum DownloadResult {
    Completed(PathBuf),
    Cancelled,
}

#[derive(Deserialize)]
struct GithubRelease {
    tag_name: String,
    html_url: String,
    draft: bool,
    prerelease: bool,
    assets: Vec<GithubAsset>,
}

#[derive(Deserialize)]
struct GithubAsset {
    name: String,
    browser_download_url: String,
    size: u64,
}

pub fn check_latest_release() -> Result<ReleaseInfo> {
    let body = fetch_bytes(LATEST_RELEASE_URL).context("request latest GitHub release")?;
    let release: GithubRelease =
        serde_json::from_slice(&body).context("parse latest GitHub release")?;
    if release.draft || release.prerelease {
        return Err(anyhow!("latest GitHub release is not a stable release"));
    }

    let version_text = release
        .tag_name
        .strip_prefix('v')
        .ok_or_else(|| anyhow!("release tag has no v prefix"))?;
    let version = Version::parse(version_text)
        .with_context(|| format!("parse release version {}", release.tag_name))?;
    let asset_name = expected_asset_name(&release.tag_name);
    let asset = release
        .assets
        .iter()
        .find(|asset| asset.name == asset_name)
        .ok_or_else(|| anyhow!("release asset not found: {asset_name}"))?;
    let checksum = release
        .assets
        .iter()
        .find(|asset| asset.name == CHECKSUM_ASSET)
        .ok_or_else(|| anyhow!("release checksum asset not found"))?;

    Ok(ReleaseInfo {
        tag_name: release.tag_name,
        version,
        page_url: release.html_url,
        asset_name,
        asset_url: asset.browser_download_url.clone(),
        asset_size: asset.size,
        checksum_url: checksum.browser_download_url.clone(),
    })
}

pub fn is_newer_than_current(release: &ReleaseInfo) -> bool {
    Version::parse(CURRENT_VERSION)
        .map(|current| release.version > current)
        .unwrap_or(false)
}

pub fn download_release(
    release: &ReleaseInfo,
    cancel: Arc<AtomicBool>,
    progress_tx: &UnboundedSender<DownloadProgress>,
) -> Result<DownloadResult> {
    let checksum_body = fetch_bytes(&release.checksum_url).context("download release checksum")?;
    let expected_hash = checksum_for_asset(&checksum_body, &release.asset_name)?;
    let destination = update_temp_path(&release.asset_name);
    let partial = destination.with_extension("zip.part");
    let _ = fs::remove_file(&partial);

    let result = download_file(
        &release.asset_url,
        &partial,
        release.asset_size,
        cancel.clone(),
        progress_tx,
    );
    match result {
        Ok(DownloadResult::Cancelled) => {
            let _ = fs::remove_file(&partial);
            return Ok(DownloadResult::Cancelled);
        }
        Ok(DownloadResult::Completed(_)) => {}
        Err(error) => {
            let _ = fs::remove_file(&partial);
            return Err(error);
        }
    }

    if cancel.load(Ordering::Relaxed) {
        let _ = fs::remove_file(&partial);
        return Ok(DownloadResult::Cancelled);
    }
    let actual_hash = sha256_file(&partial)?;
    if actual_hash != expected_hash {
        let _ = fs::remove_file(&partial);
        return Err(anyhow!(
            "release SHA-256 mismatch: expected {expected_hash}, got {actual_hash}"
        ));
    }
    fs::rename(&partial, &destination)
        .with_context(|| format!("commit downloaded update {}", destination.to_string_lossy()))?;
    Ok(DownloadResult::Completed(destination))
}

pub fn update_temp_path(asset_name: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    std::env::temp_dir().join(format!(
        "yazi-gui-update-{}-{}-{}",
        std::process::id(),
        stamp,
        asset_name
    ))
}

pub fn spawn_restart_update(archive: &Path) -> Result<()> {
    #[cfg(windows)]
    {
        use std::process::Command;

        let executable = std::env::current_exe().context("locate current executable")?;
        let updater = std::env::temp_dir().join(format!(
            "yazi-gui-updater-{}-{}.exe",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|duration| duration.as_nanos())
                .unwrap_or_default()
        ));
        fs::copy(&executable, &updater)
            .with_context(|| format!("prepare updater copy {}", updater.to_string_lossy()))?;
        let args = [
            "--apply-update".to_string(),
            archive.to_string_lossy().into_owned(),
            executable.to_string_lossy().into_owned(),
            std::process::id().to_string(),
            updater.to_string_lossy().into_owned(),
        ];
        Command::new(&updater)
            .current_dir(std::env::temp_dir())
            .args(args)
            .spawn()
            .context("start updater process")?;
        Ok(())
    }
    #[cfg(not(windows))]
    {
        let _ = archive;
        Err(anyhow!("restart update is only supported on Windows"))
    }
}

pub fn run_update_mode(args: &[String]) -> Result<bool> {
    if args.first().map(String::as_str) == Some("--apply-update") {
        if args.len() != 5 {
            return Err(anyhow!(
                "--apply-update requires archive, executable, parent pid, and updater path"
            ));
        }
        let parent_pid = args[3]
            .parse::<u32>()
            .context("parse updater parent process id")?;
        apply_update(
            Path::new(&args[1]),
            Path::new(&args[2]),
            parent_pid,
            Path::new(&args[4]),
        )?;
        return Ok(true);
    }
    if args.first().map(String::as_str) == Some("--cleanup-update") {
        if args.len() != 3 {
            return Err(anyhow!(
                "--cleanup-update requires backup and updater paths"
            ));
        }
        let _ = fs::remove_dir_all(&args[1]);
        let _ = fs::remove_file(&args[2]);
    }
    Ok(false)
}

fn expected_asset_name(tag_name: &str) -> String {
    if cfg!(feature = "bundled-yazi") {
        format!("yazi-gui-{tag_name}-windows-x64-bundled-yazi-{BUNDLED_YAZI_VERSION}.zip")
    } else {
        format!("yazi-gui-{tag_name}-windows-x64.zip")
    }
}

fn checksum_for_asset(body: &[u8], asset_name: &str) -> Result<String> {
    for line in String::from_utf8_lossy(body).lines() {
        let mut parts = line.split_whitespace();
        let Some(hash) = parts.next() else {
            continue;
        };
        let Some(name) = parts.next() else {
            continue;
        };
        if name.trim_start_matches('*') == asset_name {
            if hash.len() != 64 || !hash.chars().all(|character| character.is_ascii_hexdigit()) {
                return Err(anyhow!("invalid SHA-256 for release asset"));
            }
            return Ok(hash.to_ascii_lowercase());
        }
    }
    Err(anyhow!("release checksum not found for {asset_name}"))
}

fn sha256_file(path: &Path) -> Result<String> {
    let mut file = File::open(path).with_context(|| format!("open {}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 1024 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

#[cfg(windows)]
fn download_file(
    url: &str,
    path: &Path,
    asset_total: u64,
    cancel: Arc<AtomicBool>,
    progress_tx: &UnboundedSender<DownloadProgress>,
) -> Result<DownloadResult> {
    let response = HttpResponse::open(url)?;
    let total = response.content_length.unwrap_or(asset_total);
    let mut file = File::create(path).with_context(|| format!("create {}", path.display()))?;
    let mut downloaded = 0u64;
    let started = Instant::now();
    let mut last_report = Instant::now();
    let mut buffer = vec![0u8; 1024 * 1024];

    loop {
        if cancel.load(Ordering::Relaxed) {
            let _ = fs::remove_file(path);
            return Ok(DownloadResult::Cancelled);
        }
        let mut available = 0u32;
        if unsafe {
            windows_sys::Win32::Networking::WinHttp::WinHttpQueryDataAvailable(
                response.request.0,
                &mut available,
            )
        } == 0
        {
            return Err(win_error("WinHttpQueryDataAvailable"));
        }
        if available == 0 {
            break;
        }
        let requested = available.min(buffer.len() as u32);
        let mut read = 0u32;
        if unsafe {
            windows_sys::Win32::Networking::WinHttp::WinHttpReadData(
                response.request.0,
                buffer.as_mut_ptr().cast(),
                requested,
                &mut read,
            )
        } == 0
        {
            return Err(win_error("WinHttpReadData"));
        }
        if read == 0 {
            break;
        }
        file.write_all(&buffer[..read as usize])?;
        downloaded = downloaded.saturating_add(read as u64);
        if last_report.elapsed() >= Duration::from_millis(100) || downloaded == total {
            let elapsed = started.elapsed().as_secs_f64().max(0.001);
            let _ = progress_tx.send(DownloadProgress {
                downloaded,
                total,
                bytes_per_second: (downloaded as f64 / elapsed) as u64,
            });
            last_report = Instant::now();
        }
    }
    file.flush()?;
    let elapsed = started.elapsed().as_secs_f64().max(0.001);
    let _ = progress_tx.send(DownloadProgress {
        downloaded,
        total,
        bytes_per_second: (downloaded as f64 / elapsed) as u64,
    });
    Ok(DownloadResult::Completed(path.to_path_buf()))
}

#[cfg(not(windows))]
fn download_file(
    _url: &str,
    _path: &Path,
    _asset_total: u64,
    _cancel: Arc<AtomicBool>,
    _progress_tx: &UnboundedSender<DownloadProgress>,
) -> Result<DownloadResult> {
    Err(anyhow!("update download is only supported on Windows"))
}

#[cfg(windows)]
fn fetch_bytes(url: &str) -> Result<Vec<u8>> {
    let response = HttpResponse::open(url)?;
    let mut body = Vec::new();
    let mut buffer = vec![0u8; 64 * 1024];
    loop {
        let mut available = 0u32;
        if unsafe {
            windows_sys::Win32::Networking::WinHttp::WinHttpQueryDataAvailable(
                response.request.0,
                &mut available,
            )
        } == 0
        {
            return Err(win_error("WinHttpQueryDataAvailable"));
        }
        if available == 0 {
            break;
        }
        let requested = available.min(buffer.len() as u32);
        let mut read = 0u32;
        if unsafe {
            windows_sys::Win32::Networking::WinHttp::WinHttpReadData(
                response.request.0,
                buffer.as_mut_ptr().cast(),
                requested,
                &mut read,
            )
        } == 0
        {
            return Err(win_error("WinHttpReadData"));
        }
        if read == 0 {
            break;
        }
        body.extend_from_slice(&buffer[..read as usize]);
        if body.len() > 16 * 1024 * 1024 {
            return Err(anyhow!("update response is too large"));
        }
    }
    Ok(body)
}

#[cfg(not(windows))]
fn fetch_bytes(_url: &str) -> Result<Vec<u8>> {
    Err(anyhow!("update check is only supported on Windows"))
}

#[cfg(windows)]
struct HttpResponse {
    _session: HttpHandle,
    _connection: HttpHandle,
    request: HttpHandle,
    content_length: Option<u64>,
}

#[cfg(windows)]
struct HttpHandle(*mut std::ffi::c_void);

#[cfg(windows)]
impl Drop for HttpHandle {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe {
                let _ = windows_sys::Win32::Networking::WinHttp::WinHttpCloseHandle(self.0);
            }
        }
    }
}

#[cfg(windows)]
impl HttpResponse {
    fn open(url: &str) -> Result<Self> {
        use std::ptr::{null, null_mut};
        use windows_sys::Win32::Networking::WinHttp::{
            WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY, WINHTTP_FLAG_SECURE, WINHTTP_QUERY_CONTENT_LENGTH,
            WINHTTP_QUERY_FLAG_NUMBER, WINHTTP_QUERY_STATUS_CODE, WinHttpConnect, WinHttpOpen,
            WinHttpOpenRequest, WinHttpQueryHeaders, WinHttpReceiveResponse, WinHttpSendRequest,
            WinHttpSetTimeouts,
        };

        let parsed = Url::parse(url).with_context(|| format!("parse update URL {url}"))?;
        if parsed.scheme() != "https" || !parsed.username().is_empty() {
            return Err(anyhow!("update URL must use HTTPS without credentials"));
        }
        let host = parsed
            .host_str()
            .ok_or_else(|| anyhow!("update URL has no host"))?;
        let port = parsed.port_or_known_default().unwrap_or(443);
        let mut object = parsed.path().to_string();
        if object.is_empty() {
            object.push('/');
        }
        if let Some(query) = parsed.query() {
            object.push('?');
            object.push_str(query);
        }

        let agent = wide(&format!("yazi-gui/{CURRENT_VERSION}"));
        let host = wide(host);
        let object = wide(&object);
        let headers = wide(
            "User-Agent: yazi-gui\r\nAccept: application/vnd.github+json\r\nX-GitHub-Api-Version: 2022-11-28\r\n",
        );
        let session = unsafe {
            WinHttpOpen(
                agent.as_ptr(),
                WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY,
                null(),
                null(),
                0,
            )
        };
        let session = non_null_handle(session, "WinHttpOpen")?;
        unsafe {
            if WinHttpSetTimeouts(session.0, 10_000, 10_000, 10_000, 30_000) == 0 {
                return Err(win_error("WinHttpSetTimeouts"));
            }
        }
        let connection = unsafe { WinHttpConnect(session.0, host.as_ptr(), port, 0) };
        let connection = non_null_handle(connection, "WinHttpConnect")?;
        let request = unsafe {
            WinHttpOpenRequest(
                connection.0,
                wide("GET").as_ptr(),
                object.as_ptr(),
                null(),
                null(),
                null(),
                WINHTTP_FLAG_SECURE,
            )
        };
        let request = non_null_handle(request, "WinHttpOpenRequest")?;
        unsafe {
            if WinHttpSendRequest(request.0, headers.as_ptr(), u32::MAX, null(), 0, 0, 0) == 0 {
                return Err(win_error("WinHttpSendRequest"));
            }
            if WinHttpReceiveResponse(request.0, null_mut()) == 0 {
                return Err(win_error("WinHttpReceiveResponse"));
            }
        }

        let mut status = 0u32;
        let mut status_len = std::mem::size_of::<u32>() as u32;
        unsafe {
            if WinHttpQueryHeaders(
                request.0,
                WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
                null(),
                (&mut status as *mut u32).cast(),
                &mut status_len,
                null_mut(),
            ) == 0
            {
                return Err(win_error("WinHttpQueryHeaders status"));
            }
        }
        if !(200..300).contains(&status) {
            return Err(anyhow!("update server returned HTTP {status}"));
        }

        let mut length_buffer = [0u16; 64];
        let mut length_bytes = (length_buffer.len() * std::mem::size_of::<u16>()) as u32;
        let content_length = unsafe {
            if WinHttpQueryHeaders(
                request.0,
                WINHTTP_QUERY_CONTENT_LENGTH,
                null(),
                length_buffer.as_mut_ptr().cast(),
                &mut length_bytes,
                null_mut(),
            ) != 0
            {
                String::from_utf16_lossy(&length_buffer[..(length_bytes as usize / 2)])
                    .trim_matches('\0')
                    .trim()
                    .parse()
                    .ok()
            } else {
                None
            }
        };

        Ok(Self {
            _session: session,
            _connection: connection,
            request,
            content_length,
        })
    }
}

#[cfg(windows)]
fn non_null_handle(handle: *mut std::ffi::c_void, operation: &str) -> Result<HttpHandle> {
    if handle.is_null() {
        Err(win_error(operation))
    } else {
        Ok(HttpHandle(handle))
    }
}

#[cfg(windows)]
fn win_error(operation: &str) -> anyhow::Error {
    let code = unsafe { windows_sys::Win32::Foundation::GetLastError() };
    anyhow!("{operation} failed with Windows error {code}")
}

#[cfg(windows)]
fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

#[cfg(windows)]
fn apply_update(
    archive: &Path,
    executable: &Path,
    parent_pid: u32,
    updater_path: &Path,
) -> Result<()> {
    use std::process::Command;
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{
        INFINITE, OpenProcess, PROCESS_SYNCHRONIZE, WaitForSingleObject,
    };

    unsafe {
        let parent = OpenProcess(PROCESS_SYNCHRONIZE, 0, parent_pid);
        if !parent.is_null() {
            let _ = WaitForSingleObject(parent, INFINITE);
            let _ = CloseHandle(parent);
        }
    }

    let install_root = executable
        .parent()
        .ok_or_else(|| anyhow!("updated executable has no parent directory"))?;
    let executable_name = executable
        .file_name()
        .ok_or_else(|| anyhow!("updated executable has no file name"))?;
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    let staging = install_root.with_file_name(format!(
        ".{}-update-staging-{stamp}",
        install_root
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("yazi-gui")
    ));
    let backup = install_root.with_file_name(format!(
        ".{}-update-old-{stamp}",
        install_root
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("yazi-gui")
    ));
    if let Err(error) = extract_archive(archive, &staging) {
        let _ = fs::remove_dir_all(&staging);
        return Err(error).context("extract downloaded update");
    }
    if !staging.join(executable_name).is_file() || !staging.join(r"assets\yazi").is_dir() {
        let _ = fs::remove_dir_all(&staging);
        return Err(anyhow!("downloaded update has an invalid package layout"));
    }

    fs::rename(install_root, &backup)
        .with_context(|| format!("move current installation to {}", backup.display()))?;
    if let Err(error) = fs::rename(&staging, install_root) {
        let _ = fs::rename(&backup, install_root);
        return Err(error).context("activate downloaded update");
    }

    let new_executable = install_root.join(executable_name);
    if let Err(error) = Command::new(&new_executable)
        .current_dir(install_root)
        .args([
            "--cleanup-update",
            backup.to_string_lossy().as_ref(),
            updater_path.to_string_lossy().as_ref(),
        ])
        .spawn()
    {
        let _ = fs::remove_dir_all(install_root);
        let _ = fs::rename(&backup, install_root);
        return Err(error).context("restart updated application");
    }
    Ok(())
}

#[cfg(not(windows))]
fn apply_update(
    _archive: &Path,
    _executable: &Path,
    _parent_pid: u32,
    _updater_path: &Path,
) -> Result<()> {
    Err(anyhow!("restart update is only supported on Windows"))
}

#[cfg(windows)]
fn extract_archive(archive: &Path, staging: &Path) -> Result<()> {
    use zip::ZipArchive;

    let file = File::open(archive).with_context(|| format!("open {}", archive.display()))?;
    let mut archive = ZipArchive::new(file).context("read update archive")?;
    fs::create_dir_all(staging).with_context(|| format!("create {}", staging.display()))?;
    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .context("read update archive entry")?;
        let relative = entry
            .enclosed_name()
            .ok_or_else(|| anyhow!("update archive contains an unsafe path"))?
            .to_path_buf();
        let destination = staging.join(relative);
        if entry.is_dir() {
            fs::create_dir_all(&destination)?;
            continue;
        }
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut output = File::create(&destination)?;
        io::copy(&mut entry, &mut output)?;
        output.flush()?;
    }
    Ok(())
}

#[cfg(not(windows))]
fn extract_archive(_archive: &Path, _staging: &Path) -> Result<()> {
    Err(anyhow!("update extraction is only supported on Windows"))
}

#[cfg(test)]
mod tests {
    use super::{
        ReleaseInfo, checksum_for_asset, expected_asset_name, is_newer_than_current,
        update_temp_path,
    };
    use semver::Version;

    #[test]
    fn checksum_manifest_matches_named_asset() {
        let body = b"0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef  yazi-gui-v0.2.0-windows-x64.zip\n";
        assert_eq!(
            checksum_for_asset(body, "yazi-gui-v0.2.0-windows-x64.zip").unwrap(),
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
        );
    }

    #[test]
    fn checksum_manifest_rejects_missing_or_invalid_entries() {
        assert!(checksum_for_asset(b"bad  other.zip\n", "target.zip").is_err());
        assert!(checksum_for_asset(b"not-a-hash  target.zip\n", "target.zip").is_err());
    }

    #[test]
    fn expected_asset_matches_build_mode() {
        let asset = expected_asset_name("v0.2.0");
        if cfg!(feature = "bundled-yazi") {
            assert_eq!(
                asset,
                "yazi-gui-v0.2.0-windows-x64-bundled-yazi-26.8.15.zip"
            );
        } else {
            assert_eq!(asset, "yazi-gui-v0.2.0-windows-x64.zip");
        }
    }

    #[test]
    fn newer_release_is_required_before_download() {
        let release = ReleaseInfo {
            tag_name: "v0.1.1".to_string(),
            version: Version::new(0, 1, 1),
            page_url: String::new(),
            asset_name: String::new(),
            asset_url: String::new(),
            asset_size: 0,
            checksum_url: String::new(),
        };
        assert!(is_newer_than_current(&release));
    }

    #[test]
    fn temporary_archive_path_contains_unique_process_context() {
        let path = update_temp_path("package.zip");
        assert!(
            path.file_name()
                .unwrap()
                .to_string_lossy()
                .contains("package.zip")
        );
        assert_eq!(path.parent(), Some(std::env::temp_dir().as_path()));
    }
}
