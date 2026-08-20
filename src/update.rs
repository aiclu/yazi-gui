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
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::mpsc::UnboundedSender;

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

#[derive(Clone)]
pub(crate) enum UpdatePhase {
    Idle,
    Checking,
    UpToDate {
        version: String,
    },
    Available(ReleaseInfo),
    Downloading {
        progress: DownloadProgress,
    },
    Ready {
        release: ReleaseInfo,
        archive: PathBuf,
        progress: DownloadProgress,
    },
    Restarting,
    Failed(String),
}

pub(crate) struct UpdateState {
    pub(crate) phase: UpdatePhase,
    request_id: u64,
    cancel: Option<Arc<AtomicBool>>,
}

impl Default for UpdateState {
    fn default() -> Self {
        Self {
            phase: UpdatePhase::Idle,
            request_id: 0,
            cancel: None,
        }
    }
}

impl UpdateState {
    pub(crate) fn is_busy(&self) -> bool {
        matches!(
            self.phase,
            UpdatePhase::Checking | UpdatePhase::Downloading { .. } | UpdatePhase::Restarting
        )
    }

    pub(crate) fn begin_check(&mut self) -> u64 {
        self.request_id = self.request_id.wrapping_add(1);
        self.cancel = None;
        self.phase = UpdatePhase::Checking;
        self.request_id
    }

    pub(crate) fn begin_download(&mut self, cancel: Arc<AtomicBool>, total: u64) -> u64 {
        self.request_id = self.request_id.wrapping_add(1);
        self.cancel = Some(cancel);
        self.phase = UpdatePhase::Downloading {
            progress: DownloadProgress {
                downloaded: 0,
                total,
                bytes_per_second: 0,
            },
        };
        self.request_id
    }

    pub(crate) fn accepts(&self, request_id: u64) -> bool {
        self.request_id == request_id
    }

    pub(crate) fn update_download_progress(
        &mut self,
        request_id: u64,
        progress: DownloadProgress,
    ) -> bool {
        if !self.accepts(request_id) {
            return false;
        }
        if let UpdatePhase::Downloading { progress: current } = &mut self.phase {
            *current = progress;
            true
        } else {
            false
        }
    }

    pub(crate) fn finish_check(
        &mut self,
        request_id: u64,
        result: Result<ReleaseInfo>,
    ) -> Option<UpdatePhase> {
        if !self.accepts(request_id) {
            return None;
        }
        let phase = match result {
            Ok(release) if is_newer_than_current(&release) => UpdatePhase::Available(release),
            Ok(release) => UpdatePhase::UpToDate {
                version: release.tag_name,
            },
            Err(error) => UpdatePhase::Failed(error.to_string()),
        };
        self.phase = phase.clone();
        Some(phase)
    }

    pub(crate) fn finish_download(
        &mut self,
        request_id: u64,
        release: ReleaseInfo,
        result: Result<DownloadResult>,
    ) -> Option<UpdatePhase> {
        if !self.accepts(request_id) {
            return None;
        }
        let progress = match &self.phase {
            UpdatePhase::Downloading { progress } => *progress,
            _ => DownloadProgress::default(),
        };
        let phase = match result {
            Ok(DownloadResult::Completed(archive)) => UpdatePhase::Ready {
                release,
                archive,
                progress,
            },
            Ok(DownloadResult::Cancelled) => UpdatePhase::Available(release),
            Err(error) => UpdatePhase::Failed(error.to_string()),
        };
        self.cancel = None;
        self.phase = phase.clone();
        Some(phase)
    }

    pub(crate) fn cancel_download(&self) {
        if let Some(cancel) = &self.cancel {
            cancel.store(true, Ordering::Relaxed);
        }
    }

    pub(crate) fn begin_restart(&mut self) -> bool {
        if !matches!(self.phase, UpdatePhase::Ready { .. }) {
            return false;
        }
        self.phase = UpdatePhase::Restarting;
        true
    }
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
    let body = crate::platform::http::fetch_bytes(LATEST_RELEASE_URL)
        .context("request latest GitHub release")?;
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
    let checksum_body = crate::platform::http::fetch_bytes(&release.checksum_url)
        .context("download release checksum")?;
    let expected_hash = checksum_for_asset(&checksum_body, &release.asset_name)?;
    let destination = update_temp_path(&release.asset_name);
    let partial = destination.with_extension("zip.part");
    let _ = fs::remove_file(&partial);

    let result = crate::platform::http::download_file(
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
fn apply_update(
    archive: &Path,
    executable: &Path,
    parent_pid: u32,
    updater_path: &Path,
) -> Result<()> {
    use std::process::Command;
    crate::platform::window::wait_for_process_exit(parent_pid);

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
        DownloadResult, ReleaseInfo, UpdatePhase, UpdateState, checksum_for_asset,
        expected_asset_name, is_newer_than_current, update_temp_path,
    };
    use semver::Version;
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };

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
        let current = Version::parse(env!("CARGO_PKG_VERSION")).unwrap();
        let newer = Version::new(current.major, current.minor, current.patch + 1);
        let release = ReleaseInfo {
            tag_name: format!("v{newer}"),
            version: newer,
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

    #[test]
    fn stale_update_requests_cannot_replace_current_phase() {
        let mut state = UpdateState::default();
        let first = state.begin_check();
        let second = state.begin_check();
        assert!(
            state
                .finish_check(first, Err(anyhow::anyhow!("stale")))
                .is_none()
        );
        assert!(matches!(
            state.finish_check(second, Ok(release())),
            Some(UpdatePhase::Available(_))
        ));
    }

    #[test]
    fn download_lifecycle_owns_cancellation_and_completion() {
        let mut state = UpdateState::default();
        state.phase = UpdatePhase::Available(release());
        let cancel = Arc::new(AtomicBool::new(false));
        let request = state.begin_download(cancel.clone(), 10);
        state.update_download_progress(
            request,
            super::DownloadProgress {
                downloaded: 5,
                total: 10,
                bytes_per_second: 1,
            },
        );
        state.cancel_download();
        assert!(cancel.load(Ordering::Relaxed));
        assert!(matches!(
            state.finish_download(
                request,
                release(),
                Ok(DownloadResult::Completed(
                    std::env::temp_dir().join("update.zip")
                )),
            ),
            Some(UpdatePhase::Ready { .. })
        ));
    }

    fn release() -> ReleaseInfo {
        let current = Version::parse(env!("CARGO_PKG_VERSION")).unwrap();
        let version = Version::new(current.major, current.minor, current.patch + 1);
        ReleaseInfo {
            tag_name: format!("v{version}"),
            version,
            page_url: String::new(),
            asset_name: "update.zip".to_string(),
            asset_url: String::new(),
            asset_size: 10,
            checksum_url: String::new(),
        }
    }
}
