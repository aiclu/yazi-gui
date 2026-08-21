use super::*;

pub(crate) mod components;
pub(crate) mod files_page;
pub(crate) mod icons;
pub(crate) mod input;
pub(crate) mod settings_page;

pub(crate) use components::{
    action_button, back_button, computer_button, dialog_button, forward_button, icon_button,
    menu_item, menu_items_for, new_tab_button, parent_button, refresh_button, resize_handle,
    tab_button, tab_name, tab_scroll_button, toolbar_divider, window_control_button,
};
pub(crate) use icons::{BRAND_ICON_ASSET, Icon, icon, icon_asset};
pub(crate) use input::InputElement;

#[derive(Clone, Copy)]
pub(crate) enum MenuAction {
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

#[derive(Clone)]
pub(crate) enum UiIntent {
    ShowFiles,
    ShowSettings,
    BeginWindowMove,
    MinimizeWindow,
    ToggleMaximize,
    RequestClose,
    NewTab,
    Refresh,
    Back,
    Forward,
    Parent,
    Computer,
    TogglePreview,
    SwitchTab(usize),
    CloseTab(usize),
    ShiftTabs(isize),
    BeginResize {
        target: ResizeTarget,
        x: f32,
    },
    EndResize,
    MoveResize(f32),
    BeginFileScrollDrag(f32),
    EndFileScrollDrag,
    MoveFileScrollDrag(f32),
    ClickFile {
        name: String,
        is_dir: bool,
        modifiers: Modifiers,
        click_count: usize,
    },
    OpenMenu {
        target: Option<String>,
        position: Point<Pixels>,
    },
    ClickBlank,
    OpenSelected,
    DeleteSelected,
    StartRename,
    CopySelected,
    CutSelected,
    PasteClipboard,
    StartNewFile,
    StartNewDir,
    ToggleCurrentFavorite,
    CancelTransfer,
    CancelDeleteConfirmation,
    ConfirmDelete,
    ToggleSort(SortField),
    ExecuteMenu(MenuAction),
    ToggleTheme,
    CycleLanguage,
    SetAutostart(bool),
    ToggleShortcuts,
    CheckUpdates,
    DownloadUpdate,
    OpenExternalUrl(String),
    CancelUpdateDownload,
    RestartUpdate,
    StartShortcutEdit(&'static str),
}

pub(crate) struct UiProjection<'a> {
    pub(crate) theme: Theme,
    pub(crate) layout: LayoutState,
    pub(crate) language: Language,
    pub(crate) focus_handle: FocusHandle,
    pub(crate) current_tab: &'a Tab,
    pub(crate) drive_roots: &'a [String],
    pub(crate) file_scroll: ScrollHandle,
    pub(crate) settings: &'a AppSettings,
    pub(crate) shortcuts_expanded: bool,
    pub(crate) pending: &'a Option<PendingOp>,
    pub(crate) update: &'a UpdateState,
    pub(crate) status: SharedString,
}

impl UiProjection<'_> {
    pub(crate) fn tr(&self, text: &str) -> String {
        settings::translate(self.language, text)
    }
}
