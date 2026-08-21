use super::super::*;

pub(crate) const BRAND_ICON_ASSET: &str = "icons/yazi-gui.svg";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Icon {
    Add,
    ArrowDown,
    ArrowLeft,
    ArrowRight,
    ArrowUp,
    Archive,
    Binary,
    ChevronDown,
    ChevronRight,
    Clipboard,
    Close,
    Code,
    Computer,
    Copy,
    Cut,
    Edit,
    ExternalLink,
    File,
    FilePlus,
    Folder,
    FolderPlus,
    Image,
    LayoutPanel,
    Maximize,
    Minimize,
    Refresh,
    Settings,
    Star,
    StarFilled,
    Trash,
}

impl Icon {
    #[cfg(test)]
    pub(crate) const ALL: &[Self] = &[
        Self::Add,
        Self::ArrowDown,
        Self::ArrowLeft,
        Self::ArrowRight,
        Self::ArrowUp,
        Self::Archive,
        Self::Binary,
        Self::ChevronDown,
        Self::ChevronRight,
        Self::Clipboard,
        Self::Close,
        Self::Code,
        Self::Computer,
        Self::Copy,
        Self::Cut,
        Self::Edit,
        Self::ExternalLink,
        Self::File,
        Self::FilePlus,
        Self::Folder,
        Self::FolderPlus,
        Self::Image,
        Self::LayoutPanel,
        Self::Maximize,
        Self::Minimize,
        Self::Refresh,
        Self::Settings,
        Self::Star,
        Self::StarFilled,
        Self::Trash,
    ];

    pub(crate) const fn asset_path(self) -> &'static str {
        match self {
            Self::Add => "icons/ui/add.svg",
            Self::ArrowDown => "icons/ui/arrow-down.svg",
            Self::ArrowLeft => "icons/ui/arrow-left.svg",
            Self::ArrowRight => "icons/ui/arrow-right.svg",
            Self::ArrowUp => "icons/ui/arrow-up.svg",
            Self::Archive => "icons/ui/archive.svg",
            Self::Binary => "icons/ui/binary.svg",
            Self::ChevronDown => "icons/ui/chevron-down.svg",
            Self::ChevronRight => "icons/ui/chevron-right.svg",
            Self::Clipboard => "icons/ui/clipboard.svg",
            Self::Close => "icons/ui/close.svg",
            Self::Code => "icons/ui/code.svg",
            Self::Computer => "icons/ui/computer.svg",
            Self::Copy => "icons/ui/copy.svg",
            Self::Cut => "icons/ui/cut.svg",
            Self::Edit => "icons/ui/edit.svg",
            Self::ExternalLink => "icons/ui/external-link.svg",
            Self::File => "icons/ui/file.svg",
            Self::FilePlus => "icons/ui/file-plus.svg",
            Self::Folder => "icons/ui/folder.svg",
            Self::FolderPlus => "icons/ui/folder-plus.svg",
            Self::Image => "icons/ui/image.svg",
            Self::LayoutPanel => "icons/ui/layout-panel.svg",
            Self::Maximize => "icons/ui/maximize.svg",
            Self::Minimize => "icons/ui/minimize.svg",
            Self::Refresh => "icons/ui/refresh.svg",
            Self::Settings => "icons/ui/settings.svg",
            Self::Star => "icons/ui/star.svg",
            Self::StarFilled => "icons/ui/star-filled.svg",
            Self::Trash => "icons/ui/trash.svg",
        }
    }
}

pub(crate) fn icon(icon: Icon, size: f32, color: Hsla) -> impl IntoElement {
    svg()
        .path(icon.asset_path())
        .w(px(size))
        .h(px(size))
        .flex_shrink_0()
        .text_color(color)
}

pub(crate) fn icon_asset(path: &str) -> Option<&'static [u8]> {
    Some(match path {
        "icons/ui/add.svg" => include_bytes!("../../assets/icons/ui/add.svg"),
        "icons/ui/arrow-down.svg" => include_bytes!("../../assets/icons/ui/arrow-down.svg"),
        "icons/ui/arrow-left.svg" => include_bytes!("../../assets/icons/ui/arrow-left.svg"),
        "icons/ui/arrow-right.svg" => include_bytes!("../../assets/icons/ui/arrow-right.svg"),
        "icons/ui/arrow-up.svg" => include_bytes!("../../assets/icons/ui/arrow-up.svg"),
        "icons/ui/archive.svg" => include_bytes!("../../assets/icons/ui/archive.svg"),
        "icons/ui/binary.svg" => include_bytes!("../../assets/icons/ui/binary.svg"),
        "icons/ui/chevron-down.svg" => include_bytes!("../../assets/icons/ui/chevron-down.svg"),
        "icons/ui/chevron-right.svg" => {
            include_bytes!("../../assets/icons/ui/chevron-right.svg")
        }
        "icons/ui/clipboard.svg" => include_bytes!("../../assets/icons/ui/clipboard.svg"),
        "icons/ui/close.svg" => include_bytes!("../../assets/icons/ui/close.svg"),
        "icons/ui/code.svg" => include_bytes!("../../assets/icons/ui/code.svg"),
        "icons/ui/computer.svg" => include_bytes!("../../assets/icons/ui/computer.svg"),
        "icons/ui/copy.svg" => include_bytes!("../../assets/icons/ui/copy.svg"),
        "icons/ui/cut.svg" => include_bytes!("../../assets/icons/ui/cut.svg"),
        "icons/ui/edit.svg" => include_bytes!("../../assets/icons/ui/edit.svg"),
        "icons/ui/external-link.svg" => include_bytes!("../../assets/icons/ui/external-link.svg"),
        "icons/ui/file-plus.svg" => include_bytes!("../../assets/icons/ui/file-plus.svg"),
        "icons/ui/file.svg" => include_bytes!("../../assets/icons/ui/file.svg"),
        "icons/ui/folder-plus.svg" => include_bytes!("../../assets/icons/ui/folder-plus.svg"),
        "icons/ui/folder.svg" => include_bytes!("../../assets/icons/ui/folder.svg"),
        "icons/ui/image.svg" => include_bytes!("../../assets/icons/ui/image.svg"),
        "icons/ui/layout-panel.svg" => include_bytes!("../../assets/icons/ui/layout-panel.svg"),
        "icons/ui/maximize.svg" => include_bytes!("../../assets/icons/ui/maximize.svg"),
        "icons/ui/minimize.svg" => include_bytes!("../../assets/icons/ui/minimize.svg"),
        "icons/ui/refresh.svg" => include_bytes!("../../assets/icons/ui/refresh.svg"),
        "icons/ui/settings.svg" => include_bytes!("../../assets/icons/ui/settings.svg"),
        "icons/ui/star-filled.svg" => include_bytes!("../../assets/icons/ui/star-filled.svg"),
        "icons/ui/star.svg" => include_bytes!("../../assets/icons/ui/star.svg"),
        "icons/ui/trash.svg" => include_bytes!("../../assets/icons/ui/trash.svg"),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::{Icon, icon_asset};

    #[test]
    fn every_icon_has_a_loadable_svg_asset() {
        for icon in Icon::ALL {
            let bytes = icon_asset(icon.asset_path()).expect("icon asset is missing");
            assert!(
                bytes.starts_with(b"<svg"),
                "{} is not an SVG",
                icon.asset_path()
            );
        }
    }

    #[test]
    fn unknown_icon_asset_is_not_loaded() {
        assert!(icon_asset("icons/ui/missing.svg").is_none());
    }
}
