use crate::yazi::FileEntry;
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering as AtomicOrdering};
use time::{OffsetDateTime, UtcOffset};
use tokio::sync::mpsc::UnboundedSender;

#[cfg(windows)]
use std::os::windows::ffi::OsStrExt;
#[cfg(windows)]
use windows_sys::Win32::Storage::FileSystem::GetDriveTypeW;
#[cfg(windows)]
use windows_sys::Win32::System::WindowsProgramming::DRIVE_REMOTE;

#[derive(Clone)]
pub struct FolderEntry {
    pub path: String,
    pub name: String,
    pub is_link: bool,
}

#[derive(Clone, Copy)]
pub enum DeleteMode {
    RecycleBin,
    Permanent,
}

pub struct DeleteSummary {
    pub success: usize,
    pub failed: usize,
    pub permanent_success: usize,
    pub first_failure: Option<String>,
}

pub enum TransferOutcome {
    Completed { success: usize },
    Cancelled { success: usize },
    Failed { success: usize, message: String },
}

pub enum TransferUpdate {
    Progress {
        total_bytes: u64,
        completed_bytes: u64,
        completed_items: usize,
        current: String,
    },
    Finished(TransferOutcome),
}

pub static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

pub fn file_name_of(url: &str) -> String {
    Path::new(url)
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| url.to_string())
}

pub fn move_paths(paths: &[String], destination: &str) -> (usize, usize) {
    let mut success = 0usize;
    let mut failed = 0usize;
    for source in paths {
        let target = Path::new(destination).join(file_name_of(source));
        if fs::rename(source, target).is_ok() {
            success += 1;
        } else {
            failed += 1;
        }
    }
    (success, failed)
}

pub fn rename_path(path: &Path, fallback_parent: &Path, name: &str) -> io::Result<()> {
    let target = path.parent().unwrap_or(fallback_parent).join(name);
    fs::rename(path, target)
}

pub fn create_file(parent: &Path, name: &str) -> io::Result<()> {
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(parent.join(name))
        .map(|_| ())
}

pub fn create_directory(parent: &Path, name: &str) -> io::Result<()> {
    fs::create_dir(parent.join(name))
}

pub fn scan_folder_children(root: &Path) -> io::Result<Vec<FolderEntry>> {
    let mut children = Vec::new();
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)?;
        let is_link = is_link_metadata(&metadata);
        let is_dir = if is_link {
            fs::metadata(&path)
                .map(|target| target.is_dir())
                .unwrap_or(false)
        } else {
            metadata.is_dir()
        };
        if !is_dir {
            continue;
        }
        children.push(FolderEntry {
            path: path.to_string_lossy().into_owned(),
            name: entry.file_name().to_string_lossy().into_owned(),
            is_link,
        });
    }
    children.sort_by(|left, right| {
        left.name
            .to_lowercase()
            .cmp(&right.name.to_lowercase())
            .then_with(|| left.name.cmp(&right.name))
    });
    Ok(children)
}

fn is_link_metadata(metadata: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
        metadata.file_type().is_symlink()
            || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
    }
    #[cfg(not(windows))]
    {
        metadata.file_type().is_symlink()
    }
}

pub fn search_directory(root: &Path, query: &str) -> io::Result<Vec<FileEntry>> {
    let query = query.to_lowercase();
    let mut results = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(&directory)? {
            let entry = entry?;
            let path = entry.path();
            let metadata = fs::symlink_metadata(&path)?;
            let is_symlink = is_link_metadata(&metadata);
            let is_dir = if is_symlink {
                fs::metadata(&path)
                    .map(|target| target.is_dir())
                    .unwrap_or(false)
            } else {
                metadata.is_dir()
            };
            let relative = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .to_string_lossy()
                .into_owned();
            if query.is_empty()
                || relative.to_lowercase().contains(&query)
                || entry
                    .file_name()
                    .to_string_lossy()
                    .to_lowercase()
                    .contains(&query)
            {
                let mtime = metadata
                    .modified()
                    .ok()
                    .and_then(|value| value.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|value| value.as_secs_f64())
                    .unwrap_or(0.0);
                results.push(FileEntry {
                    name: relative,
                    is_dir,
                    is_hidden: false,
                    size: if is_dir { 0 } else { metadata.len() },
                    mtime,
                });
            }
            if is_dir && !is_symlink {
                pending.push(path);
            }
        }
    }
    Ok(results)
}

pub fn format_mtime(mtime: f64) -> String {
    if !mtime.is_finite() || mtime <= 0.0 {
        return String::new();
    }
    let utc = match OffsetDateTime::from_unix_timestamp(mtime as i64) {
        Ok(value) => value,
        Err(_) => return String::new(),
    };
    let offset = match UtcOffset::current_local_offset() {
        Ok(value) => value,
        Err(_) => return String::new(),
    };
    utc.to_offset(offset)
        .format(&time::macros::format_description!(
            "[year]-[month]-[day] [hour]:[minute]"
        ))
        .unwrap_or_default()
}

/// 判断路径是否为盘符根（如 `D:\`、`C:`）。
pub fn is_drive_root(cwd: &str) -> bool {
    let bytes = cwd.as_bytes();
    if bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' {
        let rest = &cwd[2..];
        rest.is_empty() || rest == "\\" || rest == "/"
    } else {
        false
    }
}

/// 扫描当前可见的磁盘盘符（A-Z）。
pub fn scan_drives() -> Vec<String> {
    (b'A'..=b'Z')
        .map(|c| format!("{}:\\", c as char))
        .filter(|drive| Path::new(drive).exists())
        .collect()
}

pub fn is_unc_path(path: &str) -> bool {
    let extended_prefix = "\\\\?\\";
    if let Some(rest) = path.strip_prefix(extended_prefix) {
        return rest
            .get(..4)
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case("UNC\\"));
    }
    path.starts_with("\\\\")
}

pub fn is_network_path(path: &Path) -> io::Result<bool> {
    let text = path.to_string_lossy();
    if is_unc_path(&text) {
        return Ok(true);
    }

    #[cfg(windows)]
    {
        let drive_path = text.strip_prefix("\\\\?\\").unwrap_or(&text);
        let mut chars = drive_path.chars();
        let Some(drive) = chars.next() else {
            return Ok(false);
        };
        if chars.next() != Some(':') || !drive.is_ascii_alphabetic() {
            return Ok(false);
        }

        let root = format!("{}:\\", drive);
        let wide: Vec<u16> = std::ffi::OsStr::new(&root)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        let drive_type = unsafe { GetDriveTypeW(wide.as_ptr()) };
        return match drive_type {
            DRIVE_REMOTE => Ok(true),
            0 | 1 => Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("无法识别磁盘 {}", drive),
            )),
            _ => Ok(false),
        };
    }

    #[cfg(not(windows))]
    {
        let _ = path;
        Ok(false)
    }
}

pub fn network_path_count(paths: &[String]) -> Result<usize, String> {
    let mut count = 0usize;
    for path in paths {
        match is_network_path(Path::new(path)) {
            Ok(true) => count += 1,
            Ok(false) => {}
            Err(error) => {
                return Err(format!(
                    "无法判断 {} 的存储类型: {}",
                    file_name_of(path),
                    error
                ));
            }
        }
    }
    Ok(count)
}

pub fn permanent_delete(path: &Path) -> io::Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_dir() {
        fs::remove_dir_all(path)
    } else {
        fs::remove_file(path)
    }
}

pub fn delete_path_with_policy(path: &Path) -> Result<DeleteMode, String> {
    if is_network_path(path).map_err(|error| error.to_string())? {
        permanent_delete(path).map_err(|error| error.to_string())?;
        Ok(DeleteMode::Permanent)
    } else {
        trash::delete(path).map_err(|error| error.to_string())?;
        Ok(DeleteMode::RecycleBin)
    }
}

pub fn delete_paths_with_policy(paths: &[String]) -> DeleteSummary {
    let mut summary = DeleteSummary {
        success: 0,
        failed: 0,
        permanent_success: 0,
        first_failure: None,
    };
    for path in paths {
        match delete_path_with_policy(Path::new(path)) {
            Ok(DeleteMode::Permanent) => {
                summary.success += 1;
                summary.permanent_success += 1;
            }
            Ok(DeleteMode::RecycleBin) => summary.success += 1,
            Err(error) => {
                summary.failed += 1;
                if summary.first_failure.is_none() {
                    summary.first_failure = Some(format!("{}: {}", file_name_of(path), error));
                }
            }
        }
    }
    summary
}

pub fn delete_status(summary: &DeleteSummary) -> String {
    let mut status = operation_status("删除", summary.success, summary.failed);
    if summary.permanent_success > 0 {
        status.push_str(&format!("（永久删除 {} 项）", summary.permanent_success));
    }
    if let Some(error) = &summary.first_failure {
        status.push_str(&format!("，首个失败：{}", error));
    }
    status
}

fn operation_status(label: &str, success: usize, failed: usize) -> String {
    match (success, failed) {
        (0, 0) => format!("{}未执行", label),
        (_, 0) => format!("{}完成 {} 项", label, success),
        (0, _) => format!("{}失败 {} 项", label, failed),
        _ => format!("{}完成 {} 项，失败 {} 项", label, success, failed),
    }
}

enum CopyError {
    Cancelled,
    Io(String),
}

impl From<io::Error> for CopyError {
    fn from(error: io::Error) -> Self {
        Self::Io(error.to_string())
    }
}

pub fn copy_paths_with_progress(
    paths: &[String],
    dest: &str,
    cancel: &AtomicBool,
    tx: &UnboundedSender<TransferUpdate>,
) -> TransferOutcome {
    let total_bytes = match paths.iter().try_fold(0u64, |total, path| {
        copy_entry_size(Path::new(path), cancel).map(|size| total.saturating_add(size))
    }) {
        Ok(total) => total,
        Err(CopyError::Cancelled) => return TransferOutcome::Cancelled { success: 0 },
        Err(CopyError::Io(message)) => {
            return TransferOutcome::Failed {
                success: 0,
                message,
            };
        }
    };

    let _ = tx.send(TransferUpdate::Progress {
        total_bytes,
        completed_bytes: 0,
        completed_items: 0,
        current: "准备复制".to_string(),
    });

    let mut completed_bytes = 0u64;
    let mut completed_items = 0usize;
    for source in paths {
        if cancel.load(AtomicOrdering::Relaxed) {
            return TransferOutcome::Cancelled {
                success: completed_items,
            };
        }

        let source_path = Path::new(source);
        let name = file_name_of(source);
        let destination = Path::new(dest).join(&name);
        let result = copy_entry_with_progress(
            source_path,
            &destination,
            &name,
            cancel,
            tx,
            total_bytes,
            completed_items,
            &mut completed_bytes,
        );
        match result {
            Ok(()) => {
                completed_items += 1;
                let _ = tx.send(TransferUpdate::Progress {
                    total_bytes,
                    completed_bytes,
                    completed_items,
                    current: name,
                });
            }
            Err(CopyError::Cancelled) => {
                return TransferOutcome::Cancelled {
                    success: completed_items,
                };
            }
            Err(CopyError::Io(message)) => {
                return TransferOutcome::Failed {
                    success: completed_items,
                    message,
                };
            }
        }
    }

    TransferOutcome::Completed {
        success: completed_items,
    }
}

fn copy_entry_size(path: &Path, cancel: &AtomicBool) -> Result<u64, CopyError> {
    if cancel.load(AtomicOrdering::Relaxed) {
        return Err(CopyError::Cancelled);
    }
    let metadata = fs::metadata(path)?;
    if !metadata.is_dir() {
        return Ok(metadata.len());
    }
    let mut total = 0u64;
    for entry in fs::read_dir(path)? {
        total = total.saturating_add(copy_entry_size(&entry?.path(), cancel)?);
    }
    Ok(total)
}

fn copy_entry_with_progress(
    source: &Path,
    destination: &Path,
    current: &str,
    cancel: &AtomicBool,
    tx: &UnboundedSender<TransferUpdate>,
    total_bytes: u64,
    completed_items: usize,
    completed_bytes: &mut u64,
) -> Result<(), CopyError> {
    if cancel.load(AtomicOrdering::Relaxed) {
        return Err(CopyError::Cancelled);
    }
    if same_path(source, destination) {
        return Err(CopyError::Io("源文件和目标文件相同".to_string()));
    }
    if fs::metadata(source)?.is_dir() {
        fs::create_dir_all(destination)?;
        for entry in fs::read_dir(source)? {
            let entry = entry?;
            let child_destination = destination.join(entry.file_name());
            copy_entry_with_progress(
                &entry.path(),
                &child_destination,
                current,
                cancel,
                tx,
                total_bytes,
                completed_items,
                completed_bytes,
            )?;
        }
        Ok(())
    } else {
        copy_file_with_progress(
            source,
            destination,
            current,
            cancel,
            tx,
            total_bytes,
            completed_items,
            completed_bytes,
        )
    }
}

fn copy_file_with_progress(
    source: &Path,
    destination: &Path,
    current: &str,
    cancel: &AtomicBool,
    tx: &UnboundedSender<TransferUpdate>,
    total_bytes: u64,
    completed_items: usize,
    completed_bytes: &mut u64,
) -> Result<(), CopyError> {
    if same_path(source, destination) {
        return Err(CopyError::Io("源文件和目标文件相同".to_string()));
    }
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)?;
    }

    let temporary = temporary_path(destination);
    let result = (|| {
        let mut input = File::open(source)?;
        let mut output = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        let mut buffer = vec![0u8; 1024 * 1024];

        loop {
            if cancel.load(AtomicOrdering::Relaxed) {
                return Err(CopyError::Cancelled);
            }
            let read = input.read(&mut buffer)?;
            if read == 0 {
                break;
            }
            output.write_all(&buffer[..read])?;
            *completed_bytes = completed_bytes.saturating_add(read as u64);
            let _ = tx.send(TransferUpdate::Progress {
                total_bytes,
                completed_bytes: *completed_bytes,
                completed_items,
                current: current.to_string(),
            });
        }
        output.flush()?;
        drop(output);
        if cancel.load(AtomicOrdering::Relaxed) {
            return Err(CopyError::Cancelled);
        }
        commit_staged_file(&temporary, destination)?;
        Ok(())
    })();

    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

fn same_path(left: &Path, right: &Path) -> bool {
    match (fs::canonicalize(left), fs::canonicalize(right)) {
        (Ok(left), Ok(right)) => left == right,
        _ => left == right,
    }
}

fn temporary_path(destination: &Path) -> PathBuf {
    let name = destination
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("item");
    loop {
        let id = TEMP_SEQUENCE.fetch_add(1, AtomicOrdering::Relaxed);
        let candidate = destination.with_file_name(format!(
            ".{}.yazi-gui-copying-{}-{}",
            name,
            std::process::id(),
            id
        ));
        if !candidate.exists() {
            return candidate;
        }
    }
}

fn commit_staged_file(temporary: &Path, destination: &Path) -> io::Result<()> {
    if !destination.exists() {
        return fs::rename(temporary, destination);
    }
    if destination.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "目标位置已有同名目录",
        ));
    }

    let backup = temporary_path(destination);
    fs::rename(destination, &backup)?;
    match fs::rename(temporary, destination) {
        Ok(()) => {
            let _ = fs::remove_file(backup);
            Ok(())
        }
        Err(error) => {
            let _ = fs::rename(&backup, destination);
            Err(error)
        }
    }
}
