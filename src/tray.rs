use crate::settings::{Language, translate};
use anyhow::Result;
use tokio::sync::mpsc::{UnboundedReceiver, unbounded_channel};

#[derive(Debug, Clone, Copy)]
pub enum TrayCommand {
    Show,
    Settings,
    Exit,
}

#[cfg(windows)]
pub struct TrayController {
    stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
    language: std::sync::Arc<std::sync::atomic::AtomicU8>,
}

#[cfg(not(windows))]
pub struct TrayController;

impl TrayController {
    pub fn new(language: Language) -> Result<(Self, UnboundedReceiver<TrayCommand>)> {
        #[cfg(windows)]
        {
            let (tx, rx) = unbounded_channel();
            let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
            let thread_stop = stop.clone();
            let language_state =
                std::sync::Arc::new(std::sync::atomic::AtomicU8::new(language_code(language)));
            let thread_language = language_state.clone();
            std::thread::spawn(move || {
                if let Err(error) = tray_thread(tx, thread_stop, thread_language) {
                    eprintln!("[yazi-gui] tray thread failed: {error}");
                }
            });
            return Ok((
                Self {
                    stop,
                    language: language_state,
                },
                rx,
            ));
        }

        #[cfg(not(windows))]
        {
            let _ = language;
            let _ = unbounded_channel::<TrayCommand>();
            Err(anyhow::anyhow!("tray is only supported on Windows"))
        }
    }

    pub fn set_language(&self, language: Language) {
        #[cfg(windows)]
        self.language.store(
            language_code(language),
            std::sync::atomic::Ordering::Relaxed,
        );

        #[cfg(not(windows))]
        let _ = language;
    }
}

#[cfg(windows)]
impl Drop for TrayController {
    fn drop(&mut self) {
        self.stop.store(true, std::sync::atomic::Ordering::Relaxed);
    }
}

#[cfg(windows)]
const WM_TRAYICON: u32 = 0x8000 + 42;
#[cfg(windows)]
const TRAY_ID: usize = 1;
#[cfg(windows)]
const COMMAND_SHOW: usize = 1001;
#[cfg(windows)]
const COMMAND_SETTINGS: usize = 1002;
#[cfg(windows)]
const COMMAND_EXIT: usize = 1003;

fn language_code(language: Language) -> u8 {
    match language {
        Language::System => 0,
        Language::Chinese => 1,
        Language::English => 2,
    }
}

fn language_from_code(code: u8) -> Language {
    match code {
        1 => Language::Chinese,
        2 => Language::English,
        _ => Language::System,
    }
}

#[cfg(windows)]
struct TrayState {
    sender: tokio::sync::mpsc::UnboundedSender<TrayCommand>,
    language: std::sync::Arc<std::sync::atomic::AtomicU8>,
}

#[cfg(windows)]
#[allow(unsafe_op_in_unsafe_fn)]
fn tray_thread(
    tx: tokio::sync::mpsc::UnboundedSender<TrayCommand>,
    stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
    language: std::sync::Arc<std::sync::atomic::AtomicU8>,
) -> Result<()> {
    use std::mem::size_of;
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, POINT, WPARAM};
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::UI::Shell::{
        NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW, Shell_NotifyIconW,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        AppendMenuW, CS_HREDRAW, CS_VREDRAW, CW_USEDEFAULT, CreatePopupMenu, CreateWindowExW,
        DefWindowProcW, DestroyMenu, DestroyWindow, DispatchMessageW, GWLP_USERDATA, GetCursorPos,
        GetMessageW, GetWindowLongPtrW, HMENU, IDC_ARROW, LoadCursorW, LoadIconW, MSG,
        PostQuitMessage, RegisterClassW, SetForegroundWindow, SetWindowLongPtrW, TPM_BOTTOMALIGN,
        TPM_LEFTALIGN, TrackPopupMenu, TranslateMessage, WM_COMMAND, WM_DESTROY, WM_LBUTTONUP,
        WM_RBUTTONUP, WNDCLASSW, WS_OVERLAPPED,
    };

    unsafe extern "system" fn window_proc(
        hwnd: HWND,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        match message {
            WM_TRAYICON if lparam as u32 == WM_LBUTTONUP => {
                send_tray_command(hwnd, TrayCommand::Show);
                0
            }
            WM_TRAYICON if lparam as u32 == WM_RBUTTONUP => {
                show_menu(hwnd);
                0
            }
            WM_COMMAND => {
                match (wparam & 0xffff) as usize {
                    COMMAND_SHOW => send_tray_command(hwnd, TrayCommand::Show),
                    COMMAND_SETTINGS => send_tray_command(hwnd, TrayCommand::Settings),
                    COMMAND_EXIT => send_tray_command(hwnd, TrayCommand::Exit),
                    _ => {}
                }
                0
            }
            WM_DESTROY => {
                PostQuitMessage(0);
                0
            }
            _ => DefWindowProcW(hwnd, message, wparam, lparam),
        }
    }

    unsafe fn send_tray_command(hwnd: HWND, command: TrayCommand) {
        let pointer = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const TrayState;
        if !pointer.is_null() {
            let _ = (*pointer).sender.send(command);
        }
    }

    unsafe fn show_menu(hwnd: HWND) {
        let menu: HMENU = CreatePopupMenu();
        if menu.is_null() {
            return;
        }
        let pointer = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const TrayState;
        let language = if pointer.is_null() {
            Language::System
        } else {
            language_from_code(
                (*pointer)
                    .language
                    .load(std::sync::atomic::Ordering::Relaxed),
            )
        };
        let show = wide(&translate(language, "显示"));
        let settings = wide(&translate(language, "设置"));
        let exit = wide(&translate(language, "退出"));
        AppendMenuW(menu, 0, COMMAND_SHOW, show.as_ptr());
        AppendMenuW(menu, 0, COMMAND_SETTINGS, settings.as_ptr());
        AppendMenuW(menu, 0, COMMAND_EXIT, exit.as_ptr());
        let mut point = POINT { x: 0, y: 0 };
        GetCursorPos(&mut point);
        SetForegroundWindow(hwnd);
        TrackPopupMenu(
            menu,
            TPM_LEFTALIGN | TPM_BOTTOMALIGN,
            point.x,
            point.y,
            0,
            hwnd,
            std::ptr::null(),
        );
        DestroyMenu(menu);
    }

    fn wide(text: &str) -> Vec<u16> {
        std::ffi::OsStr::new(text)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect()
    }

    let instance: HINSTANCE = unsafe { GetModuleHandleW(std::ptr::null()) };
    let class_name = wide("yazi-gui-tray");
    let class = WNDCLASSW {
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: Some(window_proc),
        hInstance: instance,
        hIcon: unsafe { LoadIconW(instance, 1usize as *const u16) },
        hCursor: unsafe { LoadCursorW(std::ptr::null_mut(), IDC_ARROW) },
        lpszClassName: class_name.as_ptr(),
        ..unsafe { std::mem::zeroed() }
    };
    if unsafe { RegisterClassW(&class) } == 0 {
        return Err(anyhow::anyhow!("RegisterClassW failed"));
    }

    let hwnd = unsafe {
        CreateWindowExW(
            0,
            class_name.as_ptr(),
            class_name.as_ptr(),
            WS_OVERLAPPED,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            instance,
            std::ptr::null(),
        )
    };
    if hwnd.is_null() {
        return Err(anyhow::anyhow!("CreateWindowExW failed"));
    }

    let state = Box::new(TrayState {
        sender: tx,
        language,
    });
    let state_ptr = Box::into_raw(state);
    unsafe { SetWindowLongPtrW(hwnd, GWLP_USERDATA, state_ptr as isize) };

    let mut icon_data: NOTIFYICONDATAW = unsafe { std::mem::zeroed() };
    icon_data.cbSize = size_of::<NOTIFYICONDATAW>() as u32;
    icon_data.hWnd = hwnd;
    icon_data.uID = TRAY_ID as u32;
    icon_data.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
    icon_data.uCallbackMessage = WM_TRAYICON;
    icon_data.hIcon = unsafe { LoadIconW(instance, 1usize as *const u16) };
    let tip = wide("yazi-gui");
    let copy_len = tip.len().min(icon_data.szTip.len());
    icon_data.szTip[..copy_len].copy_from_slice(&tip[..copy_len]);
    if unsafe { Shell_NotifyIconW(NIM_ADD, &mut icon_data) } == 0 {
        unsafe {
            drop(Box::from_raw(state_ptr));
            DestroyWindow(hwnd);
        }
        return Err(anyhow::anyhow!("Shell_NotifyIconW(NIM_ADD) failed"));
    }

    let mut message: MSG = unsafe { std::mem::zeroed() };
    while !stop.load(std::sync::atomic::Ordering::Relaxed) {
        let result = unsafe { GetMessageW(&mut message, hwnd, 0, 0) };
        if result <= 0 {
            break;
        }
        unsafe {
            TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }

    unsafe {
        Shell_NotifyIconW(NIM_DELETE, &mut icon_data);
        DestroyWindow(hwnd);
        drop(Box::from_raw(state_ptr));
    }
    Ok(())
}
