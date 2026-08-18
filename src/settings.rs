use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ThemeMode {
    Dark,
    Light,
}

impl Default for ThemeMode {
    fn default() -> Self {
        Self::Dark
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Language {
    System,
    Chinese,
    English,
}

impl Default for Language {
    fn default() -> Self {
        Self::System
    }
}

impl Language {
    pub fn effective(self) -> Self {
        match self {
            Self::System => {
                #[cfg(windows)]
                {
                    let language =
                        unsafe { windows_sys::Win32::Globalization::GetUserDefaultUILanguage() };
                    if language & 0x03ff == 0x0004 {
                        return Self::Chinese;
                    }
                    return Self::English;
                }
                #[cfg(not(windows))]
                {
                    Self::Chinese
                }
            }
            language => language,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::System => "跟随系统",
            Self::Chinese => "中文",
            Self::English => "English",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShortcutSettings {
    pub open: String,
    pub new_tab: String,
    pub close_tab: String,
    pub delete: String,
    pub rename: String,
    pub copy: String,
    pub cut: String,
    pub paste: String,
    pub new_file: String,
    pub new_dir: String,
    pub search: String,
}

impl Default for ShortcutSettings {
    fn default() -> Self {
        Self {
            open: "enter".to_string(),
            new_tab: "ctrl+t".to_string(),
            close_tab: "ctrl+w".to_string(),
            delete: "delete".to_string(),
            rename: "f2".to_string(),
            copy: "ctrl+c".to_string(),
            cut: "ctrl+x".to_string(),
            paste: "ctrl+v".to_string(),
            new_file: "ctrl+n".to_string(),
            new_dir: "ctrl+shift+n".to_string(),
            search: "ctrl+f".to_string(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppSettings {
    pub theme: ThemeMode,
    pub language: Language,
    pub shortcuts: ShortcutSettings,
    pub favorites: Vec<String>,
    /// Deliberately false: first launch must not alter Windows startup behavior.
    pub autostart: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            theme: ThemeMode::Dark,
            language: Language::System,
            shortcuts: ShortcutSettings::default(),
            favorites: Vec::new(),
            autostart: false,
        }
    }
}

pub fn settings_path() -> Result<PathBuf> {
    let app_data = std::env::var_os("APPDATA").ok_or_else(|| anyhow!("APPDATA is not set"))?;
    Ok(PathBuf::from(app_data)
        .join("yazi-gui")
        .join("settings.json"))
}

pub fn load() -> Result<AppSettings> {
    let path = settings_path()?;
    if !path.exists() {
        return Ok(AppSettings::default());
    }
    let text = fs::read_to_string(&path)
        .with_context(|| format!("read settings file {}", path.display()))?;
    serde_json::from_str(&text).with_context(|| format!("parse settings file {}", path.display()))
}

pub fn save(settings: &AppSettings) -> Result<()> {
    let path = settings_path()?;
    let parent = path
        .parent()
        .ok_or_else(|| anyhow!("settings path has no parent"))?;
    fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    let text = serde_json::to_string_pretty(settings)?;
    fs::write(&path, text).with_context(|| format!("write settings file {}", path.display()))
}

pub fn set_autostart(enabled: bool) -> Result<()> {
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

pub fn translate(language: Language, text: &str) -> String {
    if language.effective() != Language::English {
        return text.to_string();
    }
    match text {
        "跟随系统" => "System".to_string(),
        "中文" => "Chinese".to_string(),
        "暗色" => "Dark".to_string(),
        "浅色" => "Light".to_string(),
        "此电脑" => "This PC".to_string(),
        "刷新" => "Refresh".to_string(),
        "正在刷新..." => "Refreshing...".to_string(),
        "已刷新" => "Refreshed".to_string(),
        "正在刷新，请完成后再退出" => {
            "Refreshing; finish it before exiting".to_string()
        }
        "上级 .." => "Parent ..".to_string(),
        "打开" => "Open".to_string(),
        "搜索" => "Search".to_string(),
        "删除" => "Delete".to_string(),
        "重命名" => "Rename".to_string(),
        "复制" => "Copy".to_string(),
        "剪切" => "Cut".to_string(),
        "粘贴" => "Paste".to_string(),
        "新建文件" => "New file".to_string(),
        "新建文件夹" => "New folder".to_string(),
        "新建标签页" => "New tab".to_string(),
        "关闭标签页" => "Close tab".to_string(),
        "设置" => "Settings".to_string(),
        "收藏" => "Favorite".to_string(),
        "取消收藏" => "Remove favorite".to_string(),
        "收藏夹为空" => "No favorite folders".to_string(),
        "路径不可用" => "Path unavailable".to_string(),
        "此电脑视图不可收藏" => "Computer View cannot be favorited".to_string(),
        "文件夹树" => "Folder tree".to_string(),
        "展开预览" => "Show preview".to_string(),
        "折叠预览" => "Hide preview".to_string(),
        "名称" => "Name".to_string(),
        "修改时间" => "Modified".to_string(),
        "大小" => "Size".to_string(),
        "项" => "items".to_string(),
        "个磁盘" => "drives".to_string(),
        "已选择" => "Selected".to_string(),
        "已选" => "Selected".to_string(),
        "其中" => "including".to_string(),
        "项位于网络驱动器" => "items on network drives".to_string(),
        "网络项目确认后将永久删除" => {
            "network items will be permanently deleted after confirmation".to_string()
        }
        "无法恢复" => "cannot be recovered".to_string(),
        "确认永久删除网络项目？" => "Permanently delete network items?".to_string(),
        "二进制文件" => "Binary file".to_string(),
        "预览" => "Preview".to_string(),
        "目录" => "Directory".to_string(),
        "加载中..." => "Loading...".to_string(),
        "单击选中文件以预览" => "Select a file to preview".to_string(),
        "取消" => "Cancel".to_string(),
        "确认永久删除" => "Confirm permanent delete".to_string(),
        "正在复制" => "Preparing copy".to_string(),
        "检查更新" => "Check for updates".to_string(),
        "再次检查" => "Check again".to_string(),
        "重新检查" => "Check again".to_string(),
        "应用更新" => "App update".to_string(),
        "应用外观、行为和更新" => "Manage appearance, behavior, and updates".to_string(),
        "正在检查更新..." => "Checking for updates...".to_string(),
        "检查更新失败" => "Update check failed".to_string(),
        "已是最新版本" => "Up to date".to_string(),
        "发现新版本" => "New version available".to_string(),
        "下载更新" => "Download update".to_string(),
        "打开发布页" => "Open release page".to_string(),
        "正在下载更新" => "Downloading update".to_string(),
        "取消下载" => "Cancel download".to_string(),
        "下载完成" => "Download complete".to_string(),
        "下载完成，可重启更新" => "Download complete; restart to update".to_string(),
        "重启完成更新" => "Restart to update".to_string(),
        "正在重启完成更新..." => "Restarting to apply update...".to_string(),
        "下载已取消" => "Download cancelled".to_string(),
        "下载更新失败" => "Update download failed".to_string(),
        "正在取消下载" => "Cancelling download".to_string(),
        "启动更新失败" => "Failed to start update".to_string(),
        "关于" => "About".to_string(),
        "主题" => "Theme".to_string(),
        "语言" => "Language".to_string(),
        "快捷键" => "Shortcuts".to_string(),
        "自启动" => "Start with Windows".to_string(),
        "已开启" => "Enabled".to_string(),
        "已关闭" => "Disabled".to_string(),
        "外观" => "Appearance".to_string(),
        "行为" => "Behavior".to_string(),
        "返回文件" => "Back to files".to_string(),
        "输入快捷键..." => "Enter shortcut...".to_string(),
        "输入名称..." => "Enter name...".to_string(),
        "输入路径..." => "Enter path...".to_string(),
        "快捷键不能为空" => "Shortcut cannot be empty".to_string(),
        "快捷键冲突" => "Shortcut conflict".to_string(),
        "该快捷键已被占用" => "That shortcut is already in use".to_string(),
        "按下快捷键..." => "Press a shortcut...".to_string(),
        "清除快捷键" => "Clear shortcut".to_string(),
        "输入搜索..." => "Search...".to_string(),
        "搜索文件..." => "Search files...".to_string(),
        "加载文件夹..." => "Loading folder...".to_string(),
        "无法读取: " => "Cannot read: ".to_string(),
        "展开" => "Expand".to_string(),
        "收起" => "Collapse".to_string(),
        "搜索中..." => "Searching...".to_string(),
        "搜索失败" => "Search failed".to_string(),
        "搜索仅支持真实目录" => "Search is only available in a real directory".to_string(),
        "设置文件无效" => "Settings file is invalid".to_string(),
        "当前版本" => "Current version".to_string(),
        "关闭窗口时隐藏到托盘" => "Closing the window hides it to the tray".to_string(),
        "自启动默认关闭" => "Startup is disabled by default".to_string(),
        "保存失败" => "Save failed".to_string(),
        "显示" => "Show".to_string(),
        "退出" => "Exit".to_string(),
        "正在进行文件操作，请完成后再退出" => {
            "A file operation is in progress; finish it before exiting".to_string()
        }
        _ => text.to_string(),
    }
}

pub fn translate_status(language: Language, text: &str) -> String {
    if language.effective() != Language::English {
        return text.to_string();
    }
    let exact = translate(language, text);
    if exact != text {
        return exact;
    }
    for (prefix, replacement) in [
        ("正在删除 ", "Deleting "),
        ("正在粘贴 ", "Pasting "),
        ("已复制 ", "Copied "),
        ("已剪切 ", "Cut "),
        ("正在", "Working: "),
        ("粘贴完成 ", "Paste complete "),
        ("粘贴已取消，", "Paste cancelled, "),
        ("粘贴失败，", "Paste failed, "),
        ("删除完成 ", "Delete complete "),
        ("已跳转: ", "Navigated to: "),
        ("跳转失败: ", "Navigation failed: "),
        ("刷新失败: ", "Refresh failed: "),
        ("搜索失败: ", "Search failed: "),
        ("发现新版本: ", "New version available: "),
        ("已是最新版本: ", "Up to date: "),
        (
            "下载完成，可重启更新",
            "Download complete; restart to update",
        ),
        ("下载更新失败: ", "Update download failed: "),
        ("启动更新失败: ", "Failed to start update: "),
    ] {
        if let Some(rest) = text.strip_prefix(prefix) {
            return format!("{}{}", replacement, translate_status(language, rest));
        }
    }
    text.replace(" 项", " items")
        .replace(" 个磁盘", " drives")
        .replace("完成", "complete")
        .replace("失败", "failed")
}

#[cfg(test)]
mod tests {
    use super::{AppSettings, Language, ThemeMode, translate};

    #[test]
    fn defaults_keep_autostart_disabled() {
        let settings = AppSettings::default();
        assert!(!settings.autostart);
        assert_eq!(settings.language, Language::System);
        assert_eq!(settings.theme, ThemeMode::Dark);
    }

    #[test]
    fn settings_round_trip_without_compatibility_fields() {
        let settings = AppSettings::default();
        let json = serde_json::to_string(&settings).unwrap();
        let decoded: AppSettings = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, settings);
        assert!(decoded.favorites.is_empty());
    }

    #[test]
    fn settings_without_search_shortcut_are_rejected() {
        let json = r#"{
            "theme":"Dark",
            "language":"System",
            "shortcuts":{
                "open":"enter",
                "new_tab":"ctrl+t",
                "close_tab":"ctrl+w",
                "delete":"delete",
                "rename":"f2",
                "copy":"ctrl+c",
                "cut":"ctrl+x",
                "paste":"ctrl+v",
                "new_file":"ctrl+n",
                "new_dir":"ctrl+shift+n"
            },
            "autostart":false
        }"#;
        assert!(serde_json::from_str::<AppSettings>(json).is_err());
    }

    #[test]
    fn settings_without_favorites_are_rejected() {
        let mut value = serde_json::to_value(AppSettings::default()).unwrap();
        value.as_object_mut().unwrap().remove("favorites");
        assert!(serde_json::from_value::<AppSettings>(value).is_err());
    }

    #[test]
    fn english_translation_covers_settings_labels() {
        assert_eq!(translate(Language::English, "自启动"), "Start with Windows");
        assert_eq!(
            translate(Language::English, "检查更新"),
            "Check for updates"
        );
    }
}
