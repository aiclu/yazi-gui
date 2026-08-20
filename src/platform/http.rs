use crate::update::{CURRENT_VERSION, DownloadProgress, DownloadResult};
use anyhow::{Context, Result, anyhow};
use std::fs::{self, File};
use std::io::Write;
use std::path::Path;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};
use tokio::sync::mpsc::UnboundedSender;
use url::Url;

#[cfg(windows)]
pub(crate) fn download_file(
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
pub(crate) fn download_file(
    _url: &str,
    _path: &Path,
    _asset_total: u64,
    _cancel: Arc<AtomicBool>,
    _progress_tx: &UnboundedSender<DownloadProgress>,
) -> Result<DownloadResult> {
    Err(anyhow!("update download is only supported on Windows"))
}

#[cfg(windows)]
pub(crate) fn fetch_bytes(url: &str) -> Result<Vec<u8>> {
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
pub(crate) fn fetch_bytes(_url: &str) -> Result<Vec<u8>> {
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
