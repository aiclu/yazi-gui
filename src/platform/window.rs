use gpui::{AnyWindowHandle, Context, Window};

#[cfg(windows)]
use raw_window_handle::{HasWindowHandle, RawWindowHandle};

#[cfg(windows)]
fn hwnd(window: &Window) -> Option<windows_sys::Win32::Foundation::HWND> {
    match HasWindowHandle::window_handle(window).ok()?.as_raw() {
        RawWindowHandle::Win32(handle) => Some(handle.hwnd.get() as *mut std::ffi::c_void),
        _ => None,
    }
}

pub(crate) fn show_error_message(message: &str) -> ! {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::UI::WindowsAndMessaging::{MB_ICONERROR, MB_OK, MessageBoxW};
        let text: Vec<u16> = std::ffi::OsStr::new(message)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        let title: Vec<u16> = std::ffi::OsStr::new("yazi-gui")
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        unsafe {
            MessageBoxW(
                std::ptr::null_mut(),
                text.as_ptr(),
                title.as_ptr(),
                MB_OK | MB_ICONERROR,
            );
        }
        std::process::exit(1);
    }

    #[cfg(not(windows))]
    panic!("{message}");
}

pub(crate) fn hide(window: &Window) {
    #[cfg(windows)]
    if let Some(hwnd) = hwnd(window) {
        unsafe {
            windows_sys::Win32::UI::WindowsAndMessaging::ShowWindow(
                hwnd,
                windows_sys::Win32::UI::WindowsAndMessaging::SW_HIDE,
            );
        }
    }

    #[cfg(not(windows))]
    let _ = window;
}

pub(crate) fn toggle_maximize(window: &Window) {
    #[cfg(windows)]
    if let Some(hwnd) = hwnd(window) {
        let command = if window.is_maximized() {
            windows_sys::Win32::UI::WindowsAndMessaging::SW_RESTORE
        } else {
            windows_sys::Win32::UI::WindowsAndMessaging::SW_MAXIMIZE
        };
        unsafe {
            windows_sys::Win32::UI::WindowsAndMessaging::ShowWindowAsync(hwnd, command);
        }
    }

    #[cfg(not(windows))]
    let _ = window;
}

pub(crate) fn begin_move(window: &mut Window) {
    #[cfg(windows)]
    if let Some(hwnd) = hwnd(window) {
        unsafe {
            windows_sys::Win32::UI::Input::KeyboardAndMouse::ReleaseCapture();
            windows_sys::Win32::UI::WindowsAndMessaging::SendMessageW(
                hwnd,
                windows_sys::Win32::UI::WindowsAndMessaging::WM_NCLBUTTONDOWN,
                windows_sys::Win32::UI::WindowsAndMessaging::HTCAPTION as usize,
                0,
            );
        }
    }

    #[cfg(not(windows))]
    window.start_window_move();
}

pub(crate) fn request_close(window: &Window) -> bool {
    #[cfg(windows)]
    if let Some(hwnd) = hwnd(window) {
        unsafe {
            windows_sys::Win32::UI::WindowsAndMessaging::PostMessageW(
                hwnd,
                windows_sys::Win32::UI::WindowsAndMessaging::WM_CLOSE,
                0,
                0,
            );
        }
        return true;
    }

    #[cfg(not(windows))]
    let _ = window;
    false
}

pub(crate) fn show(handle: AnyWindowHandle, cx: &mut Context<crate::Root>) {
    let _ = handle.update(cx, |_, window, _| {
        #[cfg(windows)]
        if let Some(hwnd) = hwnd(window) {
            unsafe {
                windows_sys::Win32::UI::WindowsAndMessaging::ShowWindow(
                    hwnd,
                    windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOW,
                );
                windows_sys::Win32::UI::WindowsAndMessaging::SetForegroundWindow(hwnd);
            }
        }

        #[cfg(not(windows))]
        window.activate_window();
    });
}

pub(crate) fn wait_for_process_exit(parent_pid: u32) {
    #[cfg(windows)]
    {
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
    }

    #[cfg(not(windows))]
    let _ = parent_pid;
}
