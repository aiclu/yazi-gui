use gpui::*;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use std::ops::Range;
#[cfg(windows)]
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::sync::{
    Arc, OnceLock,
    atomic::{AtomicBool, Ordering as AtomicOrdering},
};
use tokio::sync::mpsc::{UnboundedReceiver, unbounded_channel};
use unicode_segmentation::UnicodeSegmentation;

actions!(
    yazi_input,
    [
        Backspace,
        Delete,
        Left,
        Right,
        SelectLeft,
        SelectRight,
        SelectAll,
        Home,
        End,
        PasteText,
        CopyText,
        CutText,
    ]
);

mod yazi;
use yazi::{FileEntry, YaziClient, YaziEvent};
mod fs_ops;
#[cfg(test)]
pub(crate) use fs_ops::{DeleteSummary, TEMP_SEQUENCE, is_unc_path, permanent_delete};
use fs_ops::{
    TransferOutcome, TransferUpdate, copy_paths_with_progress, create_directory, create_file,
    delete_paths_with_policy, delete_status, file_name_of, format_mtime, is_drive_root, move_paths,
    network_path_count, rename_path, scan_drives, scan_folder_children, search_directory,
};
mod workspace;
use workspace::{
    FolderTreeRow, FolderTreeRowKind, FolderTreeState, Preview, SearchState, SortDirection,
    SortField, SortState, Tab, reconcile_selection, sort_files, tree_ancestor_paths, tree_path_key,
};
mod ui;
use ui::{
    InputElement, action_button, command_button, computer_button, dialog_button, menu_item,
    menu_items_for, new_tab_button, parent_button, refresh_button, tab_button, tab_name,
    toolbar_divider,
};
mod settings;
use settings::{AppSettings, Language, ThemeMode};
mod tray;
use tray::{TrayCommand, TrayController};
mod update;

const PRIMARY_YAZI_TAB: usize = 1;

fn default_start_dir() -> String {
    std::env::current_dir()
        .expect("current working directory is unavailable")
        .to_string_lossy()
        .into_owned()
}

fn load_settings_or_exit() -> AppSettings {
    match settings::load() {
        Ok(settings) => settings,
        Err(error) => {
            let message = format!("设置文件无效：{error}");
            #[cfg(windows)]
            {
                use windows_sys::Win32::UI::WindowsAndMessaging::{
                    MB_ICONERROR, MB_OK, MessageBoxW,
                };
                let text: Vec<u16> = std::ffi::OsStr::new(&message)
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
    }
}

/// 界面配色主题。颜色采用 Catppuccin 色板。
#[derive(Clone, Copy)]
struct Theme {
    base: Hsla,
    mantle: Hsla,
    surface0: Hsla,
    crust: Hsla,
    border: Hsla,
    hover: Hsla,
    selected: Hsla,
    text: Hsla,
    muted: Hsla,
    blue: Hsla,
    syntax_theme: &'static str,
}

impl Theme {
    /// 暗色（参考 Win11 Fluent dark palette）。
    fn dark() -> Self {
        Theme {
            base: rgb(0x202020).into(),
            mantle: rgb(0x252525).into(),
            surface0: rgb(0x2d2d2d).into(),
            crust: rgb(0x1b1b1b).into(),
            border: rgb(0x3a3a3a).into(),
            hover: rgb(0x383838).into(),
            selected: rgb(0x3a3a3a).into(),
            text: rgb(0xf5f5f5).into(),
            muted: rgb(0xb3b3b3).into(),
            blue: rgb(0x60cdff).into(),
            syntax_theme: "base16-ocean.dark",
        }
    }

    /// 浅色（参考 Win11 Fluent light palette）。
    fn light() -> Self {
        Theme {
            base: rgb(0xf9f9f9).into(),
            mantle: rgb(0xf3f3f3).into(),
            surface0: rgb(0xffffff).into(),
            crust: rgb(0xf6f6f6).into(),
            border: rgb(0xe5e5e5).into(),
            hover: rgb(0xf0f0f0).into(),
            selected: rgb(0xe5f1fb).into(),
            text: rgb(0x1a1a1a).into(),
            muted: rgb(0x616161).into(),
            blue: rgb(0x0067c0).into(),
            syntax_theme: "InspiredGitHub",
        }
    }
}

/// 输入模式下的待处理操作。
#[derive(Clone)]
enum PendingOp {
    Rename { path: String },
    NewFile,
    NewDir,
    GoToPath,
    Search,
    EditShortcut(ShortcutAction),
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Page {
    Files,
    Settings,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ShortcutAction {
    Open,
    Search,
    NewTab,
    CloseTab,
    Delete,
    Rename,
    Copy,
    Cut,
    Paste,
    NewFile,
    NewDir,
}

impl ShortcutAction {
    fn label(self) -> &'static str {
        match self {
            Self::Open => "打开",
            Self::Search => "搜索",
            Self::NewTab => "新建标签页",
            Self::CloseTab => "关闭标签页",
            Self::Delete => "删除",
            Self::Rename => "重命名",
            Self::Copy => "复制",
            Self::Cut => "剪切",
            Self::Paste => "粘贴",
            Self::NewFile => "新建文件",
            Self::NewDir => "新建文件夹",
        }
    }

    fn shortcut<'a>(self, shortcuts: &'a settings::ShortcutSettings) -> &'a str {
        match self {
            Self::Open => &shortcuts.open,
            Self::Search => &shortcuts.search,
            Self::NewTab => &shortcuts.new_tab,
            Self::CloseTab => &shortcuts.close_tab,
            Self::Delete => &shortcuts.delete,
            Self::Rename => &shortcuts.rename,
            Self::Copy => &shortcuts.copy,
            Self::Cut => &shortcuts.cut,
            Self::Paste => &shortcuts.paste,
            Self::NewFile => &shortcuts.new_file,
            Self::NewDir => &shortcuts.new_dir,
        }
    }

    fn set_shortcut(self, shortcuts: &mut settings::ShortcutSettings, value: String) {
        match self {
            Self::Open => shortcuts.open = value,
            Self::Search => shortcuts.search = value,
            Self::NewTab => shortcuts.new_tab = value,
            Self::CloseTab => shortcuts.close_tab = value,
            Self::Delete => shortcuts.delete = value,
            Self::Rename => shortcuts.rename = value,
            Self::Copy => shortcuts.copy = value,
            Self::Cut => shortcuts.cut = value,
            Self::Paste => shortcuts.paste = value,
            Self::NewFile => shortcuts.new_file = value,
            Self::NewDir => shortcuts.new_dir = value,
        }
    }

    fn id(self) -> &'static str {
        match self {
            Self::Open => "shortcut-open",
            Self::Search => "shortcut-search",
            Self::NewTab => "shortcut-new-tab",
            Self::CloseTab => "shortcut-close-tab",
            Self::Delete => "shortcut-delete",
            Self::Rename => "shortcut-rename",
            Self::Copy => "shortcut-copy",
            Self::Cut => "shortcut-cut",
            Self::Paste => "shortcut-paste",
            Self::NewFile => "shortcut-new-file",
            Self::NewDir => "shortcut-new-dir",
        }
    }
}

/// 文件复制/剪切剪贴板。
struct Clipboard {
    paths: Vec<String>,
    cut: bool,
}

/// 右键上下文菜单状态。
struct MenuState {
    /// 右键命中的文件/目录名（None = 空白处）。
    target: Option<String>,
    position: Point<Pixels>,
}

struct PendingDelete {
    cwd: String,
    paths: Vec<String>,
    network_count: usize,
}

struct TransferProgress {
    completed_bytes: u64,
    total_bytes: u64,
    completed_items: usize,
    total_items: usize,
    current: String,
}

struct TransferState {
    progress: TransferProgress,
    cancel: Arc<AtomicBool>,
}

enum UpdatePhase {
    Idle,
    Checking,
    UpToDate {
        version: String,
    },
    Available(update::ReleaseInfo),
    Downloading {
        progress: update::DownloadProgress,
    },
    Ready {
        release: update::ReleaseInfo,
        archive: PathBuf,
        progress: update::DownloadProgress,
    },
    Restarting,
    Failed(String),
}

struct UpdateState {
    phase: UpdatePhase,
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

/// 右键菜单项。
#[derive(Clone, Copy)]
enum MenuAction {
    Open,
    Favorite,
    Rename,
    Delete,
    Copy,
    Cut,
    Paste,
    NewFile,
    NewDir,
}

struct Root {
    client: Option<YaziClient>,
    tabs: Vec<Tab>,
    active: usize,
    drive_roots: Vec<String>,
    clipboard: Option<Clipboard>,
    transfer: Option<TransferState>,
    pending_delete: Option<PendingDelete>,
    delete_in_progress: bool,
    menu: Option<MenuState>,
    preview_collapsed: bool,
    folder_tree: FolderTreeState,
    focus_handle: FocusHandle,
    input_blur_subscription: Option<Subscription>,
    pending: Option<PendingOp>,
    input: String,
    input_selection: Range<usize>,
    selection_reversed: bool,
    marked_range: Option<Range<usize>>,
    input_layout: Option<ShapedLine>,
    input_bounds: Option<Bounds<Pixels>>,
    input_selecting: bool,
    status: Option<String>,
    theme: Theme,
    settings: AppSettings,
    page: Page,
    tray: Option<TrayController>,
    window_handle: Option<AnyWindowHandle>,
    close_requested: bool,
    shortcuts_expanded: bool,
    update: UpdateState,
}

impl Root {
    fn new(cx: &mut Context<Self>) -> Self {
        let settings = load_settings_or_exit();
        let start_dir = default_start_dir();
        let theme = match settings.theme {
            ThemeMode::Dark => Theme::dark(),
            ThemeMode::Light => Theme::light(),
        };
        let (tray, mut tray_rx) = match TrayController::new(settings.language) {
            Ok((tray, rx)) => (Some(tray), Some(rx)),
            Err(error) => {
                eprintln!("[yazi-gui] tray unavailable: {error}");
                (None, None)
            }
        };
        let mut root = Self {
            client: None,
            tabs: vec![Tab::new(&start_dir)],
            active: 0,
            drive_roots: Vec::new(),
            clipboard: None,
            transfer: None,
            pending_delete: None,
            delete_in_progress: false,
            menu: None,
            preview_collapsed: true,
            folder_tree: FolderTreeState::default(),
            focus_handle: cx.focus_handle(),
            input_blur_subscription: None,
            pending: None,
            input: String::new(),
            input_selection: 0..0,
            selection_reversed: false,
            marked_range: None,
            input_layout: None,
            input_bounds: None,
            input_selecting: false,
            status: None,
            theme,
            settings,
            page: Page::Files,
            tray,
            window_handle: None,
            close_requested: false,
            shortcuts_expanded: false,
            update: UpdateState::default(),
        };
        if let Some(mut rx) = tray_rx.take() {
            cx.spawn(async move |weak, cx| {
                while let Some(command) = rx.recv().await {
                    weak.update(cx, |this, cx| this.handle_tray_command(command, cx))
                        .ok();
                }
            })
            .detach();
        }
        root.start_yazi(cx);
        root.start_drive_scan(cx, false);
        root
    }

    fn cur(&self) -> &Tab {
        &self.tabs[self.active]
    }

    fn cur_mut(&mut self) -> &mut Tab {
        &mut self.tabs[self.active]
    }

    fn visible_files(&self) -> &[FileEntry] {
        self.cur()
            .search
            .as_ref()
            .filter(|search| !search.query.is_empty())
            .map(|search| search.results.as_slice())
            .unwrap_or_else(|| self.cur().files.as_slice())
    }

    fn is_inline_editing(&self) -> bool {
        matches!(
            self.pending,
            Some(PendingOp::Rename { .. } | PendingOp::NewFile | PendingOp::NewDir)
        )
    }

    fn reset_input_editor(&mut self, text: String, select_all: bool) {
        let end = text.len();
        self.input = text;
        self.input_selection = if select_all { 0..end } else { end..end };
        self.selection_reversed = false;
        self.marked_range = None;
        self.input_layout = None;
        self.input_bounds = None;
        self.input_selecting = false;
    }

    fn clear_input_editor(&mut self) {
        self.reset_input_editor(String::new(), false);
    }

    fn cancel_input_state(&mut self) {
        self.pending = None;
        self.cur_mut().cancel_search();
        self.clear_input_editor();
    }

    fn set_status(&mut self, status: impl Into<String>) {
        self.status = Some(settings::translate_status(self.language(), &status.into()));
    }

    fn clear_status(&mut self) {
        self.status = None;
    }

    fn language(&self) -> Language {
        self.settings.language
    }

    fn tr(&self, text: &str) -> String {
        settings::translate(self.language(), text)
    }

    fn persist_settings(&mut self, cx: &mut Context<Self>) {
        if let Err(error) = settings::save(&self.settings) {
            self.set_status(format!("{}: {}", self.tr("保存失败"), error));
        }
        cx.notify();
    }

    fn set_theme_mode(&mut self, theme: ThemeMode, cx: &mut Context<Self>) {
        self.settings.theme = theme;
        self.theme = match theme {
            ThemeMode::Dark => Theme::dark(),
            ThemeMode::Light => Theme::light(),
        };
        self.persist_settings(cx);
    }

    fn show_files(&mut self, cx: &mut Context<Self>) {
        self.page = Page::Files;
        cx.notify();
    }

    fn show_settings(&mut self, cx: &mut Context<Self>) {
        self.cancel_input_state();
        self.menu = None;
        self.page = Page::Settings;
        cx.notify();
    }

    fn update_is_busy(&self) -> bool {
        matches!(
            &self.update.phase,
            UpdatePhase::Checking | UpdatePhase::Downloading { .. } | UpdatePhase::Restarting
        )
    }

    fn check_for_updates(&mut self, cx: &mut Context<Self>) {
        if self.update_is_busy() {
            return;
        }
        self.update.request_id = self.update.request_id.wrapping_add(1);
        let request_id = self.update.request_id;
        self.update.phase = UpdatePhase::Checking;
        self.clear_status();
        cx.notify();
        cx.spawn(async move |weak, cx| {
            let result = cx
                .background_executor()
                .spawn(async { update::check_latest_release() })
                .await;
            weak.update(cx, |this, cx| {
                if this.update.request_id != request_id {
                    return;
                }
                match result {
                    Ok(release) if update::is_newer_than_current(&release) => {
                        this.set_status(format!("{}: {}", this.tr("发现新版本"), release.tag_name));
                        this.update.phase = UpdatePhase::Available(release);
                    }
                    Ok(release) => {
                        this.set_status(format!(
                            "{}: {}",
                            this.tr("已是最新版本"),
                            release.tag_name
                        ));
                        this.update.phase = UpdatePhase::UpToDate {
                            version: release.tag_name,
                        };
                    }
                    Err(error) => {
                        this.set_status(format!("{}: {}", this.tr("检查更新失败"), error));
                        this.update.phase = UpdatePhase::Failed(error.to_string());
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn download_update(&mut self, cx: &mut Context<Self>) {
        let release = match &self.update.phase {
            UpdatePhase::Available(release) => release.clone(),
            _ => return,
        };
        let cancel = Arc::new(AtomicBool::new(false));
        let (tx, rx) = unbounded_channel();
        self.update.request_id = self.update.request_id.wrapping_add(1);
        let request_id = self.update.request_id;
        self.update.cancel = Some(cancel.clone());
        self.update.phase = UpdatePhase::Downloading {
            progress: update::DownloadProgress {
                downloaded: 0,
                total: release.asset_size,
                bytes_per_second: 0,
            },
        };
        self.set_status(self.tr("正在下载更新"));
        cx.notify();
        cx.spawn(async move |weak, cx| {
            let worker_release = release.clone();
            let worker = cx
                .background_executor()
                .spawn(async move { update::download_release(&worker_release, cancel, &tx) });
            let mut rx = rx;
            while let Some(progress) = rx.recv().await {
                weak.update(cx, |this, cx| {
                    if this.update.request_id != request_id {
                        return;
                    }
                    if let UpdatePhase::Downloading {
                        progress: current, ..
                    } = &mut this.update.phase
                    {
                        *current = progress;
                    }
                    cx.notify();
                })
                .ok();
            }
            let result = worker.await;
            weak.update(cx, |this, cx| {
                if this.update.request_id != request_id {
                    return;
                }
                this.update.cancel = None;
                match result {
                    Ok(update::DownloadResult::Completed(archive)) => {
                        let progress = match &this.update.phase {
                            UpdatePhase::Downloading { progress, .. } => *progress,
                            _ => update::DownloadProgress::default(),
                        };
                        this.update.phase = UpdatePhase::Ready {
                            release: release.clone(),
                            archive,
                            progress,
                        };
                        this.set_status(this.tr("下载完成，可重启更新"));
                    }
                    Ok(update::DownloadResult::Cancelled) => {
                        this.update.phase = UpdatePhase::Available(release.clone());
                        this.set_status(this.tr("下载已取消"));
                    }
                    Err(error) => {
                        this.update.phase = UpdatePhase::Failed(error.to_string());
                        this.set_status(format!("{}: {}", this.tr("下载更新失败"), error));
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn cancel_update_download(&mut self, cx: &mut Context<Self>) {
        if let Some(cancel) = &self.update.cancel {
            cancel.store(true, AtomicOrdering::Relaxed);
            self.set_status(self.tr("正在取消下载"));
            cx.notify();
        }
    }

    fn restart_update(&mut self, cx: &mut Context<Self>) {
        let archive = match &self.update.phase {
            UpdatePhase::Ready { archive, .. } => archive.clone(),
            _ => return,
        };
        match update::spawn_restart_update(&archive) {
            Ok(()) => {
                self.update.phase = UpdatePhase::Restarting;
                self.close_requested = true;
                cx.notify();
                cx.quit();
            }
            Err(error) => {
                self.update.phase = UpdatePhase::Failed(error.to_string());
                self.set_status(format!("{}: {}", self.tr("启动更新失败"), error));
                cx.notify();
            }
        }
    }

    fn open_external_url(&self, url: String, cx: &mut Context<Self>) {
        cx.spawn(async move |_weak, cx| {
            cx.background_executor()
                .spawn(async move {
                    let _ = open::that(url);
                })
                .await;
        })
        .detach();
    }

    fn handle_tray_command(&mut self, command: TrayCommand, cx: &mut Context<Self>) {
        match command {
            TrayCommand::Show => {
                self.show_native_window(cx);
                cx.notify();
            }
            TrayCommand::Settings => {
                self.show_native_window(cx);
                self.show_settings(cx);
            }
            TrayCommand::Exit => {
                if self.has_active_work() {
                    self.set_status(self.tr("正在进行文件操作，请完成后再退出"));
                    cx.notify();
                } else {
                    self.close_requested = true;
                    cx.quit();
                }
            }
        }
    }

    fn has_active_work(&self) -> bool {
        self.transfer.is_some()
            || self.delete_in_progress
            || self.update_is_busy()
            || self.tabs.iter().any(|tab| tab.refreshing)
    }

    fn handle_window_close(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if self.close_requested {
            return true;
        }
        if self.has_active_work() {
            let message = if self.tabs.iter().any(|tab| tab.refreshing) {
                self.tr("正在刷新，请完成后再退出")
            } else {
                self.tr("正在进行文件操作，请完成后再退出")
            };
            self.set_status(message);
            cx.notify();
            return false;
        }
        if self.tray.is_none() {
            return true;
        }
        self.hide_native_window(window);
        false
    }

    #[cfg(windows)]
    fn window_hwnd(window: &Window) -> Option<windows_sys::Win32::Foundation::HWND> {
        match HasWindowHandle::window_handle(window).ok()?.as_raw() {
            RawWindowHandle::Win32(handle) => Some(handle.hwnd.get() as *mut std::ffi::c_void),
            _ => None,
        }
    }

    #[cfg(windows)]
    fn hide_native_window(&self, window: &Window) {
        if let Some(hwnd) = Self::window_hwnd(window) {
            unsafe {
                windows_sys::Win32::UI::WindowsAndMessaging::ShowWindow(
                    hwnd,
                    windows_sys::Win32::UI::WindowsAndMessaging::SW_HIDE,
                );
            }
        }
    }

    #[cfg(not(windows))]
    fn hide_native_window(&self, _window: &Window) {}

    fn show_native_window(&self, cx: &mut Context<Self>) {
        let Some(handle) = self.window_handle else {
            return;
        };
        let _ = handle.update(cx, |_, window, _| {
            #[cfg(windows)]
            if let Some(hwnd) = Root::window_hwnd(window) {
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

    fn input_cursor_offset(&self) -> usize {
        if self.selection_reversed {
            self.input_selection.start
        } else {
            self.input_selection.end
        }
    }

    fn move_input_cursor(&mut self, offset: usize, cx: &mut Context<Self>) {
        let offset = offset.min(self.input.len());
        self.input_selection = offset..offset;
        self.selection_reversed = false;
        cx.notify();
    }

    fn select_input_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        let offset = offset.min(self.input.len());
        if self.selection_reversed {
            self.input_selection.start = offset;
        } else {
            self.input_selection.end = offset;
        }
        if self.input_selection.end < self.input_selection.start {
            self.selection_reversed = !self.selection_reversed;
            self.input_selection = self.input_selection.end..self.input_selection.start;
        }
        cx.notify();
    }

    fn previous_input_boundary(&self, offset: usize) -> usize {
        self.input
            .grapheme_indices(true)
            .rev()
            .find_map(|(index, _)| (index < offset).then_some(index))
            .unwrap_or(0)
    }

    fn next_input_boundary(&self, offset: usize) -> usize {
        self.input
            .grapheme_indices(true)
            .find_map(|(index, _)| (index > offset).then_some(index))
            .unwrap_or(self.input.len())
    }

    fn input_offset_to_utf16(&self, offset: usize) -> usize {
        self.input[..offset.min(self.input.len())]
            .chars()
            .map(char::len_utf16)
            .sum()
    }

    fn input_offset_from_utf16(&self, offset: usize) -> usize {
        let mut utf8_offset = 0;
        let mut utf16_count = 0;
        for ch in self.input.chars() {
            if utf16_count >= offset {
                break;
            }
            utf16_count += ch.len_utf16();
            utf8_offset += ch.len_utf8();
        }
        utf8_offset.min(self.input.len())
    }

    fn input_range_to_utf16(&self, range: &Range<usize>) -> Range<usize> {
        self.input_offset_to_utf16(range.start)..self.input_offset_to_utf16(range.end)
    }

    fn input_range_from_utf16(&self, range: &Range<usize>) -> Range<usize> {
        self.input_offset_from_utf16(range.start)..self.input_offset_from_utf16(range.end)
    }

    fn replace_input_range(&mut self, range: Range<usize>, text: &str, cx: &mut Context<Self>) {
        let range = range.start.min(self.input.len())..range.end.min(self.input.len());
        self.input.replace_range(range.clone(), text);
        let cursor = range.start + text.len();
        self.input_selection = cursor..cursor;
        self.selection_reversed = false;
        self.marked_range = None;
        if matches!(self.pending, Some(PendingOp::Search)) {
            self.update_search_query(cx);
        }
        cx.notify();
    }

    fn replace_current_input(&mut self, text: &str, cx: &mut Context<Self>) {
        self.replace_input_range(self.input_selection.clone(), text, cx);
    }

    fn input_byte_index_for_point(&self, point: Point<Pixels>) -> Option<usize> {
        let bounds = self.input_bounds.as_ref()?;
        let line = self.input_layout.as_ref()?;
        let local = bounds.localize(&point)?;
        Some(line.closest_index_for_x(local.x).min(self.input.len()))
    }

    fn input_backspace(&mut self, _: &Backspace, _window: &mut Window, cx: &mut Context<Self>) {
        if self.input_selection.is_empty() {
            let cursor = self.input_cursor_offset();
            if cursor == 0 {
                return;
            }
            self.input_selection = self.previous_input_boundary(cursor)..cursor;
        }
        self.replace_current_input("", cx);
    }

    fn input_delete(&mut self, _: &Delete, _window: &mut Window, cx: &mut Context<Self>) {
        if self.input_selection.is_empty() {
            let cursor = self.input_cursor_offset();
            if cursor == self.input.len() {
                return;
            }
            self.input_selection = cursor..self.next_input_boundary(cursor);
        }
        self.replace_current_input("", cx);
    }

    fn input_left(&mut self, _: &Left, _window: &mut Window, cx: &mut Context<Self>) {
        if self.input_selection.is_empty() {
            self.move_input_cursor(self.previous_input_boundary(self.input_cursor_offset()), cx);
        } else {
            self.move_input_cursor(self.input_selection.start, cx);
        }
    }

    fn input_right(&mut self, _: &Right, _window: &mut Window, cx: &mut Context<Self>) {
        if self.input_selection.is_empty() {
            self.move_input_cursor(self.next_input_boundary(self.input_cursor_offset()), cx);
        } else {
            self.move_input_cursor(self.input_selection.end, cx);
        }
    }

    fn input_select_left(&mut self, _: &SelectLeft, _window: &mut Window, cx: &mut Context<Self>) {
        self.select_input_to(self.previous_input_boundary(self.input_cursor_offset()), cx);
    }

    fn input_select_right(
        &mut self,
        _: &SelectRight,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.select_input_to(self.next_input_boundary(self.input_cursor_offset()), cx);
    }

    fn input_select_all(&mut self, _: &SelectAll, _window: &mut Window, cx: &mut Context<Self>) {
        self.input_selection = 0..self.input.len();
        self.selection_reversed = false;
        cx.notify();
    }

    fn input_home(&mut self, _: &Home, _window: &mut Window, cx: &mut Context<Self>) {
        self.move_input_cursor(0, cx);
    }

    fn input_end(&mut self, _: &End, _window: &mut Window, cx: &mut Context<Self>) {
        self.move_input_cursor(self.input.len(), cx);
    }

    fn input_paste(&mut self, _: &PasteText, _window: &mut Window, cx: &mut Context<Self>) {
        let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) else {
            self.set_status("剪贴板为空或不含文本");
            cx.notify();
            return;
        };
        let text = if matches!(self.pending.as_ref(), Some(PendingOp::GoToPath)) {
            match normalize_single_path(&text) {
                Ok(path) => path,
                Err(message) => {
                    self.set_status(message);
                    cx.notify();
                    return;
                }
            }
        } else {
            text.replace('\r', " ").replace('\n', " ")
        };
        self.replace_current_input(&text, cx);
    }

    fn input_copy(&mut self, _: &CopyText, _window: &mut Window, cx: &mut Context<Self>) {
        if self.input_selection.is_empty() {
            return;
        }
        cx.write_to_clipboard(ClipboardItem::new_string(
            self.input[self.input_selection.clone()].to_string(),
        ));
    }

    fn input_cut(&mut self, _: &CutText, _window: &mut Window, cx: &mut Context<Self>) {
        if self.input_selection.is_empty() {
            return;
        }
        cx.write_to_clipboard(ClipboardItem::new_string(
            self.input[self.input_selection.clone()].to_string(),
        ));
        self.replace_current_input("", cx);
    }

    fn input_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.focus_handle.focus(window);
        self.input_selecting = true;
        let offset = self.input_byte_index_for_point(event.position).unwrap_or(0);
        if event.modifiers.shift {
            self.select_input_to(offset, cx);
        } else {
            self.move_input_cursor(offset, cx);
        }
    }

    fn input_mouse_up(&mut self, _: &MouseUpEvent, _window: &mut Window, _cx: &mut Context<Self>) {
        self.input_selecting = false;
    }

    fn input_mouse_move(
        &mut self,
        event: &MouseMoveEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.input_selecting {
            if let Some(offset) = self.input_byte_index_for_point(event.position) {
                self.select_input_to(offset, cx);
            }
        }
    }

    fn start_drive_scan(&mut self, cx: &mut Context<Self>, mark_refresh: bool) {
        let active = self.active;
        let scan_id = {
            let tab = &mut self.tabs[active];
            tab.drive_scan_id = tab.drive_scan_id.wrapping_add(1);
            tab.drive_scan_id
        };
        if mark_refresh {
            self.tabs[active].refreshing = true;
            self.set_status(self.tr("正在刷新..."));
        }
        cx.spawn(async move |weak, cx| {
            let drives = cx
                .background_executor()
                .spawn(async move { scan_drives() })
                .await;
            weak.update(cx, |this, cx| {
                if active >= this.tabs.len() || this.tabs[active].drive_scan_id != scan_id {
                    return;
                }
                this.drive_roots = drives;
                let names = this.drive_roots.clone();
                for (index, tab) in this.tabs.iter_mut().enumerate() {
                    if !tab.computer_view {
                        continue;
                    }
                    reconcile_selection(&mut tab.selected, &mut tab.anchor, &names);
                    if mark_refresh && index == active {
                        tab.invalidate_refresh();
                    }
                }
                if !this.cur().computer_view {
                    let cwd = this.cur().cwd.clone();
                    this.sync_folder_tree_to_path(&cwd, cx);
                }
                if mark_refresh && this.active == active {
                    this.set_status(format!(
                        "{}，{} {}",
                        this.tr("已刷新"),
                        this.drive_roots.len(),
                        this.tr("个磁盘")
                    ));
                }
                this.reconcile_preview();
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn start_folder_tree_scan(&mut self, path: String, force: bool, cx: &mut Context<Self>) {
        let Some(scan_id) = self.folder_tree.begin_scan(&path, force) else {
            return;
        };
        let scan_path = path.clone();
        cx.spawn(async move |weak, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { scan_folder_children(Path::new(&scan_path)) })
                .await;
            weak.update(cx, |this, cx| {
                this.folder_tree.finish_scan(&path, scan_id, result);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn toggle_folder_tree(&mut self, path: String, is_link: bool, cx: &mut Context<Self>) {
        if is_link {
            return;
        }
        let expanded = self.folder_tree.toggle(&path);
        if expanded {
            self.start_folder_tree_scan(path, false, cx);
        }
        cx.notify();
    }

    fn sync_folder_tree_to_path(&mut self, path: &str, cx: &mut Context<Self>) {
        let ancestors = tree_ancestor_paths(path);
        for ancestor in ancestors {
            if !self.folder_tree.is_expanded(&ancestor) {
                self.folder_tree.toggle(&ancestor);
            }
            self.start_folder_tree_scan(ancestor, false, cx);
        }
    }

    fn refresh_folder_tree(&mut self, cx: &mut Context<Self>) {
        let mut paths = self.folder_tree.expanded_paths();
        if !self.cur().computer_view {
            paths.push(self.cur().cwd.clone());
        }
        paths.sort_by_key(|path| tree_path_key(path));
        paths.dedup_by(|a, b| tree_path_key(a) == tree_path_key(b));
        for path in paths {
            self.start_folder_tree_scan(path, true, cx);
        }
    }

    fn navigate_to_directory(&mut self, target: String, cx: &mut Context<Self>) {
        if !Path::new(&target).is_dir() {
            self.set_status(format!("{}: {}", self.tr("路径不可用"), target));
            cx.notify();
            return;
        }
        self.clear_status();
        match self.send_checked(&["cd", target.as_str()]) {
            Ok(()) => {
                self.cancel_input_state();
                self.menu = None;
                let tab = self.cur_mut();
                tab.selected.clear();
                tab.anchor = None;
                tab.computer_view = false;
                tab.invalidate_refresh();
                tab.clear_preview();
                self.sync_folder_tree_to_path(&target, cx);
                self.set_status(format!("已跳转: {}", target));
            }
            Err(error) => self.set_status(format!("跳转失败: {}", error)),
        }
        cx.notify();
    }

    fn toggle_preview(&mut self, cx: &mut Context<Self>) {
        self.preview_collapsed = !self.preview_collapsed;
        if !self.preview_collapsed {
            self.update_preview_for_selection(cx);
        }
        cx.notify();
    }

    fn is_favorite(&self, path: &str) -> bool {
        let key = favorite_path_key(path);
        self.settings
            .favorites
            .iter()
            .any(|favorite| favorite_path_key(favorite) == key)
    }

    fn toggle_favorite_path(&mut self, path: String, cx: &mut Context<Self>) {
        let path = normalize_favorite_path(&path);
        let key = favorite_path_key(&path);
        let previous = self.settings.favorites.clone();
        if let Some(index) = self
            .settings
            .favorites
            .iter()
            .position(|favorite| favorite_path_key(favorite) == key)
        {
            self.settings.favorites.remove(index);
            self.set_status(format!("{}: {}", self.tr("取消收藏"), path));
        } else {
            self.settings.favorites.push(path.clone());
            self.set_status(format!("{}: {}", self.tr("收藏"), path));
        }
        if let Err(error) = settings::save(&self.settings) {
            self.settings.favorites = previous;
            self.set_status(format!("{}: {}", self.tr("保存失败"), error));
        }
        cx.notify();
    }

    fn toggle_current_favorite(&mut self, cx: &mut Context<Self>) {
        if self.cur().computer_view {
            self.set_status(self.tr("此电脑视图不可收藏"));
            cx.notify();
            return;
        }
        self.toggle_favorite_path(self.cur().cwd.clone(), cx);
    }

    fn open_favorite(&mut self, path: String, cx: &mut Context<Self>) {
        self.navigate_to_directory(path, cx);
    }

    fn start_yazi(&mut self, cx: &mut Context<Self>) {
        let cwd = self.cur().cwd.clone();
        match YaziClient::spawn(Path::new(&cwd)) {
            Ok((client, mut rx)) => {
                self.client = Some(client);
                cx.spawn(async move |weak, cx| {
                    while let Some(event) = rx.recv().await {
                        weak.update(cx, |this, cx| {
                            this.on_event(event, cx);
                            cx.notify();
                        })
                        .ok();
                    }
                })
                .detach();
            }
            Err(e) => {
                eprintln!("[yazi-gui] failed to start yazi: {e}");
            }
        }
    }

    fn on_event(&mut self, event: YaziEvent, cx: &mut Context<Self>) {
        match event {
            YaziEvent::Cd { tab: yazi_tab, url } => {
                if !is_primary_yazi_tab(yazi_tab) {
                    return;
                }
                if !self.cur().computer_view {
                    self.cancel_input_state();
                    let previous_cwd = tree_path_key(&self.cur().cwd);
                    if let Some(u) = url {
                        self.cur_mut().cwd = u;
                    }
                    self.cur_mut().computer_view = false;
                    if tree_path_key(&self.cur().cwd) != previous_cwd {
                        self.cur_mut().invalidate_refresh();
                    }
                    let cwd = self.cur().cwd.clone();
                    self.sync_folder_tree_to_path(&cwd, cx);
                }
            }
            YaziEvent::Hover { tab: yazi_tab, .. } => {
                if !is_primary_yazi_tab(yazi_tab) {
                    return;
                }
            }
            YaziEvent::GuiFiles { cwd, files, .. } => {
                if !self.cur().computer_view
                    && tree_path_key(&self.cur().cwd) == tree_path_key(&cwd)
                {
                    let search_active = self
                        .cur()
                        .search
                        .as_ref()
                        .is_some_and(|search| !search.query.is_empty());
                    let refreshed = {
                        let tab = self.cur_mut();
                        let refreshed = refresh_request_matches(tab.refreshing, &tab.cwd, &cwd);
                        tab.cwd = cwd;
                        tab.files = files;
                        sort_files(&mut tab.files, tab.sort);
                        let names = tab
                            .files
                            .iter()
                            .map(|file| file.name.clone())
                            .collect::<Vec<_>>();
                        if refreshed {
                            tab.invalidate_refresh();
                        }
                        (refreshed, names)
                    };
                    if !search_active {
                        let tab = self.cur_mut();
                        reconcile_selection(&mut tab.selected, &mut tab.anchor, &refreshed.1);
                    }
                    if refreshed.0 {
                        self.set_status(format!(
                            "{}，{} {}",
                            self.tr("已刷新"),
                            self.cur().files.len(),
                            self.tr("项")
                        ));
                    }
                    self.reconcile_preview();
                    let current_cwd = self.cur().cwd.clone();
                    self.sync_folder_tree_to_path(&current_cwd, cx);
                }
            }
            _ => {}
        }
        let _ = cx;
    }

    fn reconcile_preview(&mut self) {
        let expected = {
            let tab = self.cur();
            if tab.selected.len() != 1 {
                None
            } else if tab.computer_view {
                Some(
                    PathBuf::from(&tab.selected[0])
                        .to_string_lossy()
                        .into_owned(),
                )
            } else {
                Some(
                    Path::new(&tab.cwd)
                        .join(&tab.selected[0])
                        .to_string_lossy()
                        .into_owned(),
                )
            }
        };
        let valid = expected.as_deref().is_some_and(|path| {
            self.cur().preview_path.as_deref() == Some(path) && Path::new(path).exists()
        });
        if !valid {
            self.cur_mut().clear_preview();
        }
    }

    // ---- 选中模型 ----

    fn select_single(&mut self, name: &str) {
        let tab = self.cur_mut();
        tab.selected = vec![name.to_string()];
        tab.anchor = Some(name.to_string());
    }

    fn toggle_select(&mut self, name: &str) {
        let tab = self.cur_mut();
        if let Some(pos) = tab.selected.iter().position(|s| s == name) {
            tab.selected.remove(pos);
        } else {
            tab.selected.push(name.to_string());
        }
        tab.anchor = Some(name.to_string());
    }

    fn range_select(&mut self, name: &str) {
        let visible_names: Vec<String> = self
            .visible_files()
            .iter()
            .map(|file| file.name.clone())
            .collect();
        let tab = self.cur_mut();
        let Some(end) = visible_names.iter().position(|item| item == name) else {
            return;
        };
        let start = tab
            .anchor
            .as_deref()
            .and_then(|anchor| visible_names.iter().position(|item| item == anchor))
            .unwrap_or(end);
        let (lo, hi) = if start <= end {
            (start, end)
        } else {
            (end, start)
        };
        tab.selected = visible_names[lo..=hi].to_vec();
    }

    fn clear_selection(&mut self) {
        let tab = self.cur_mut();
        tab.selected.clear();
        tab.anchor = None;
    }

    fn toggle_sort(&mut self, field: SortField, cx: &mut Context<Self>) {
        if self.is_inline_editing() {
            self.confirm_input(cx);
        }
        let sort = {
            let tab = self.cur_mut();
            if tab.sort.field == field {
                tab.sort.direction = match tab.sort.direction {
                    SortDirection::Ascending => SortDirection::Descending,
                    SortDirection::Descending => SortDirection::Ascending,
                };
            } else {
                tab.sort = SortState {
                    field,
                    direction: SortDirection::Ascending,
                };
            }
            tab.sort
        };
        if let Some(search) = self
            .cur_mut()
            .search
            .as_mut()
            .filter(|search| !search.query.is_empty())
        {
            sort_files(&mut search.results, sort);
        } else {
            sort_files(&mut self.cur_mut().files, sort);
        }
        cx.notify();
    }

    /// 单击/双击文件行的统一入口。
    fn click_file(
        &mut self,
        name: &str,
        is_dir: bool,
        modifiers: Modifiers,
        click_count: usize,
        cx: &mut Context<Self>,
    ) {
        if self.is_inline_editing() {
            self.confirm_input(cx);
        } else if !matches!(self.pending, Some(PendingOp::Search)) {
            self.cancel_input_state();
        }
        self.clear_status();
        if click_count >= 2 {
            if is_dir {
                self.enter(name);
            } else {
                self.open_path(name, cx);
            }
            return;
        }
        if modifiers.control {
            self.toggle_select(name);
        } else if modifiers.shift {
            self.range_select(name);
        } else {
            self.select_single(name);
        }
        self.update_preview_for_selection(cx);
        cx.notify();
    }

    /// 点击空白处：取消选中。
    fn click_blank(&mut self, cx: &mut Context<Self>) {
        if self.is_inline_editing() {
            self.confirm_input(cx);
            return;
        }
        if !matches!(self.pending, Some(PendingOp::Search)) {
            self.cancel_input_state();
        }
        self.clear_status();
        self.clear_selection();
        self.cur_mut().clear_preview();
        cx.notify();
    }

    /// 根据当前选中态刷新预览。
    fn update_preview_for_selection(&mut self, cx: &mut Context<Self>) {
        let (cwd, name, is_dir, size, count) = {
            let tab = self.cur();
            let count = tab.selected.len();
            if count == 1 {
                let name = tab.selected[0].clone();
                let (is_dir, size) = self
                    .visible_files()
                    .iter()
                    .find(|f| f.name == name)
                    .map(|f| (f.is_dir, f.size))
                    .unwrap_or((false, 0));
                (tab.cwd.clone(), name, is_dir, size, 1usize)
            } else {
                (tab.cwd.clone(), String::new(), false, 0u64, count)
            }
        };

        if count == 1 {
            let full = if self.cur().computer_view {
                PathBuf::from(&name).to_string_lossy().into_owned()
            } else {
                Path::new(&cwd).join(&name).to_string_lossy().into_owned()
            };
            self.cur_mut().preview_path = Some(full.clone());
            self.load_preview(full, is_dir, size, cx);
        } else {
            self.cur_mut().clear_preview();
        }
    }

    fn load_preview(&mut self, path: String, is_dir: bool, size: u64, cx: &mut Context<Self>) {
        let tab_index = self.active;
        if is_dir {
            self.cur_mut().preview = Preview::Dir;
            return;
        }
        if is_image_file(&path) {
            self.cur_mut().preview = Preview::Image { path };
            return;
        }
        if size > 1_000_000 {
            self.cur_mut().preview = Preview::Binary { size };
            return;
        }
        self.cur_mut().preview = Preview::Loading;
        let syntax_theme = self.theme.syntax_theme;

        cx.spawn(async move |weak, cx| {
            let path2 = path.clone();
            let result = cx
                .background_executor()
                .spawn(async move {
                    let bytes = std::fs::read(&path2).ok()?;
                    if !is_probably_text(&bytes) {
                        return None;
                    }
                    let text = String::from_utf8_lossy(&bytes).into_owned();
                    let truncated = truncate_preview(&text);
                    let ext = extension_of(&path2);
                    let highlights = highlight_code(&truncated, &ext, syntax_theme);
                    Some((truncated, highlights))
                })
                .await;

            weak.update(cx, |this, cx| {
                let valid = this
                    .tabs
                    .get(tab_index)
                    .is_some_and(|tab| tab.preview_path.as_deref() == Some(path.as_str()));
                if valid {
                    this.tabs[tab_index].preview = match result {
                        Some((text, Some(highlights))) => Preview::Code { text, highlights },
                        Some((text, None)) => Preview::Text(text),
                        None => Preview::Binary { size },
                    };
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    // ---- 基础导航与操作 ----

    fn send_checked(&self, action: &[&str]) -> anyhow::Result<()> {
        let client = self
            .client
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("yazi client unavailable"))?;
        client.send(action).map(|_| ())
    }

    fn send(&self, action: &[&str]) {
        let _ = self.send_checked(action);
    }

    fn refresh_current(&mut self, cx: &mut Context<Self>) {
        self.cancel_input_state();
        self.menu = None;
        if self.pending_delete.is_some() {
            return;
        }
        if self.transfer.is_some() {
            self.set_status("粘贴进行中，暂不能刷新");
            cx.notify();
            return;
        }
        if self.delete_in_progress {
            self.set_status("删除进行中，暂不能刷新");
            cx.notify();
            return;
        }
        self.clear_status();
        self.refresh_folder_tree(cx);

        if self.cur().computer_view {
            if self.cur().refreshing {
                self.set_status(self.tr("正在刷新..."));
                cx.notify();
                return;
            }
            self.start_drive_scan(cx, true);
            cx.notify();
            return;
        }

        if self.cur().refreshing {
            self.set_status(self.tr("正在刷新..."));
            cx.notify();
            return;
        }
        let tab_index = self.active;
        let cwd = self.cur().cwd.clone();
        let Some(client_id) = self
            .client
            .as_ref()
            .map(|client| client.client_id().to_string())
        else {
            self.set_status(self.tr("刷新失败: yazi client unavailable"));
            cx.notify();
            return;
        };
        let request_id = {
            let tab = self.cur_mut();
            tab.refresh_request_id = tab.refresh_request_id.wrapping_add(1);
            tab.refreshing = true;
            tab.refresh_request_id
        };
        self.set_status(self.tr("正在刷新..."));
        cx.spawn(async move |weak, cx| {
            let command_cwd = cwd.clone();
            let result = cx
                .background_executor()
                .spawn(async move {
                    YaziClient::send_with_client_id(&client_id, &["cd".to_string(), command_cwd])
                })
                .await;
            weak.update(cx, |this, cx| {
                if tab_index >= this.tabs.len()
                    || !refresh_token_matches(
                        this.tabs[tab_index].refreshing,
                        this.tabs[tab_index].refresh_request_id,
                        request_id,
                    )
                    || tree_path_key(&this.tabs[tab_index].cwd) != tree_path_key(&cwd)
                {
                    return;
                }
                this.tabs[tab_index].refreshing = false;
                if this.active == tab_index {
                    match result {
                        Ok(_) => this.set_status(this.tr("已刷新")),
                        Err(error) => {
                            this.set_status(format!("{}: {}", this.tr("刷新失败"), error))
                        }
                    }
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    fn enter(&mut self, name: &str) {
        self.cancel_input_state();
        self.clear_status();
        if self.cur().computer_view {
            // 点击盘符：进入该磁盘根目录
            self.cur_mut().computer_view = false;
            self.cur_mut().invalidate_refresh();
            self.cur_mut().cwd = name.to_string();
            self.cur_mut().selected.clear();
            self.cur_mut().anchor = None;
            self.cur_mut().clear_preview();
            self.send(&["cd", name]);
            return;
        }
        self.cur_mut().invalidate_refresh();
        let path = std::path::Path::new(&self.cur().cwd).join(name);
        self.send(&["cd", &path.to_string_lossy()]);
    }

    fn show_computer_view(&mut self, cx: &mut Context<Self>) {
        self.cancel_input_state();
        self.clear_status();
        self.menu = None;
        self.cur_mut().computer_view = true;
        self.cur_mut().selected.clear();
        self.cur_mut().anchor = None;
        self.cur_mut().clear_preview();
        self.start_drive_scan(cx, true);
        cx.notify();
    }

    fn go_parent(&mut self, cx: &mut Context<Self>) {
        self.cancel_input_state();
        self.clear_status();
        if self.cur().computer_view {
            return; // 已在「此电脑」顶层
        }
        if is_drive_root(&self.cur().cwd) {
            // 从盘符根向上 → 进入「此电脑」虚拟视图
            self.cur_mut().computer_view = true;
            self.cur_mut().selected.clear();
            self.cur_mut().anchor = None;
            self.cur_mut().clear_preview();
            self.start_drive_scan(cx, true);
            cx.notify();
            return;
        }
        self.cur_mut().invalidate_refresh();
        self.send(&["cd", ".."]);
    }

    fn open_path(&self, name: &str, cx: &mut Context<Self>) {
        let path = std::path::Path::new(&self.cur().cwd)
            .join(name)
            .to_string_lossy()
            .into_owned();
        cx.spawn(async move |_weak, cx| {
            cx.background_executor()
                .spawn(async move {
                    let _ = open::that(&path);
                })
                .await;
        })
        .detach();
    }

    fn open_selected(&mut self, cx: &mut Context<Self>) {
        self.cancel_input_state();
        self.clear_status();
        let tab = self.cur();
        let Some(name) = tab.selected.first().cloned() else {
            self.set_status("未选择项目");
            cx.notify();
            return;
        };
        if tab.computer_view {
            self.enter(&name);
            return;
        }
        let is_dir = tab.files.iter().any(|f| f.name == name && f.is_dir);
        if is_dir {
            self.enter(&name);
        } else {
            self.open_path(&name, cx);
        }
    }

    fn delete_selected(&mut self, cx: &mut Context<Self>) {
        self.cancel_input_state();
        self.clear_status();
        self.menu = None;
        if self.pending_delete.is_some() {
            return;
        }
        if self.transfer.is_some() {
            self.set_status("粘贴进行中，暂不能删除");
            cx.notify();
            return;
        }
        if self.delete_in_progress {
            self.set_status("删除进行中");
            cx.notify();
            return;
        }
        if self.cur().computer_view {
            self.set_status("此电脑视图不可删除");
            cx.notify();
            return;
        }
        let cwd = self.cur().cwd.clone();
        let paths: Vec<String> = self
            .cur()
            .selected
            .iter()
            .map(|n| {
                std::path::Path::new(&cwd)
                    .join(n)
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        if paths.is_empty() {
            self.set_status("未选择项目");
            cx.notify();
            return;
        }

        let network_count = match network_path_count(&paths) {
            Ok(count) => count,
            Err(message) => {
                self.set_status(message);
                cx.notify();
                return;
            }
        };
        if network_count > 0 {
            self.pending_delete = Some(PendingDelete {
                cwd,
                paths,
                network_count,
            });
            cx.notify();
            return;
        }
        self.start_delete_operation(cwd, paths, cx);
    }

    fn start_delete_operation(&mut self, cwd: String, paths: Vec<String>, cx: &mut Context<Self>) {
        let count = paths.len();
        self.delete_in_progress = true;
        self.set_status(format!("正在删除 {} 项...", count));
        cx.notify();
        cx.spawn(async move |weak, cx| {
            let summary = cx
                .background_executor()
                .spawn(async move { delete_paths_with_policy(&paths) })
                .await;
            weak.update(cx, |this, cx| {
                this.delete_in_progress = false;
                if summary.success > 0 && this.cur().cwd == cwd && !this.cur().computer_view {
                    this.send(&["cd", cwd.as_str()]);
                }
                this.set_status(delete_status(&summary));
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn confirm_delete(&mut self, cx: &mut Context<Self>) {
        let Some(request) = self.pending_delete.take() else {
            return;
        };
        self.start_delete_operation(request.cwd, request.paths, cx);
    }

    fn cancel_delete_confirmation(&mut self, cx: &mut Context<Self>) {
        if self.pending_delete.take().is_some() {
            self.set_status("已取消删除");
            cx.notify();
        }
    }

    fn selected_paths(&self) -> Vec<String> {
        let cwd = self.cur().cwd.clone();
        self.cur()
            .selected
            .iter()
            .map(|n| {
                std::path::Path::new(&cwd)
                    .join(n)
                    .to_string_lossy()
                    .into_owned()
            })
            .collect()
    }

    fn copy_selected(&mut self, cx: &mut Context<Self>) {
        self.cancel_input_state();
        self.clear_status();
        if self.transfer.is_some() {
            self.set_status("粘贴进行中，暂不能修改文件剪贴板");
            cx.notify();
            return;
        }
        if self.cur().computer_view {
            self.set_status("此电脑视图不可复制");
            cx.notify();
            return;
        }
        let paths = self.selected_paths();
        if paths.is_empty() {
            self.set_status("未选择项目");
            cx.notify();
            return;
        }
        let count = paths.len();
        self.clipboard = Some(Clipboard { paths, cut: false });
        self.set_status(format!("已复制 {} 项", count));
        cx.notify();
    }

    fn cut_selected(&mut self, cx: &mut Context<Self>) {
        self.cancel_input_state();
        self.clear_status();
        if self.transfer.is_some() {
            self.set_status("粘贴进行中，暂不能修改文件剪贴板");
            cx.notify();
            return;
        }
        if self.cur().computer_view {
            self.set_status("此电脑视图不可剪切");
            cx.notify();
            return;
        }
        let paths = self.selected_paths();
        if paths.is_empty() {
            self.set_status("未选择项目");
            cx.notify();
            return;
        }
        let count = paths.len();
        self.clipboard = Some(Clipboard { paths, cut: true });
        self.set_status(format!("已剪切 {} 项", count));
        cx.notify();
    }

    fn paste_clipboard(&mut self, cx: &mut Context<Self>) {
        self.cancel_input_state();
        self.clear_status();
        if self.transfer.is_some() {
            self.set_status("已有粘贴操作正在进行");
            cx.notify();
            return;
        }
        if self.cur().computer_view {
            self.set_status("此电脑视图不可粘贴");
            cx.notify();
            return;
        }
        let Some(clip) = &self.clipboard else {
            self.set_status("文件剪贴板为空");
            cx.notify();
            return;
        };
        let paths = clip.paths.clone();
        let cut = clip.cut;
        let count = paths.len();

        if cut {
            let dest = self.cur().cwd.clone();
            self.set_status(format!("正在粘贴 {} 项...", count));
            cx.notify();
            cx.spawn(async move |weak, cx| {
                let destination = dest.clone();
                let (success, failed) = cx
                    .background_executor()
                    .spawn(async move { move_paths(&paths, &destination) })
                    .await;
                weak.update(cx, |this, cx| {
                    if failed == 0 {
                        this.clipboard = None;
                    }
                    if success > 0 && this.cur().cwd == dest {
                        this.send(&["cd", dest.as_str()]);
                    }
                    this.set_status(operation_status("粘贴", success, failed));
                    cx.notify();
                })
                .ok();
            })
            .detach();
            return;
        }

        let total_items = paths.len();
        let cancel = Arc::new(AtomicBool::new(false));
        let (tx, rx) = unbounded_channel::<TransferUpdate>();
        let refresh_cwd = self.cur().cwd.clone();
        self.transfer = Some(TransferState {
            progress: TransferProgress {
                completed_bytes: 0,
                total_bytes: 0,
                completed_items: 0,
                total_items,
                current: "准备复制".to_string(),
            },
            cancel: cancel.clone(),
        });
        self.set_status(format!("正在粘贴 {} 项...", count));
        cx.notify();
        cx.spawn(async move |weak, cx| {
            let tx2 = tx.clone();
            let worker_dest = refresh_cwd.clone();
            let worker = cx.background_executor().spawn(async move {
                let outcome = copy_paths_with_progress(&paths, &worker_dest, &cancel, &tx2);
                let _ = tx2.send(TransferUpdate::Finished(outcome));
            });
            let mut rx: UnboundedReceiver<TransferUpdate> = rx;
            while let Some(update) = rx.recv().await {
                let finished = matches!(&update, TransferUpdate::Finished(_));
                weak.update(cx, |this, cx| {
                    match update {
                        TransferUpdate::Progress {
                            total_bytes,
                            completed_bytes,
                            completed_items,
                            current,
                        } => {
                            if let Some(transfer) = &mut this.transfer {
                                transfer.progress.total_bytes = total_bytes;
                                transfer.progress.completed_bytes = completed_bytes;
                                transfer.progress.completed_items = completed_items;
                                transfer.progress.current = current;
                            }
                        }
                        TransferUpdate::Finished(outcome) => {
                            this.finish_transfer(outcome, &refresh_cwd, cx);
                        }
                    }
                    cx.notify();
                })
                .ok();
                if finished {
                    break;
                }
            }
            let _ = worker.await;
        })
        .detach();
    }

    fn cancel_transfer(&mut self, cx: &mut Context<Self>) {
        if let Some(transfer) = &self.transfer {
            transfer.cancel.store(true, AtomicOrdering::Relaxed);
            self.set_status("正在取消粘贴...");
            cx.notify();
        }
    }

    fn finish_transfer(&mut self, outcome: TransferOutcome, dest: &str, cx: &mut Context<Self>) {
        self.transfer = None;
        match outcome {
            TransferOutcome::Completed { success } => {
                if success > 0 && self.cur().cwd == dest {
                    self.send(&["cd", dest]);
                }
                self.set_status(format!("粘贴完成 {} 项", success));
            }
            TransferOutcome::Cancelled { success } => {
                self.clipboard = None;
                if success > 0 && self.cur().cwd == dest {
                    self.send(&["cd", dest]);
                }
                self.set_status(format!("粘贴已取消，完成 {} 项", success));
            }
            TransferOutcome::Failed { success, message } => {
                if success > 0 && self.cur().cwd == dest {
                    self.send(&["cd", dest]);
                }
                self.set_status(format!("粘贴失败，已完成 {} 项：{}", success, message));
            }
        }
        cx.notify();
    }

    fn open_menu(
        &mut self,
        target: Option<String>,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        self.cancel_input_state();
        self.clear_status();
        // 右键命中的文件若不在选中集里，先单选它（Windows 习惯）。
        if let Some(t) = &target {
            if !self.cur().selected.iter().any(|s| s == t) {
                self.select_single(t);
            }
        }
        self.menu = Some(MenuState { target, position });
        cx.notify();
    }

    fn close_menu(&mut self, cx: &mut Context<Self>) {
        self.menu = None;
        cx.notify();
    }

    fn exec_menu_action(
        &mut self,
        action: MenuAction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let target = self.menu.as_ref().and_then(|m| m.target.clone());
        match action {
            MenuAction::Open => {
                if let Some(name) = &target {
                    let is_dir = self
                        .visible_files()
                        .iter()
                        .any(|file| &file.name == name && file.is_dir);
                    if is_dir {
                        self.enter(name);
                    } else {
                        self.open_path(name, cx);
                    }
                } else {
                    self.open_selected(cx);
                }
            }
            MenuAction::Favorite => {
                if let Some(name) = target {
                    let path = if self.cur().computer_view {
                        name
                    } else {
                        Path::new(&self.cur().cwd)
                            .join(name)
                            .to_string_lossy()
                            .into_owned()
                    };
                    self.toggle_favorite_path(path, cx);
                }
            }
            MenuAction::Rename => self.start_rename(window, cx),
            MenuAction::Delete => self.delete_selected(cx),
            MenuAction::Copy => self.copy_selected(cx),
            MenuAction::Cut => self.cut_selected(cx),
            MenuAction::Paste => self.paste_clipboard(cx),
            MenuAction::NewFile => self.start_new_file(window, cx),
            MenuAction::NewDir => self.start_new_dir(window, cx),
        }
        self.menu = None;
        cx.notify();
    }

    fn toggle_theme(&mut self, cx: &mut Context<Self>) {
        self.cancel_input_state();
        self.clear_status();
        let theme = match self.settings.theme {
            ThemeMode::Dark => ThemeMode::Light,
            ThemeMode::Light => ThemeMode::Dark,
        };
        self.set_theme_mode(theme, cx);
    }

    fn toggle_shortcuts(&mut self, cx: &mut Context<Self>) {
        self.shortcuts_expanded = !self.shortcuts_expanded;
        cx.notify();
    }

    fn cycle_language(&mut self, cx: &mut Context<Self>) {
        self.clear_status();
        self.settings.language = match self.settings.language {
            Language::System => Language::Chinese,
            Language::Chinese => Language::English,
            Language::English => Language::System,
        };
        if let Some(tray) = &self.tray {
            tray.set_language(self.settings.language);
        }
        self.persist_settings(cx);
    }

    fn set_autostart(&mut self, enabled: bool, cx: &mut Context<Self>) {
        if let Err(error) = settings::set_autostart(enabled) {
            self.set_status(format!("{}: {}", self.tr("保存失败"), error));
            cx.notify();
            return;
        }
        self.settings.autostart = enabled;
        self.persist_settings(cx);
    }

    fn start_shortcut_edit(
        &mut self,
        action: ShortcutAction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.cancel_input_state();
        self.pending = Some(PendingOp::EditShortcut(action));
        self.clear_status();
        cx.focus_self(window);
        cx.notify();
    }

    fn record_shortcut(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {
        let Some(PendingOp::EditShortcut(action)) = self.pending else {
            return;
        };
        let keystroke = &event.keystroke;
        match keystroke.key.as_str() {
            "escape" => {
                self.cancel_input(cx);
                return;
            }
            "backspace" => {
                action.set_shortcut(&mut self.settings.shortcuts, String::new());
                self.pending = None;
                self.persist_settings(cx);
                self.set_status(format!("{}已清除", self.tr(action.label())));
                cx.notify();
                return;
            }
            "shift" | "control" | "ctrl" | "alt" | "platform" | "super" | "win" | "function"
            | "fn" => {
                self.set_status(self.tr("按下快捷键..."));
                cx.notify();
                return;
            }
            _ => {}
        }

        let Some(shortcut) = keystroke_to_shortcut(keystroke) else {
            return;
        };
        if let Some(conflict) = shortcut_actions().into_iter().find(|candidate| {
            *candidate != action
                && normalize_shortcut(candidate.shortcut(&self.settings.shortcuts)) == shortcut
        }) {
            self.set_status(format!(
                "{}：{} {}",
                self.tr("快捷键冲突"),
                self.tr(conflict.label()),
                self.tr("该快捷键已被占用")
            ));
            cx.notify();
            return;
        }
        action.set_shortcut(&mut self.settings.shortcuts, shortcut);
        self.pending = None;
        self.persist_settings(cx);
        self.set_status(format!("{}已更新", self.tr(action.label())));
        cx.notify();
    }

    // ---- 多标签 ----

    fn switch_tab(&mut self, i: usize, cx: &mut Context<Self>) {
        self.cancel_input_state();
        self.clear_status();
        if i == self.active || i >= self.tabs.len() {
            return;
        }
        self.tabs[self.active].invalidate_refresh();
        self.active = i;
        self.cur_mut().clear_preview();
        if self.tabs[i].computer_view {
            self.start_drive_scan(cx, true);
        } else {
            let cwd = self.tabs[i].cwd.clone();
            self.send(&["cd", cwd.as_str()]);
            self.sync_folder_tree_to_path(&cwd, cx);
        }
        cx.notify();
    }

    fn new_tab(&mut self, cx: &mut Context<Self>) {
        self.cancel_input_state();
        self.clear_status();
        let cwd = self.cur().cwd.clone();
        self.cur_mut().invalidate_refresh();
        self.tabs.push(Tab::new(&cwd));
        self.active = self.tabs.len() - 1;
        self.cur_mut().clear_preview();
        self.send(&["cd", cwd.as_str()]);
        self.sync_folder_tree_to_path(&cwd, cx);
        cx.notify();
    }

    fn close_tab(&mut self, i: usize, cx: &mut Context<Self>) {
        self.cancel_input_state();
        self.clear_status();
        if i >= self.tabs.len() {
            return;
        }
        if self.tabs.len() == 1 {
            cx.quit();
            return;
        }
        self.tabs.remove(i);
        let len = self.tabs.len();
        if self.active > i {
            self.active -= 1;
        }
        if self.active >= len {
            self.active = len - 1;
        }
        self.cur_mut().clear_preview();
        let cwd = self.tabs[self.active].cwd.clone();
        self.send(&["cd", cwd.as_str()]);
        self.sync_folder_tree_to_path(&cwd, cx);
        cx.notify();
    }

    // ---- 输入模式（地址栏 / 重命名 / 新建） ----

    fn start_rename(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.cancel_input_state();
        if self.cur().computer_view {
            self.set_status("此电脑视图不可重命名");
            cx.notify();
            return;
        }
        let Some(name) = self.cur().selected.first().cloned() else {
            self.set_status("未选择项目");
            cx.notify();
            return;
        };
        let path = std::path::Path::new(&self.cur().cwd)
            .join(&name)
            .to_string_lossy()
            .into_owned();
        let is_dir = self
            .visible_files()
            .iter()
            .find(|file| file.name == name)
            .is_some_and(|file| file.is_dir);
        self.reset_input_editor(name.clone(), false);
        let file_name_len = file_name_of(&name).len();
        let file_name_start = name.len().saturating_sub(file_name_len);
        let selection = if is_dir {
            file_name_start..name.len()
        } else {
            let stem_len = std::path::Path::new(&name)
                .file_name()
                .and_then(|file| file.to_str())
                .and_then(|file| std::path::Path::new(file).file_stem())
                .and_then(|stem| stem.to_str())
                .map(str::len)
                .unwrap_or(file_name_len);
            file_name_start..file_name_start + stem_len
        };
        self.input_selection = selection;
        self.pending = Some(PendingOp::Rename { path });
        self.clear_status();
        self.focus_handle.focus(window);
        cx.notify();
    }

    fn start_new_file(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.cancel_input_state();
        self.reset_input_editor(String::new(), false);
        self.pending = Some(PendingOp::NewFile);
        self.clear_status();
        self.focus_handle.focus(window);
        cx.notify();
    }

    fn start_new_dir(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.cancel_input_state();
        self.reset_input_editor(String::new(), false);
        self.pending = Some(PendingOp::NewDir);
        self.clear_status();
        self.focus_handle.focus(window);
        cx.notify();
    }

    fn start_goto(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let cwd = self.cur().cwd.clone();
        self.cancel_input_state();
        self.reset_input_editor(cwd, true);
        self.pending = Some(PendingOp::GoToPath);
        self.clear_status();
        self.focus_handle.focus(window);
        cx.notify();
    }

    fn start_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.page != Page::Files {
            return;
        }
        self.cancel_input_state();
        if self.cur().computer_view {
            self.set_status(self.tr("搜索仅支持真实目录"));
            cx.notify();
            return;
        }
        let cwd = self.cur().cwd.clone();
        self.reset_input_editor(String::new(), false);
        self.pending = Some(PendingOp::Search);
        {
            let tab = self.cur_mut();
            tab.search_generation = tab.search_generation.wrapping_add(1);
            let generation = tab.search_generation;
            tab.search = Some(SearchState {
                cwd,
                query: String::new(),
                results: Vec::new(),
                generation,
                scanning: false,
            });
        };
        self.clear_selection();
        self.cur_mut().clear_preview();
        self.menu = None;
        self.clear_status();
        cx.focus_self(window);
        cx.notify();
    }

    fn update_search_query(&mut self, cx: &mut Context<Self>) {
        if self.cur().search.is_none() {
            return;
        }
        let query = self.input.clone();
        let is_empty = query.is_empty();
        {
            let tab = self.cur_mut();
            tab.search_generation = tab.search_generation.wrapping_add(1);
            let generation = tab.search_generation;
            if let Some(search) = tab.search.as_mut() {
                search.generation = generation;
                search.query = query;
                search.results.clear();
                search.scanning = !is_empty;
            }
        }
        self.clear_selection();
        self.cur_mut().clear_preview();
        if is_empty {
            self.clear_status();
        } else {
            self.set_status(self.tr("搜索中..."));
            self.start_search_scan(cx);
        }
    }

    fn start_search_scan(&mut self, cx: &mut Context<Self>) {
        let tab_index = self.active;
        let Some(search) = &self.tabs[tab_index].search else {
            return;
        };
        let cwd = search.cwd.clone();
        let query = search.query.clone();
        let generation = search.generation;
        cx.spawn(async move |weak, cx| {
            let scan_cwd = cwd.clone();
            let result = cx
                .background_executor()
                .spawn(async move { search_directory(Path::new(&scan_cwd), &query) })
                .await;
            weak.update(cx, |this, cx| {
                let valid = this.tabs.get(tab_index).is_some_and(|tab| {
                    tab.search
                        .as_ref()
                        .is_some_and(|search| search.generation == generation && search.cwd == cwd)
                });
                if !valid {
                    return;
                }
                match result {
                    Ok(mut results) => {
                        sort_files(&mut results, this.tabs[tab_index].sort);
                        let names = results
                            .iter()
                            .map(|file| file.name.clone())
                            .collect::<Vec<_>>();
                        if let Some(search) = this.tabs[tab_index].search.as_mut() {
                            search.results = results;
                            search.scanning = false;
                        }
                        let tab = &mut this.tabs[tab_index];
                        reconcile_selection(&mut tab.selected, &mut tab.anchor, &names);
                        if this.active == tab_index {
                            this.clear_status();
                            this.reconcile_preview();
                        }
                    }
                    Err(error) => {
                        if let Some(search) = this.tabs[tab_index].search.as_mut() {
                            search.results.clear();
                            search.scanning = false;
                        }
                        if this.active == tab_index {
                            this.set_status(format!("{}: {}", this.tr("搜索失败"), error));
                        }
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn cancel_input(&mut self, cx: &mut Context<Self>) {
        self.cancel_input_state();
        cx.notify();
    }

    fn confirm_input(&mut self, cx: &mut Context<Self>) {
        if matches!(self.pending.as_ref(), Some(PendingOp::GoToPath)) {
            self.confirm_goto(cx);
            return;
        }
        let Some(pending) = self.pending.clone() else {
            return;
        };
        if matches!(pending, PendingOp::Search | PendingOp::EditShortcut(_)) {
            return;
        }
        let name = self.input.trim().to_string();
        if name.is_empty() {
            self.set_status("名称不能为空");
            cx.notify();
            return;
        }

        let operation = match &pending {
            PendingOp::Rename { .. } => "重命名",
            PendingOp::NewFile => "新建文件",
            PendingOp::NewDir => "新建文件夹",
            PendingOp::GoToPath | PendingOp::Search | PendingOp::EditShortcut(_) => return,
        }
        .to_string();
        let cwd = self.cur().cwd.clone();
        let name_for_status = name.clone();
        self.set_status(format!("正在{} {}...", operation, name_for_status));
        cx.notify();
        cx.spawn(async move |weak, cx| {
            let cwd2 = cwd.clone();
            let name2 = name.clone();
            let result = cx
                .background_executor()
                .spawn(async move {
                    match &pending {
                        PendingOp::Rename { path } => {
                            rename_path(Path::new(path), Path::new(&cwd2), &name2)
                                .map_err(|error| error.to_string())
                        }
                        PendingOp::NewFile => {
                            create_file(Path::new(&cwd2), &name2).map_err(|error| error.to_string())
                        }
                        PendingOp::NewDir => create_directory(Path::new(&cwd2), &name2)
                            .map_err(|error| error.to_string()),
                        PendingOp::GoToPath | PendingOp::Search | PendingOp::EditShortcut(_) => {
                            unreachable!()
                        }
                    }
                })
                .await;
            let succeeded = result.is_ok();
            weak.update(cx, |this, cx| {
                match result {
                    Ok(()) => {
                        this.cancel_input_state();
                        this.set_status(format!("{}完成", operation));
                    }
                    Err(error) => {
                        this.set_status(format!("{}失败: {}", operation, error));
                    }
                }
                if succeeded {
                    this.send(&["cd", cwd.as_str()]);
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn confirm_goto(&mut self, cx: &mut Context<Self>) {
        let cwd = self.cur().cwd.clone();
        let target = match resolve_address_path(&cwd, &self.input) {
            Ok(target) if std::path::Path::new(&target).is_dir() => target,
            Ok(_) => {
                self.set_status("目标不是有效目录");
                cx.notify();
                return;
            }
            Err(message) => {
                self.set_status(message);
                cx.notify();
                return;
            }
        };

        self.navigate_to_directory(target, cx);
    }

    fn on_input_key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if matches!(self.pending, Some(PendingOp::EditShortcut(_))) {
            self.record_shortcut(event, cx);
        } else if matches!(self.pending, Some(PendingOp::Search)) {
            if event.keystroke.key.as_str() == "escape" {
                self.cancel_input(cx);
            }
        } else if self.pending.is_some() {
            match event.keystroke.key.as_str() {
                "enter" => self.confirm_input(cx),
                "escape" => self.cancel_input(cx),
                _ => {}
            }
        } else {
            self.handle_shortcut(event, window, cx);
        }
    }

    fn handle_shortcut(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let ks = &event.keystroke;
        let ctrl = ks.modifiers.control;
        if self.pending_delete.is_some() {
            if ks.key.as_str() == "escape" {
                self.cancel_delete_confirmation(cx);
            }
            return;
        }
        if shortcut_matches(&self.settings.shortcuts.new_tab, ks) {
            self.new_tab(cx);
        } else if shortcut_matches(&self.settings.shortcuts.close_tab, ks) {
            self.close_tab(self.active, cx);
        } else if shortcut_matches(&self.settings.shortcuts.open, ks) {
            self.open_selected(cx);
        } else if shortcut_matches(&self.settings.shortcuts.search, ks) {
            self.start_search(window, cx);
        } else if shortcut_matches(&self.settings.shortcuts.copy, ks) {
            self.copy_selected(cx);
        } else if shortcut_matches(&self.settings.shortcuts.cut, ks) {
            self.cut_selected(cx);
        } else if shortcut_matches(&self.settings.shortcuts.paste, ks) {
            self.paste_clipboard(cx);
        } else if shortcut_matches(&self.settings.shortcuts.delete, ks) {
            self.delete_selected(cx);
        } else if shortcut_matches(&self.settings.shortcuts.rename, ks) {
            self.start_rename(window, cx);
        } else if shortcut_matches(&self.settings.shortcuts.new_file, ks) {
            self.start_new_file(window, cx);
        } else if shortcut_matches(&self.settings.shortcuts.new_dir, ks) {
            self.start_new_dir(window, cx);
        } else if ks.key.as_str() == "a" && ctrl {
            let names: Vec<String> = self.cur().files.iter().map(|f| f.name.clone()).collect();
            self.cur_mut().selected = names;
            self.cur_mut().clear_preview();
            cx.notify();
        } else if ks.key.as_str() == "backspace" {
            self.go_parent(cx);
        } else if ks.key.as_str() == "escape" {
            if self.menu.is_some() {
                self.close_menu(cx);
            } else if !self.cur().selected.is_empty() {
                self.click_blank(cx);
            }
        }
    }

    fn input_field(
        &self,
        cx: &mut Context<Self>,
        placeholder: impl Into<SharedString>,
    ) -> AnyElement {
        self.input_field_with_activation(cx, placeholder.into(), false)
    }

    fn input_field_with_activation(
        &self,
        cx: &mut Context<Self>,
        placeholder: SharedString,
        activate_search: bool,
    ) -> AnyElement {
        let mut field = div()
            .flex()
            .flex_1()
            .key_context("YaziInput")
            .track_focus(&self.focus_handle)
            .cursor(CursorStyle::IBeam)
            .on_action(cx.listener(Self::input_backspace))
            .on_action(cx.listener(Self::input_delete))
            .on_action(cx.listener(Self::input_left))
            .on_action(cx.listener(Self::input_right))
            .on_action(cx.listener(Self::input_select_left))
            .on_action(cx.listener(Self::input_select_right))
            .on_action(cx.listener(Self::input_select_all))
            .on_action(cx.listener(Self::input_home))
            .on_action(cx.listener(Self::input_end))
            .on_action(cx.listener(Self::input_paste))
            .on_action(cx.listener(Self::input_copy))
            .on_action(cx.listener(Self::input_cut))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::input_mouse_down))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::input_mouse_up))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::input_mouse_up))
            .on_mouse_move(cx.listener(Self::input_mouse_move))
            .id("input-field");
        if activate_search {
            field = field.on_click(cx.listener(|this, _event, window, cx| {
                if !matches!(this.pending, Some(PendingOp::Search)) {
                    this.start_search(window, cx);
                }
                cx.stop_propagation();
            }));
        } else {
            field = field.on_click(cx.listener(|_this, _event, _window, cx| {
                cx.stop_propagation();
            }));
        }
        field
            .child(InputElement {
                root: cx.entity(),
                placeholder,
            })
            .into_any_element()
    }

    fn file_search_field(&self, cx: &mut Context<Root>) -> AnyElement {
        let theme = self.theme;
        let active = matches!(self.pending, None | Some(PendingOp::Search));
        let mut field = div()
            .w(px(260.0))
            .h(px(28.0))
            .px_2()
            .bg(theme.surface0)
            .border_1()
            .border_color(theme.border)
            .rounded_sm()
            .id("file-search-field");
        if active {
            field = field.child(self.input_field_with_activation(
                cx,
                self.tr("搜索文件...").into(),
                true,
            ));
        } else {
            field = field.child(
                div()
                    .flex_1()
                    .text_sm()
                    .text_color(theme.muted)
                    .child(self.tr("搜索文件...")),
            );
        }
        field.into_any_element()
    }

    fn input_bar(&self, cx: &mut Context<Self>) -> AnyElement {
        let _ = cx;
        div().into_any_element()
    }

    fn folder_tree_pane(&self, cx: &mut Context<Root>) -> AnyElement {
        let theme = self.theme;
        let computer_view = self.cur().computer_view;
        let rows = self.folder_tree.visible_rows(&self.drive_roots);
        let items = uniform_list(
            "folder-tree-items",
            rows.len(),
            cx.processor(move |this, range: Range<usize>, _window, list_cx| {
                range
                    .filter_map(|index| rows.get(index).cloned())
                    .map(|row| this.folder_tree_row(list_cx, row))
                    .collect()
            }),
        )
        .h_full();

        div()
            .w(px(240.0))
            .h_full()
            .flex_shrink_0()
            .flex()
            .flex_col()
            .bg(theme.mantle)
            .id("folder-tree")
            .child(
                div()
                    .w_full()
                    .px_2()
                    .py_1()
                    .bg(if computer_view {
                        theme.surface0
                    } else {
                        theme.mantle
                    })
                    .cursor_pointer()
                    .id("tree-computer")
                    .on_click(cx.listener(|this, _event, _window, cx| {
                        this.show_computer_view(cx);
                        cx.stop_propagation();
                    }))
                    .child(div().text_sm().child(format!("🖥 {}", self.tr("此电脑")))),
            )
            .child(
                div()
                    .flex_1()
                    .h_full()
                    .id("folder-tree-scroll")
                    .overflow_y_scroll()
                    .child(items),
            )
            .into_any_element()
    }

    fn folder_tree_row(&self, cx: &mut Context<Root>, row: FolderTreeRow) -> AnyElement {
        let theme = self.theme;
        let indent = px((row.depth as f32) * 16.0);
        match row.kind {
            FolderTreeRowKind::Loading => div()
                .w_full()
                .px_2()
                .py_1()
                .pl(indent)
                .text_xs()
                .text_color(theme.muted)
                .child(self.tr("加载文件夹..."))
                .into_any_element(),
            FolderTreeRowKind::Error(error) => div()
                .w_full()
                .px_2()
                .py_1()
                .pl(indent)
                .text_xs()
                .text_color(theme.muted)
                .child(SharedString::from(format!(
                    "{} {}",
                    self.tr("无法读取:"),
                    error
                )))
                .into_any_element(),
            FolderTreeRowKind::Folder(entry) => {
                let is_link = entry.is_link;
                let expanded = self.folder_tree.is_expanded(&entry.path);
                let selected = !self.cur().computer_view
                    && tree_path_key(&self.cur().cwd) == tree_path_key(&entry.path);
                let favorite = self.is_favorite(&entry.path);
                let arrow = if entry.is_link {
                    " "
                } else if expanded {
                    "▾"
                } else {
                    "▸"
                };
                let toggle_path = entry.path.clone();
                let navigate_path = entry.path.clone();
                let favorite_path = entry.path.clone();
                let row_id = SharedString::from(format!("tree-row-{}", tree_path_key(&entry.path)));
                let toggle = div()
                    .w(px(16.0))
                    .text_sm()
                    .text_color(theme.muted)
                    .child(arrow);
                let toggle = if is_link {
                    toggle.into_any_element()
                } else {
                    toggle
                        .cursor_pointer()
                        .id(SharedString::from(format!(
                            "tree-toggle-{}",
                            tree_path_key(&entry.path)
                        )))
                        .on_click(cx.listener(move |this, _event, _window, cx| {
                            this.toggle_folder_tree(toggle_path.clone(), false, cx);
                            cx.stop_propagation();
                        }))
                        .into_any_element()
                };
                div()
                    .w_full()
                    .px_2()
                    .py_1()
                    .pl(indent)
                    .flex()
                    .items_center()
                    .gap_1()
                    .bg(if selected {
                        theme.selected
                    } else {
                        theme.mantle
                    })
                    .hover(|style| {
                        style.bg(if selected {
                            theme.selected
                        } else {
                            theme.hover
                        })
                    })
                    .cursor_pointer()
                    .id(row_id)
                    .on_click(cx.listener(move |this, _event, _window, cx| {
                        this.navigate_to_directory(navigate_path.clone(), cx);
                        cx.stop_propagation();
                    }))
                    .child(toggle)
                    .child(
                        div()
                            .flex_1()
                            .text_sm()
                            .text_color(if selected { theme.blue } else { theme.text })
                            .child(entry.name),
                    )
                    .child(
                        div()
                            .px_1()
                            .text_xs()
                            .text_color(if favorite { theme.blue } else { theme.muted })
                            .cursor_pointer()
                            .id(SharedString::from(format!(
                                "tree-favorite-{}",
                                tree_path_key(&favorite_path)
                            )))
                            .on_click(cx.listener(move |this, _event, _window, cx| {
                                this.toggle_favorite_path(favorite_path.clone(), cx);
                                cx.stop_propagation();
                            }))
                            .child(if favorite { "★" } else { "☆" }),
                    )
                    .into_any_element()
            }
        }
    }

    fn favorites_bar(&self, cx: &mut Context<Root>) -> AnyElement {
        let theme = self.theme;
        let mut bar = div()
            .w_full()
            .h(px(34.0))
            .px_2()
            .py_1()
            .bg(theme.mantle)
            .border_1()
            .border_color(theme.border)
            .flex()
            .items_center()
            .gap_2()
            .id("favorites-bar")
            .overflow_x_scroll();
        if self.settings.favorites.is_empty() {
            return bar
                .text_xs()
                .text_color(theme.muted)
                .child(self.tr("收藏夹为空"))
                .into_any_element();
        }
        for (index, favorite) in self.settings.favorites.iter().enumerate() {
            let path = favorite.clone();
            let available = Path::new(&path).is_dir();
            let label = favorite_display_name(&path);
            let mut item = div()
                .flex()
                .items_center()
                .gap_1()
                .px_2()
                .py_1()
                .bg(theme.surface0)
                .border_1()
                .border_color(theme.border)
                .rounded_sm()
                .hover(|style| style.bg(theme.hover))
                .id(SharedString::from(format!("favorite-{}", index)));
            if available {
                let open_path = path.clone();
                item = item.cursor_pointer().on_click(cx.listener(
                    move |this, _event, _window, cx| {
                        this.open_favorite(open_path.clone(), cx);
                        cx.stop_propagation();
                    },
                ));
            } else {
                item = item
                    .text_color(theme.muted)
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _event, _window, cx| {
                        this.set_status(format!("{}: {}", this.tr("路径不可用"), path));
                        cx.stop_propagation();
                    }));
            }
            let remove_path = favorite.clone();
            bar = bar.child(
                item.child(div().text_sm().child(label)).child(
                    div()
                        .px_1()
                        .cursor_pointer()
                        .id(SharedString::from(format!("favorite-remove-{}", index)))
                        .on_click(cx.listener(move |this, _event, _window, cx| {
                            this.toggle_favorite_path(remove_path.clone(), cx);
                            cx.stop_propagation();
                        }))
                        .child("×"),
                ),
            );
        }
        bar.into_any_element()
    }

    fn content_row(&self, cx: &mut Context<Root>) -> AnyElement {
        let theme = self.theme;
        let mut row = div()
            .flex()
            .flex_1()
            .h_full()
            .flex_row()
            .child(self.folder_tree_pane(cx))
            .child(div().w(px(1.0)).bg(theme.border))
            .child(self.file_list(cx));
        if !self.preview_collapsed {
            row = row
                .child(div().w(px(1.0)).bg(theme.border))
                .child(self.preview_pane());
        }
        row.into_any_element()
    }

    fn settings_page(&self, cx: &mut Context<Root>) -> AnyElement {
        ui::settings_page::render(self, cx)
    }

    fn address_bar(&self, cx: &mut Context<Self>, cwd: SharedString) -> AnyElement {
        if matches!(self.pending.as_ref(), Some(PendingOp::GoToPath)) {
            div()
                .flex_1()
                .px_2()
                .py_1()
                .bg(self.theme.surface0)
                .rounded_sm()
                .child(self.input_field(cx, self.tr("输入路径...")))
                .into_any_element()
        } else {
            div()
                .flex_1()
                .px_3()
                .py_1()
                .bg(self.theme.mantle)
                .border_1()
                .border_color(self.theme.border)
                .rounded_sm()
                .text_sm()
                .id("addr-bar")
                .cursor_pointer()
                .child("⌂")
                .child(cwd)
                .on_click(cx.listener(|this, _event, window, cx| {
                    this.start_goto(window, cx);
                }))
                .into_any_element()
        }
    }

    fn tab_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.theme;
        div()
            .w_full()
            .px_2()
            .py_1()
            .bg(theme.crust)
            .flex()
            .items_center()
            .gap_1()
            .children(self.tabs.iter().enumerate().map(|(i, tab)| {
                let active = i == self.active;
                let name = SharedString::from(if tab.computer_view {
                    self.tr("此电脑")
                } else {
                    tab_name(&tab.cwd)
                });
                tab_button(cx, theme, i, name, active)
            }))
            .child(new_tab_button(cx, theme))
    }

    fn context_menu(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(menu) = &self.menu else {
            return div().into_any_element();
        };
        let theme = self.theme;
        let pos = menu.position;
        let target_is_dir = menu.target.as_ref().is_some_and(|target| {
            self.cur().computer_view
                || self
                    .visible_files()
                    .iter()
                    .any(|file| file.name == *target && file.is_dir)
        });
        let target_path = menu.target.as_ref().map(|target| {
            if self.cur().computer_view {
                target.clone()
            } else {
                Path::new(&self.cur().cwd)
                    .join(target)
                    .to_string_lossy()
                    .into_owned()
            }
        });
        let items = menu_items_for(
            &menu.target,
            self.cur().computer_view,
            target_is_dir,
            target_path
                .as_deref()
                .is_some_and(|path| self.is_favorite(path)),
            self.language(),
        );

        div()
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .id("menu-mask")
            .on_click(cx.listener(|this, _e, _w, cx| {
                this.close_menu(cx);
            }))
            .child(
                div()
                    .absolute()
                    .left(pos.x)
                    .top(pos.y)
                    .w(px(170.0))
                    .py_1()
                    .bg(theme.mantle)
                    .border_1()
                    .border_color(theme.surface0)
                    .rounded_md()
                    .children(
                        items
                            .iter()
                            .map(|(label, action)| menu_item(cx, theme, label.clone(), *action)),
                    ),
            )
            .into_any_element()
    }
}

impl Focusable for Root {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl EntityInputHandler for Root {
    fn text_for_range(
        &mut self,
        range_utf16: Range<usize>,
        adjusted_range: &mut Option<Range<usize>>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<String> {
        let range = self.input_range_from_utf16(&range_utf16);
        adjusted_range.replace(self.input_range_to_utf16(&range));
        Some(self.input[range].to_string())
    }

    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: self.input_range_to_utf16(&self.input_selection),
            reversed: self.selection_reversed,
        })
    }

    fn marked_text_range(
        &self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Range<usize>> {
        self.marked_range
            .as_ref()
            .map(|range| self.input_range_to_utf16(range))
    }

    fn unmark_text(&mut self, _window: &mut Window, _cx: &mut Context<Self>) {
        self.marked_range = None;
    }

    fn replace_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        text: &str,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = range_utf16
            .as_ref()
            .map(|range| self.input_range_from_utf16(range))
            .or_else(|| self.marked_range.clone())
            .unwrap_or_else(|| self.input_selection.clone());
        self.replace_input_range(range, text, cx);
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        new_selected_range_utf16: Option<Range<usize>>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = range_utf16
            .as_ref()
            .map(|range| self.input_range_from_utf16(range))
            .or_else(|| self.marked_range.clone())
            .unwrap_or_else(|| self.input_selection.clone());
        self.input.replace_range(range.clone(), new_text);
        self.marked_range = if new_text.is_empty() {
            None
        } else {
            Some(range.start..range.start + new_text.len())
        };
        self.input_selection = new_selected_range_utf16
            .as_ref()
            .map(|range| self.input_range_from_utf16(range))
            .map(|selected| range.start + selected.start..range.start + selected.end)
            .unwrap_or_else(|| range.start + new_text.len()..range.start + new_text.len());
        self.selection_reversed = false;
        if matches!(self.pending, Some(PendingOp::Search)) {
            self.update_search_query(cx);
        }
        cx.notify();
    }

    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        bounds: Bounds<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let line = self.input_layout.as_ref()?;
        let range = self.input_range_from_utf16(&range_utf16);
        Some(Bounds::from_corners(
            point(bounds.left() + line.x_for_index(range.start), bounds.top()),
            point(bounds.left() + line.x_for_index(range.end), bounds.bottom()),
        ))
    }

    fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        let bounds = self.input_bounds.as_ref()?;
        let line = self.input_layout.as_ref()?;
        let local = bounds.localize(&point)?;
        let index = line.index_for_x(local.x)?.min(self.input.len());
        Some(self.input_offset_to_utf16(index))
    }
}

impl Render for Root {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.page == Page::Settings {
            return self.settings_page(cx);
        }
        let computer_view = self.cur().computer_view;
        let cwd = SharedString::from(if computer_view {
            self.tr("此电脑")
        } else {
            self.cur().cwd.clone()
        });
        let status = SharedString::from(self.status.clone().unwrap_or_else(|| {
            if computer_view {
                format!("{} {}", self.drive_roots.len(), self.tr("个磁盘"))
            } else if self
                .cur()
                .search
                .as_ref()
                .is_some_and(|search| !search.query.is_empty())
            {
                let count = self
                    .cur()
                    .search
                    .as_ref()
                    .map_or(0, |search| search.results.len());
                format!("{} {}", count, self.tr("项"))
            } else {
                format!("{} {}", self.cur().files.len(), self.tr("项"))
            }
        }));
        let focus_handle = self.focus_handle.clone();
        let theme = self.theme;
        let favorite_label = if !computer_view && self.is_favorite(&self.cur().cwd) {
            "★"
        } else {
            "☆"
        };
        let action_row = div()
            .w_full()
            .px_4()
            .py_2()
            .bg(theme.mantle)
            .border_1()
            .border_color(theme.border)
            .flex()
            .items_center()
            .gap_2()
            .child(command_button(
                cx,
                theme,
                "btn-open",
                "↗",
                self.tr("打开"),
                |this, _w, cx| this.open_selected(cx),
            ))
            .child(command_button(
                cx,
                theme,
                "btn-delete",
                "⌫",
                self.tr("删除"),
                |this, _w, cx| this.delete_selected(cx),
            ))
            .child(command_button(
                cx,
                theme,
                "btn-rename",
                "✎",
                self.tr("重命名"),
                |this, w, cx| this.start_rename(w, cx),
            ))
            .child(command_button(
                cx,
                theme,
                "btn-yank",
                "⧉",
                self.tr("复制"),
                |this, _w, cx| this.copy_selected(cx),
            ))
            .child(command_button(
                cx,
                theme,
                "btn-cut",
                "✂",
                self.tr("剪切"),
                |this, _w, cx| this.cut_selected(cx),
            ))
            .child(command_button(
                cx,
                theme,
                "btn-paste",
                "📋",
                self.tr("粘贴"),
                |this, _w, cx| this.paste_clipboard(cx),
            ))
            .child(toolbar_divider(theme))
            .child(command_button(
                cx,
                theme,
                "btn-newfile",
                "＋",
                self.tr("新建文件"),
                |this, w, cx| this.start_new_file(w, cx),
            ))
            .child(command_button(
                cx,
                theme,
                "btn-newdir",
                "＋",
                self.tr("新建文件夹"),
                |this, w, cx| this.start_new_dir(w, cx),
            ))
            .child(toolbar_divider(theme))
            .child(command_button(
                cx,
                theme,
                "btn-preview-toggle",
                "▣",
                self.tr(if self.preview_collapsed {
                    "展开预览"
                } else {
                    "折叠预览"
                }),
                |this, _w, cx| this.toggle_preview(cx),
            ))
            .child(div().flex_1())
            .child(self.file_search_field(cx))
            .child(command_button(
                cx,
                theme,
                "btn-settings",
                "⚙",
                self.tr("设置"),
                |this, _w, cx| this.show_settings(cx),
            ));

        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(theme.base)
            .text_color(theme.text)
            .id("root")
            .track_focus(&focus_handle)
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                this.on_input_key(event, window, cx);
            }))
            .child(self.tab_bar(cx))
            .child(
                div()
                    .w_full()
                    .px_4()
                    .py_2()
                    .bg(theme.base)
                    .border_1()
                    .border_color(theme.border)
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(self.address_bar(cx, cwd))
                    .child(action_button(
                        cx,
                        theme,
                        "btn-favorite-current",
                        favorite_label,
                        |this, _w, cx| this.toggle_current_favorite(cx),
                    ))
                    .child(refresh_button(cx, theme, self.language()))
                    .child(parent_button(cx, theme, self.language()))
                    .child(computer_button(cx, theme, self.language())),
            )
            .child(self.favorites_bar(cx))
            .child(action_row)
            .child(self.input_bar(cx))
            .child(self.content_row(cx))
            .child(self.status_bar(cx, theme, status))
            .child(self.context_menu(cx))
            .child(self.delete_confirmation(cx))
            .into_any_element()
    }
}

impl Root {
    fn status_bar(&self, cx: &mut Context<Root>, theme: Theme, status: SharedString) -> AnyElement {
        let Some(transfer) = &self.transfer else {
            return div()
                .w_full()
                .px_3()
                .py_1()
                .bg(theme.crust)
                .text_xs()
                .text_color(theme.muted)
                .child(status)
                .into_any_element();
        };

        let progress = &transfer.progress;
        let fraction = if progress.total_bytes > 0 {
            progress.completed_bytes as f32 / progress.total_bytes as f32
        } else if progress.total_items > 0 {
            progress.completed_items as f32 / progress.total_items as f32
        } else {
            0.0
        }
        .clamp(0.0, 1.0);
        let current = settings::translate(self.language(), &progress.current);
        let summary = SharedString::from(format!(
            "{} {:.0}% · {} / {} · {}",
            self.tr("粘贴"),
            fraction * 100.0,
            human_size(progress.completed_bytes),
            human_size(progress.total_bytes),
            current
        ));

        div()
            .w_full()
            .px_3()
            .py_1()
            .bg(theme.crust)
            .flex()
            .items_center()
            .gap_2()
            .child(
                div()
                    .flex_1()
                    .h(px(4.0))
                    .bg(theme.surface0)
                    .rounded_sm()
                    .child(
                        div()
                            .h_full()
                            .w(relative(fraction))
                            .bg(theme.blue)
                            .rounded_sm(),
                    ),
            )
            .child(div().text_xs().text_color(theme.muted).child(summary))
            .child(action_button(
                cx,
                theme,
                "btn-cancel-transfer",
                "取消",
                |this, _window, cx| this.cancel_transfer(cx),
            ))
            .into_any_element()
    }

    fn delete_confirmation(&self, cx: &mut Context<Root>) -> AnyElement {
        let Some(request) = &self.pending_delete else {
            return div().into_any_element();
        };
        let theme = self.theme;
        let summary = SharedString::from(format!(
            "{} {}，{} {} {}。{}，{}。",
            self.tr("已选择"),
            request.paths.len(),
            self.tr("其中"),
            request.network_count,
            self.tr("项位于网络驱动器"),
            self.tr("网络项目确认后将永久删除"),
            self.tr("无法恢复"),
        ));

        div()
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .bg(theme.crust)
            .id("delete-confirmation-mask")
            .on_click(cx.listener(|this, _event, _window, cx| {
                this.cancel_delete_confirmation(cx);
            }))
            .child(
                div()
                    .w(px(460.0))
                    .p_4()
                    .bg(theme.mantle)
                    .rounded_md()
                    .id("delete-confirmation-dialog")
                    .on_click(cx.listener(|_this, _event, _window, cx| {
                        cx.stop_propagation();
                    }))
                    .child(div().text_sm().child(self.tr("确认永久删除网络项目？")))
                    .child(
                        div()
                            .pt_2()
                            .text_xs()
                            .text_color(theme.muted)
                            .child(summary),
                    )
                    .child(
                        div()
                            .pt_4()
                            .flex()
                            .justify_end()
                            .gap_2()
                            .child(dialog_button(
                                cx,
                                theme,
                                "btn-cancel-delete",
                                self.tr("取消"),
                                |this, _window, cx| this.cancel_delete_confirmation(cx),
                            ))
                            .child(dialog_button(
                                cx,
                                theme,
                                "btn-confirm-delete",
                                self.tr("确认永久删除"),
                                |this, _window, cx| this.confirm_delete(cx),
                            )),
                    ),
            )
            .into_any_element()
    }

    fn file_list(&self, cx: &Context<Root>) -> impl IntoElement {
        ui::files_page::file_list(self, cx)
    }

    fn preview_pane(&self) -> impl IntoElement {
        ui::files_page::preview_pane(self)
    }
}

fn favorite_display_name(path: &str) -> String {
    if is_drive_root(path) {
        return path.to_string();
    }
    Path::new(path)
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| path.to_string())
}

fn normalize_favorite_path(path: &str) -> String {
    let mut normalized = path.trim().replace('/', "\\");
    let preserve_drive_root = is_drive_root(&normalized);
    if !preserve_drive_root {
        while normalized.ends_with('\\') {
            normalized.pop();
        }
    }
    normalized
}

fn favorite_path_key(path: &str) -> String {
    normalize_favorite_path(path).to_ascii_lowercase()
}

fn normalize_single_path(raw: &str) -> Result<String, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("路径不能为空".to_string());
    }
    if trimmed.contains(['\r', '\n']) {
        return Err("只支持粘贴单个目录路径".to_string());
    }

    let value = if let Some(value) = trimmed.strip_prefix('"') {
        value
            .strip_suffix('"')
            .ok_or_else(|| "路径引号不完整".to_string())?
    } else if trimmed.ends_with('"') {
        return Err("路径引号不完整".to_string());
    } else {
        trimmed
    };
    let value = value.trim();
    if value.is_empty() {
        return Err("路径不能为空".to_string());
    }
    Ok(value.to_string())
}

fn resolve_address_path(cwd: &str, raw: &str) -> Result<String, String> {
    let normalized = normalize_single_path(raw)?;
    let path = std::path::Path::new(&normalized);
    let path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::path::Path::new(cwd).join(path)
    };
    Ok(path.to_string_lossy().into_owned())
}

fn normalize_shortcut(raw: &str) -> String {
    raw.trim().to_ascii_lowercase().replace(' ', "")
}

fn shortcut_actions() -> [ShortcutAction; 11] {
    [
        ShortcutAction::Open,
        ShortcutAction::Search,
        ShortcutAction::NewTab,
        ShortcutAction::CloseTab,
        ShortcutAction::Delete,
        ShortcutAction::Rename,
        ShortcutAction::Copy,
        ShortcutAction::Cut,
        ShortcutAction::Paste,
        ShortcutAction::NewFile,
        ShortcutAction::NewDir,
    ]
}

fn keystroke_to_shortcut(keystroke: &Keystroke) -> Option<String> {
    let key = keystroke.key.trim().to_ascii_lowercase();
    if key.is_empty()
        || matches!(
            key.as_str(),
            "shift" | "control" | "ctrl" | "alt" | "platform" | "super" | "win" | "function" | "fn"
        )
    {
        return None;
    }
    let mut parts = Vec::new();
    if keystroke.modifiers.control {
        parts.push("ctrl");
    }
    if keystroke.modifiers.alt {
        parts.push("alt");
    }
    if keystroke.modifiers.shift {
        parts.push("shift");
    }
    if keystroke.modifiers.platform {
        parts.push("win");
    }
    if keystroke.modifiers.function {
        parts.push("fn");
    }
    parts.push(key.as_str());
    Some(parts.join("+"))
}

fn shortcut_matches(configured: &str, keystroke: &Keystroke) -> bool {
    let normalized = normalize_shortcut(configured);
    let parts: Vec<&str> = normalized.split('+').collect();
    let Some(key) = parts.last().copied() else {
        return false;
    };
    if key != keystroke.key.as_str() {
        return false;
    }
    let control = parts.contains(&"ctrl") || parts.contains(&"control");
    let shift = parts.contains(&"shift");
    let alt = parts.contains(&"alt");
    let platform = parts.contains(&"win")
        || parts.contains(&"super")
        || parts.contains(&"cmd")
        || parts.contains(&"platform");
    let function = parts.contains(&"fn") || parts.contains(&"function");
    control == keystroke.modifiers.control
        && shift == keystroke.modifiers.shift
        && alt == keystroke.modifiers.alt
        && platform == keystroke.modifiers.platform
        && function == keystroke.modifiers.function
}

fn refresh_request_matches(refreshing: bool, request_cwd: &str, event_cwd: &str) -> bool {
    refreshing && tree_path_key(request_cwd) == tree_path_key(event_cwd)
}

fn is_primary_yazi_tab(tab: usize) -> bool {
    tab == PRIMARY_YAZI_TAB
}

fn refresh_token_matches(refreshing: bool, current_id: u64, request_id: u64) -> bool {
    refreshing && current_id == request_id
}

fn operation_status(label: &str, success: usize, failed: usize) -> String {
    match (success, failed) {
        (0, 0) => format!("{}未执行", label),
        (_, 0) => format!("{}完成 {} 项", label, success),
        (0, _) => format!("{}失败 {} 项", label, failed),
        _ => format!("{}完成 {} 项，失败 {} 项", label, success, failed),
    }
}

fn is_image_file(path: &str) -> bool {
    std::path::Path::new(path)
        .extension()
        .and_then(|s| s.to_str())
        .map(|ext| {
            matches!(
                ext.to_ascii_lowercase().as_str(),
                "png"
                    | "jpg"
                    | "jpeg"
                    | "gif"
                    | "webp"
                    | "bmp"
                    | "svg"
                    | "ico"
                    | "avif"
                    | "tif"
                    | "tiff"
            )
        })
        .unwrap_or(false)
}

/// 根据文件类型返回一个 emoji 图标（用于文件列表）。
fn file_icon(name: &str, is_dir: bool) -> &'static str {
    if is_dir {
        return "📁";
    }
    let ext = std::path::Path::new(name)
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match ext.as_str() {
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "svg" | "ico" | "avif" => "🖼️",
        "zip" | "tar" | "gz" | "rar" | "7z" | "xz" | "bz2" => "📦",
        "exe" | "dll" | "msi" | "bin" => "⚙️",
        "rs" | "py" | "js" | "ts" | "go" | "c" | "cpp" | "h" | "hpp" | "java" | "lua" | "toml"
        | "json" | "yaml" | "yml" | "sh" | "md" | "html" | "css" | "rb" | "php" => "📝",
        _ => "📄",
    }
}

fn is_probably_text(bytes: &[u8]) -> bool {
    !bytes.iter().take(8000).any(|&b| b == 0)
}

fn truncate_preview(s: &str) -> String {
    const MAX: usize = 64 * 1024;
    if s.len() <= MAX {
        s.to_string()
    } else {
        let mut end = MAX;
        while !s.is_char_boundary(end) {
            end -= 1;
        }
        format!("{}\n\n... (预览已截断)", &s[..end])
    }
}

// ---- 语法高亮（syntect） ----

fn extension_of(path: &str) -> String {
    std::path::Path::new(path)
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_ascii_lowercase())
        .unwrap_or_default()
}

static SYNTAX_SET: OnceLock<syntect::parsing::SyntaxSet> = OnceLock::new();
static THEME_SET: OnceLock<syntect::highlighting::ThemeSet> = OnceLock::new();

fn syntax_set() -> &'static syntect::parsing::SyntaxSet {
    SYNTAX_SET.get_or_init(syntect::parsing::SyntaxSet::load_defaults_newlines)
}

fn theme_set() -> &'static syntect::highlighting::ThemeSet {
    THEME_SET.get_or_init(syntect::highlighting::ThemeSet::load_defaults)
}

/// 把 syntect 的 Style 转成 GPUI 的 HighlightStyle。
fn syntect_to_gpui_style(style: syntect::highlighting::Style) -> HighlightStyle {
    use syntect::highlighting::FontStyle as SynFontStyle;
    let c = style.foreground;
    HighlightStyle {
        color: Some(rgb(((c.r as u32) << 16) | ((c.g as u32) << 8) | (c.b as u32)).into()),
        font_weight: if style.font_style.contains(SynFontStyle::BOLD) {
            Some(FontWeight::BOLD)
        } else {
            None
        },
        font_style: if style.font_style.contains(SynFontStyle::ITALIC) {
            Some(gpui::FontStyle::Italic)
        } else {
            None
        },
        ..Default::default()
    }
}

/// 对文本做语法高亮，返回字节区间 -> 高亮样式。无匹配语法时返回 None。
fn highlight_code(
    text: &str,
    ext: &str,
    syntax_theme: &str,
) -> Option<Vec<(Range<usize>, HighlightStyle)>> {
    use syntect::easy::HighlightLines;
    use syntect::util::LinesWithEndings;

    let ss = syntax_set();
    let syntax = ss.find_syntax_by_extension(ext)?;
    let theme = theme_set().themes.get(syntax_theme)?;
    let mut highlighter = HighlightLines::new(syntax, theme);

    let mut highlights = Vec::new();
    let mut pos = 0usize;
    for line in LinesWithEndings::from(text) {
        let ranges = highlighter.highlight_line(line, ss).ok()?;
        for (style, seg) in ranges {
            let len = seg.len();
            if len > 0 {
                highlights.push((pos..pos + len, syntect_to_gpui_style(style)));
            }
            pos += len;
        }
    }
    Some(highlights)
}

fn human_size(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    if bytes < 1024 {
        return format!("{} B", bytes);
    }
    let mut size = bytes as f64;
    let mut i = 0;
    while size >= 1024.0 && i < UNITS.len() - 1 {
        size /= 1024.0;
        i += 1;
    }
    format!("{:.1} {}", size, UNITS[i])
}

fn main() {
    let args = std::env::args().collect::<Vec<_>>();
    match update::run_update_mode(&args[1..]) {
        Ok(true) => return,
        Ok(false) => {}
        Err(error) => {
            eprintln!("[yazi-gui] update process failed: {error:#}");
            return;
        }
    }
    let background = args.iter().any(|arg| arg == "--background");
    Application::new().run(move |cx: &mut App| {
        cx.bind_keys([
            KeyBinding::new("backspace", Backspace, None),
            KeyBinding::new("delete", Delete, None),
            KeyBinding::new("left", Left, None),
            KeyBinding::new("right", Right, None),
            KeyBinding::new("shift-left", SelectLeft, None),
            KeyBinding::new("shift-right", SelectRight, None),
            KeyBinding::new("ctrl-a", SelectAll, None),
            KeyBinding::new("ctrl-c", CopyText, None),
            KeyBinding::new("ctrl-v", PasteText, None),
            KeyBinding::new("ctrl-x", CutText, None),
            KeyBinding::new("home", Home, None),
            KeyBinding::new("end", End, None),
        ]);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::centered(size(px(1000.0), px(650.0)), cx)),
                show: !background,
                focus: !background,
                titlebar: Some(TitlebarOptions {
                    title: Some("yazi-gui".into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            |window, cx| {
                let root = cx.new(|cx| Root::new(cx));
                let window_handle = gpui::Window::window_handle(window);
                root.update(cx, |root, _cx| {
                    root.window_handle = Some(window_handle);
                });
                root.update(cx, |root, cx| {
                    let focus_handle = root.focus_handle.clone();
                    root.input_blur_subscription =
                        Some(cx.on_blur(&focus_handle, window, |this, _window, cx| {
                            if this.is_inline_editing() {
                                this.confirm_input(cx);
                            }
                        }));
                });
                let weak = root.downgrade();
                window.on_window_should_close(cx, move |window, cx| {
                    weak.update(cx, |root, cx| root.handle_window_close(window, cx))
                        .unwrap_or(true)
                });
                root
            },
        )
        .unwrap();
    });
}

#[cfg(test)]
mod tests {
    use super::{
        DeleteSummary, FileEntry, SortDirection, SortField, SortState, TransferOutcome,
        copy_paths_with_progress, delete_status, favorite_path_key, format_mtime,
        is_primary_yazi_tab, is_unc_path, keystroke_to_shortcut, normalize_favorite_path,
        normalize_shortcut, normalize_single_path, permanent_delete, reconcile_selection,
        refresh_request_matches, refresh_token_matches, resolve_address_path, scan_folder_children,
        search_directory, sort_files, tree_ancestor_paths, tree_path_key,
    };
    use std::fs;
    use std::sync::atomic::AtomicBool;
    use tokio::sync::mpsc::unbounded_channel;

    fn entry(name: &str, is_dir: bool, size: u64, mtime: f64) -> FileEntry {
        FileEntry {
            name: name.to_string(),
            is_dir,
            is_hidden: false,
            size,
            mtime,
        }
    }

    #[test]
    fn normalizes_a_quoted_single_path() {
        assert_eq!(
            normalize_single_path(r#""D:\Projects\gui_for_yazi""#).unwrap(),
            r"D:\Projects\gui_for_yazi"
        );
    }

    #[test]
    fn rejects_multiline_paths() {
        assert!(normalize_single_path("D:\\one\r\nD:\\two").is_err());
    }

    #[test]
    fn resolves_relative_paths_from_the_active_directory() {
        assert_eq!(
            resolve_address_path(r"D:\Projects\gui_for_yazi", "assets").unwrap(),
            r"D:\Projects\gui_for_yazi\assets"
        );
    }

    #[test]
    fn preserves_absolute_paths() {
        assert_eq!(
            resolve_address_path(r"D:\Projects\gui_for_yazi", r"C:\Temp").unwrap(),
            r"C:\Temp"
        );
    }

    #[test]
    fn normalizes_favorite_paths_and_deduplicates_case_insensitively() {
        assert_eq!(normalize_favorite_path(r"D:\Projects\"), r"D:\Projects");
        assert_eq!(normalize_favorite_path(r"D:\"), r"D:\");
        assert_eq!(
            favorite_path_key(r"\\SERVER\Share\Folder\"),
            favorite_path_key(r"\\server\share\folder")
        );
    }

    #[test]
    fn expands_drive_ancestors_for_the_folder_tree() {
        assert_eq!(
            tree_ancestor_paths(r"D:\Projects\gui_for_yazi"),
            vec![
                r"D:\".to_string(),
                r"D:\Projects".to_string(),
                r"D:\Projects\gui_for_yazi".to_string()
            ]
        );
        assert!(tree_ancestor_paths(r"\\server\share\folder").is_empty());
        assert_eq!(tree_path_key(r"D:/Projects/"), r"d:\projects");
    }

    #[test]
    fn folder_tree_scan_returns_only_direct_child_folders() {
        let root = std::env::temp_dir().join(format!(
            "yazi-gui-tree-test-{}-{}",
            std::process::id(),
            super::TEMP_SEQUENCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        fs::create_dir_all(root.join("z-folder")).unwrap();
        fs::create_dir_all(root.join("A-folder")).unwrap();
        fs::create_dir_all(root.join("z-folder").join("nested")).unwrap();
        fs::write(root.join("file.txt"), b"not a folder").unwrap();

        let children = scan_folder_children(&root).unwrap();
        assert_eq!(
            children
                .iter()
                .map(|child| child.name.as_str())
                .collect::<Vec<_>>(),
            vec!["A-folder", "z-folder"]
        );
        assert!(children.iter().all(|child| !child.is_link));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn sorts_directories_first_and_toggles_fields() {
        let mut files = vec![
            entry("z.txt", false, 3, 10.0),
            entry("a-dir", true, 0, 1.0),
            entry("b.txt", false, 1, 20.0),
            entry("A-dir-2", true, 0, 2.0),
        ];

        sort_files(&mut files, SortState::default());
        assert_eq!(
            files
                .iter()
                .map(|file| file.name.as_str())
                .collect::<Vec<_>>(),
            vec!["a-dir", "A-dir-2", "b.txt", "z.txt"]
        );

        sort_files(
            &mut files,
            SortState {
                field: SortField::Size,
                direction: SortDirection::Descending,
            },
        );
        assert_eq!(
            files
                .iter()
                .map(|file| file.name.as_str())
                .collect::<Vec<_>>(),
            vec!["a-dir", "A-dir-2", "z.txt", "b.txt"]
        );
    }

    #[test]
    fn missing_mtime_is_not_displayed() {
        assert_eq!(format_mtime(0.0), "");
        assert_eq!(format_mtime(-1.0), "");
    }

    #[test]
    fn recognizes_unc_and_extended_unc_paths() {
        assert!(is_unc_path(r"\\server\share\file.txt"));
        assert!(is_unc_path(r"\\?\UNC\server\share\file.txt"));
        assert!(!is_unc_path(r"\\?\C:\file.txt"));
        assert!(!is_unc_path(r"D:\file.txt"));
    }

    #[test]
    fn reconciles_selection_and_anchor_after_refresh() {
        let mut selected = vec!["keep.txt".to_string(), "gone.txt".to_string()];
        let mut anchor = Some("gone.txt".to_string());
        let available = vec!["keep.txt".to_string(), "new.txt".to_string()];

        reconcile_selection(&mut selected, &mut anchor, &available);

        assert_eq!(selected, vec!["keep.txt"]);
        assert_eq!(anchor, None);
    }

    #[test]
    fn permanent_delete_removes_files_and_directories() {
        let root = std::env::temp_dir().join(format!(
            "yazi-gui-delete-test-{}-{}",
            std::process::id(),
            super::TEMP_SEQUENCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        let file = root.join("file.txt");
        let directory = root.join("directory");
        fs::create_dir_all(&directory).unwrap();
        fs::write(&file, b"delete me").unwrap();
        fs::write(directory.join("nested.txt"), b"delete me too").unwrap();

        permanent_delete(&file).unwrap();
        permanent_delete(&directory).unwrap();

        assert!(!file.exists());
        assert!(!directory.exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn delete_status_reports_first_failure_and_permanent_count() {
        let status = delete_status(&DeleteSummary {
            success: 2,
            failed: 1,
            permanent_success: 1,
            first_failure: Some("remote.txt: access denied".to_string()),
        });

        assert_eq!(
            status,
            "删除完成 2 项，失败 1 项（永久删除 1 项），首个失败：remote.txt: access denied"
        );
    }

    #[test]
    fn copy_reports_bytes_and_commits_the_target() {
        let root = std::env::temp_dir().join(format!(
            "yazi-gui-copy-test-{}-{}",
            std::process::id(),
            super::TEMP_SEQUENCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        let source_dir = root.join("source");
        let destination_dir = root.join("destination");
        fs::create_dir_all(&source_dir).unwrap();
        fs::create_dir_all(&destination_dir).unwrap();
        let source = source_dir.join("data.bin");
        fs::write(&source, vec![7u8; 2_500_000]).unwrap();

        let (tx, mut rx) = unbounded_channel();
        let outcome = copy_paths_with_progress(
            &[source.to_string_lossy().into_owned()],
            &destination_dir.to_string_lossy(),
            &AtomicBool::new(false),
            &tx,
        );
        drop(tx);

        let mut saw_byte_progress = false;
        while let Ok(update) = rx.try_recv() {
            if let super::TransferUpdate::Progress {
                completed_bytes, ..
            } = update
            {
                saw_byte_progress |= completed_bytes > 0;
            }
        }
        assert!(matches!(outcome, TransferOutcome::Completed { success: 1 }));
        assert!(saw_byte_progress);
        assert_eq!(
            fs::metadata(destination_dir.join("data.bin"))
                .unwrap()
                .len(),
            2_500_000
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn cancelled_copy_does_not_leave_a_destination_file() {
        let root = std::env::temp_dir().join(format!(
            "yazi-gui-cancel-test-{}-{}",
            std::process::id(),
            super::TEMP_SEQUENCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        let source_dir = root.join("source");
        let destination_dir = root.join("destination");
        fs::create_dir_all(&source_dir).unwrap();
        fs::create_dir_all(&destination_dir).unwrap();
        let source = source_dir.join("data.bin");
        fs::write(&source, vec![3u8; 32]).unwrap();

        let (tx, _rx) = unbounded_channel();
        let cancel = AtomicBool::new(true);
        let outcome = copy_paths_with_progress(
            &[source.to_string_lossy().into_owned()],
            &destination_dir.to_string_lossy(),
            &cancel,
            &tx,
        );

        assert!(matches!(outcome, TransferOutcome::Cancelled { success: 0 }));
        assert!(!destination_dir.join("data.bin").exists());
        assert_eq!(fs::read_dir(&destination_dir).unwrap().count(), 0);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn refresh_completes_only_for_the_requested_directory() {
        assert!(refresh_request_matches(
            true,
            r"D:\Projects\gui_for_yazi",
            r"D:\Projects\gui_for_yazi"
        ));
        assert!(!refresh_request_matches(
            true,
            r"D:\Projects\gui_for_yazi",
            r"D:\Projects\gui_for_yazi\assets"
        ));
        assert!(!refresh_request_matches(
            false,
            r"D:\Projects\gui_for_yazi",
            r"D:\Projects\gui_for_yazi"
        ));
    }

    #[test]
    fn refresh_tokens_are_scoped_to_each_tab() {
        let tab_a_request = 7;
        let tab_b_request = 12;

        assert!(refresh_token_matches(true, tab_a_request, tab_a_request));
        assert!(refresh_token_matches(true, tab_b_request, tab_b_request));
        assert!(!refresh_token_matches(true, tab_a_request, tab_b_request));
        assert!(!refresh_token_matches(false, tab_a_request, tab_a_request));
    }

    #[test]
    fn recognizes_yazis_one_based_primary_tab() {
        assert!(is_primary_yazi_tab(1));
        assert!(!is_primary_yazi_tab(0));
    }

    #[test]
    fn normalizes_editable_shortcuts() {
        assert_eq!(normalize_shortcut(" Ctrl + Shift + N "), "ctrl+shift+n");
        assert_eq!(normalize_shortcut("F2"), "f2");
        let keystroke = gpui::Keystroke::parse("ctrl-shift-f").unwrap();
        assert_eq!(
            keystroke_to_shortcut(&keystroke),
            Some("ctrl+shift+f".to_string())
        );
    }

    #[test]
    fn searches_case_insensitive_names_and_relative_paths() {
        let root = std::env::temp_dir().join(format!(
            "yazi-gui-search-test-{}-{}",
            std::process::id(),
            super::TEMP_SEQUENCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        fs::create_dir_all(root.join("NestedFolder")).unwrap();
        fs::write(root.join("NestedFolder").join("Report.TXT"), b"report").unwrap();
        fs::write(root.join("visible.bin"), b"visible").unwrap();

        let results = search_directory(&root, "report").unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "NestedFolder\\Report.TXT");

        let folder_results = search_directory(&root, "nestedfolder").unwrap();
        assert!(
            folder_results
                .iter()
                .any(|result| result.name == "NestedFolder" && result.is_dir)
        );
        assert!(
            folder_results
                .iter()
                .any(|result| result.name == "NestedFolder\\Report.TXT")
        );
        fs::remove_dir_all(root).unwrap();
    }
}
