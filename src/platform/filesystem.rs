use std::io;
use std::path::Path;

pub(crate) fn scan_drives() -> Vec<String> {
    (b'A'..=b'Z')
        .map(|c| format!("{}:\\", c as char))
        .filter(|drive| Path::new(drive).exists())
        .collect()
}

pub(crate) fn is_unc_path(path: &str) -> bool {
    let extended_prefix = "\\\\?\\";
    if let Some(rest) = path.strip_prefix(extended_prefix) {
        return rest
            .get(..4)
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case("UNC\\"));
    }
    path.starts_with("\\\\")
}

pub(crate) fn is_network_path(path: &Path) -> io::Result<bool> {
    let text = path.to_string_lossy();
    if is_unc_path(&text) {
        return Ok(true);
    }

    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::Storage::FileSystem::GetDriveTypeW;
        use windows_sys::Win32::System::WindowsProgramming::DRIVE_REMOTE;

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
