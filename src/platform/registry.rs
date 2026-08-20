use crate::settings::Language;
use anyhow::{Result, anyhow};

pub(crate) fn effective_language() -> Language {
    #[cfg(windows)]
    {
        let language = unsafe { windows_sys::Win32::Globalization::GetUserDefaultUILanguage() };
        if language & 0x03ff == 0x0004 {
            return Language::Chinese;
        }
        return Language::English;
    }

    #[cfg(not(windows))]
    {
        Language::Chinese
    }
}

pub(crate) fn set_autostart(enabled: bool) -> Result<()> {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS};
        use windows_sys::Win32::System::Registry::{
            HKEY_CURRENT_USER, KEY_SET_VALUE, REG_SZ, RegCloseKey, RegCreateKeyExW,
            RegDeleteValueW, RegOpenKeyExW, RegSetValueExW,
        };

        let subkey: Vec<u16> =
            std::ffi::OsStr::new(r"Software\Microsoft\Windows\CurrentVersion\Run")
                .encode_wide()
                .chain(std::iter::once(0))
                .collect();
        let value_name: Vec<u16> = std::ffi::OsStr::new("yazi-gui")
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();

        let mut key = std::ptr::null_mut();
        let status = if enabled {
            unsafe {
                RegCreateKeyExW(
                    HKEY_CURRENT_USER,
                    subkey.as_ptr(),
                    0,
                    std::ptr::null_mut(),
                    0,
                    KEY_SET_VALUE,
                    std::ptr::null(),
                    &mut key,
                    std::ptr::null_mut(),
                )
            }
        } else {
            unsafe {
                RegOpenKeyExW(
                    HKEY_CURRENT_USER,
                    subkey.as_ptr(),
                    0,
                    KEY_SET_VALUE,
                    &mut key,
                )
            }
        };

        if status != ERROR_SUCCESS {
            if !enabled && status == ERROR_FILE_NOT_FOUND {
                return Ok(());
            }
            return Err(anyhow!(
                "open Windows startup registry key failed: {}",
                status
            ));
        }

        let result = if enabled {
            let executable = std::env::current_exe()?.to_string_lossy().into_owned();
            let command = format!(r#""{}" --background"#, executable);
            let wide_command: Vec<u16> = std::ffi::OsStr::new(&command)
                .encode_wide()
                .chain(std::iter::once(0))
                .collect();
            let bytes = &wide_command[..wide_command.len() - 1];
            unsafe {
                RegSetValueExW(
                    key,
                    value_name.as_ptr(),
                    0,
                    REG_SZ,
                    bytes.as_ptr().cast(),
                    (wide_command.len() * std::mem::size_of::<u16>()) as u32,
                )
            }
        } else {
            unsafe { RegDeleteValueW(key, value_name.as_ptr()) }
        };
        unsafe { RegCloseKey(key) };
        if result != ERROR_SUCCESS && !(!enabled && result == ERROR_FILE_NOT_FOUND) {
            return Err(anyhow!("update Windows startup entry failed: {}", result));
        }
        Ok(())
    }

    #[cfg(not(windows))]
    {
        let _ = enabled;
        Ok(())
    }
}
