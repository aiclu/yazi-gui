use gpui::*;
use std::ops::Range;
use std::sync::OnceLock;

mod yazi;
use yazi::{FileEntry, YaziClient, YaziEvent};

const START_DIR: &str = "D:\\Projects\\gui_for_yazi";

enum Preview {
    Empty,
    Loading,
    Dir,
    Binary { size: u64 },
    Image { path: String },
    Code {
        text: String,
        highlights: Vec<(Range<usize>, HighlightStyle)>,
    },
    Text(String),
}

/// 界面配色主题。颜色采用 Catppuccin 色板。
#[derive(Clone, Copy)]
struct Theme {
    name: &'static str,
    base: Hsla,
    mantle: Hsla,
    surface0: Hsla,
    crust: Hsla,
    surface1: Hsla,
    text: Hsla,
    muted: Hsla,
    blue: Hsla,
    syntax_theme: &'static str,
}

impl Theme {
    /// 暗色（Catppuccin Mocha）。
    fn dark() -> Self {
        Theme {
            name: "暗色",
            base: rgb(0x1e1e2e).into(),
            mantle: rgb(0x181825).into(),
            surface0: rgb(0x313244).into(),
            crust: rgb(0x11111b).into(),
            surface1: rgb(0x45475a).into(),
            text: rgb(0xcdd6f4).into(),
            muted: rgb(0x6c7086).into(),
            blue: rgb(0x89b4fa).into(),
            syntax_theme: "base16-ocean.dark",
        }
    }

    /// 浅色（Catppuccin Latte）。
    fn light() -> Self {
        Theme {
            name: "浅色",
            base: rgb(0xeff1f5).into(),
            mantle: rgb(0xe6e9ef).into(),
            surface0: rgb(0xccd0da).into(),
            crust: rgb(0xdce0e8).into(),
            surface1: rgb(0xbcc0cc).into(),
            text: rgb(0x4c4f69).into(),
            muted: rgb(0x8c8fa1).into(),
            blue: rgb(0x1e66f5).into(),
            syntax_theme: "InspiredGitHub",
        }
    }
}

/// 输入模式下的待处理操作。
enum PendingOp {
    Rename { path: String },
    NewFile,
    NewDir,
}

struct Root {
    client: Option<YaziClient>,
    cwd: String,
    files: Vec<FileEntry>,
    hovered: Option<String>,
    preview: Preview,
    preview_path: Option<String>,
    focus_handle: FocusHandle,
    pending: Option<PendingOp>,
    input: String,
    theme: Theme,
}

impl Root {
    fn new(cx: &mut Context<Self>) -> Self {
        let mut root = Self {
            client: None,
            cwd: START_DIR.to_string(),
            files: Vec::new(),
            hovered: None,
            preview: Preview::Empty,
            preview_path: None,
            focus_handle: cx.focus_handle(),
            pending: None,
            input: String::new(),
            theme: Theme::dark(),
        };
        root.start_yazi(cx);
        root
    }

    fn start_yazi(&mut self, cx: &mut Context<Self>) {
        match YaziClient::spawn(std::path::Path::new(START_DIR)) {
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
            YaziEvent::Cd { url, .. } => {
                if let Some(u) = url {
                    self.cwd = u;
                }
            }
            YaziEvent::Hover { url, .. } => {
                self.set_hovered(url, cx);
            }
            YaziEvent::GuiFiles { cwd, files, hovered } => {
                self.cwd = cwd;
                self.files = files;
                self.set_hovered(hovered, cx);
            }
            _ => {}
        }
    }

    fn set_hovered(&mut self, url: Option<String>, cx: &mut Context<Self>) {
        if self.hovered == url {
            return;
        }
        self.hovered = url.clone();
        self.trigger_preview(url, cx);
    }

    fn trigger_preview(&mut self, hovered: Option<String>, cx: &mut Context<Self>) {
        match &hovered {
            Some(h) => {
                let name = file_name_of(h);
                let (is_dir, size) = self
                    .files
                    .iter()
                    .find(|f| f.name == name)
                    .map(|f| (f.is_dir, f.size))
                    .unwrap_or((false, 0));
                self.preview_path = Some(h.clone());
                self.load_preview(h.clone(), is_dir, size, cx);
            }
            None => {
                self.preview = Preview::Empty;
                self.preview_path = None;
            }
        }
    }

    fn load_preview(&mut self, path: String, is_dir: bool, size: u64, cx: &mut Context<Self>) {
        if is_dir {
            self.preview = Preview::Dir;
            return;
        }
        if is_image_file(&path) {
            self.preview = Preview::Image { path };
            return;
        }
        if size > 1_000_000 {
            self.preview = Preview::Binary { size };
            return;
        }
        self.preview = Preview::Loading;
        let syntax_theme = self.theme.syntax_theme;

        cx.spawn(async move |weak, cx| {
            let path2 = path.clone();
            // 读取 + 截断 + 语法高亮，全部放后台线程（syntect 是 CPU 密集）。
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
                if this.preview_path.as_deref() == Some(path.as_str()) {
                    this.preview = match result {
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

    fn send(&self, action: &[&str]) {
        if let Some(client) = &self.client {
            let _ = client.send(action);
        }
    }

    fn enter(&self, name: &str) {
        let path = std::path::Path::new(&self.cwd).join(name);
        self.send(&["cd", &path.to_string_lossy()]);
    }

    fn go_parent(&self) {
        self.send(&["cd", ".."]);
    }

    fn toggle_theme(&mut self, cx: &mut Context<Self>) {
        self.theme = match self.theme.syntax_theme {
            "InspiredGitHub" => Theme::dark(),
            _ => Theme::light(),
        };
        cx.notify();
    }

    fn reveal(&self, name: &str) {
        let path = std::path::Path::new(&self.cwd).join(name);
        self.send(&["reveal", &path.to_string_lossy()]);
    }

    fn open_hovered(&self, cx: &mut Context<Self>) {
        let Some(h) = self.hovered.clone() else { return };
        let name = file_name_of(&h);
        let is_dir = self.files.iter().any(|f| f.name == name && f.is_dir);
        if is_dir {
            self.enter(&name);
            return;
        }
        cx.spawn(async move |_weak, cx| {
            cx.background_executor()
                .spawn(async move {
                    let _ = open::that(&h);
                })
                .await;
        })
        .detach();
    }

    fn delete_hovered(&self, cx: &mut Context<Self>) {
        let Some(h) = self.hovered.clone() else { return };
        let cwd = self.cwd.clone();
        cx.spawn(async move |weak, cx| {
            let path2 = h.clone();
            let ok = cx
                .background_executor()
                .spawn(async move { trash::delete(&path2).is_ok() })
                .await;
            if ok {
                let cwd2 = cwd.clone();
                weak.update(cx, |this, _cx| {
                    this.send(&["cd", cwd2.as_str()]);
                })
                .ok();
            }
        })
        .detach();
    }

    // ---- 输入模式（重命名 / 新建） ----

    fn start_rename(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(h) = self.hovered.clone() else { return };
        self.input = file_name_of(&h);
        self.pending = Some(PendingOp::Rename { path: h });
        cx.focus_self(window);
        cx.notify();
    }

    fn start_new_file(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.input.clear();
        self.pending = Some(PendingOp::NewFile);
        cx.focus_self(window);
        cx.notify();
    }

    fn start_new_dir(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.input.clear();
        self.pending = Some(PendingOp::NewDir);
        cx.focus_self(window);
        cx.notify();
    }

    fn cancel_input(&mut self, cx: &mut Context<Self>) {
        self.pending = None;
        self.input.clear();
        cx.notify();
    }

    fn confirm_input(&mut self, cx: &mut Context<Self>) {
        let Some(pending) = self.pending.take() else { return };
        let name = self.input.trim().to_string();
        self.input.clear();
        cx.notify();

        if name.is_empty() {
            return;
        }

        let cwd = self.cwd.clone();
        cx.spawn(async move |weak, cx| {
            let cwd2 = cwd.clone();
            let name2 = name.clone();
            let ok = cx
                .background_executor()
                .spawn(async move {
                    let target = std::path::Path::new(&cwd2).join(&name2);
                    match &pending {
                        PendingOp::Rename { path } => std::fs::rename(path, &target).is_ok(),
                        PendingOp::NewFile => std::fs::write(&target, b"").is_ok(),
                        PendingOp::NewDir => std::fs::create_dir_all(&target).is_ok(),
                    }
                })
                .await;
            if ok {
                let cwd3 = cwd.clone();
                weak.update(cx, |this, _cx| {
                    this.send(&["cd", cwd3.as_str()]);
                })
                .ok();
            }
        })
        .detach();
    }

    fn on_input_key(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {
        if self.pending.is_none() {
            return;
        }
        let ks = &event.keystroke;
        match ks.key.as_str() {
            "enter" => self.confirm_input(cx),
            "escape" => self.cancel_input(cx),
            "backspace" => {
                self.input.pop();
                cx.notify();
            }
            "space" => {
                self.input.push(' ');
                cx.notify();
            }
            key if key.len() == 1 => {
                if !ks.modifiers.control && !ks.modifiers.alt && !ks.modifiers.platform {
                    let ch = ks.key_char.clone().unwrap_or_else(|| key.to_string());
                    self.input.push_str(&ch);
                    cx.notify();
                }
            }
            _ => {}
        }
    }

    fn input_bar(&self) -> impl IntoElement {
        let (label, text) = match &self.pending {
            Some(PendingOp::Rename { .. }) => ("重命名", self.input.clone()),
            Some(PendingOp::NewFile) => ("新建文件", self.input.clone()),
            Some(PendingOp::NewDir) => ("新建文件夹", self.input.clone()),
            None => return div(),
        };
        div()
            .w_full()
            .px_3()
            .py_1()
            .bg(self.theme.surface1)
            .text_sm()
            .child(SharedString::from(format!("{}: {}_", label, text)))
    }
}

impl Focusable for Root {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for Root {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let cwd = SharedString::from(self.cwd.clone());
        let status = SharedString::from(format!("{} 项", self.files.len()));
        let focus_handle = self.focus_handle.clone();
        let theme = self.theme;

        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(theme.base)
            .text_color(theme.text)
            .id("root")
            .track_focus(&focus_handle)
            .on_key_down(cx.listener(|this, event, _window, cx| {
                this.on_input_key(event, cx);
            }))
            .child(
                div()
                    .w_full()
                    .px_3()
                    .py_2()
                    .bg(theme.crust)
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(div().flex_1().text_sm().child(cwd))
                    .child(parent_button(cx, theme)),
            )
            .child(
                div()
                    .w_full()
                    .px_3()
                    .py_1()
                    .bg(theme.mantle)
                    .flex()
                    .gap_2()
                    .child(action_button(cx, theme, "btn-open", "打开", |this, _w, cx| this.open_hovered(cx)))
                    .child(action_button(cx, theme, "btn-delete", "删除", |this, _w, cx| this.delete_hovered(cx)))
                    .child(action_button(cx, theme, "btn-rename", "重命名", |this, w, cx| this.start_rename(w, cx)))
                    .child(action_button(cx, theme, "btn-yank", "复制", |this, _w, _cx| this.send(&["yank"])))
                    .child(action_button(cx, theme, "btn-cut", "剪切", |this, _w, _cx| this.send(&["cut"])))
                    .child(action_button(cx, theme, "btn-paste", "粘贴", |this, _w, _cx| this.send(&["paste"])))
                    .child(action_button(cx, theme, "btn-newfile", "新建文件", |this, w, cx| this.start_new_file(w, cx)))
                    .child(action_button(cx, theme, "btn-newdir", "新建文件夹", |this, w, cx| this.start_new_dir(w, cx)))
                    .child(action_button(cx, theme, "btn-theme", theme.name, |this, _w, cx| this.toggle_theme(cx))),
            )
            .child(self.input_bar())
            .child(
                div()
                    .flex()
                    .flex_1()
                    .flex_row()
                    .child(self.file_list(cx))
                    .child(div().w(px(1.0)).bg(theme.surface0))
                    .child(self.preview_pane()),
            )
            .child(
                div()
                    .w_full()
                    .px_3()
                    .py_1()
                    .bg(theme.crust)
                    .text_xs()
                    .text_color(theme.muted)
                    .child(status),
            )
    }
}

impl Root {
    fn file_list(&self, cx: &Context<Self>) -> impl IntoElement {
        div()
            .flex_1()
            .id("file-list")
            .overflow_y_scroll()
            .children(self.files.iter().map(|f| {
                let name = f.name.clone();
                let is_dir = f.is_dir;
                let size = f.size;
                let hovered = is_hovered(&self.hovered, &self.cwd, &name);
                file_row(cx, self.theme, name, is_dir, size, hovered)
            }))
    }

    fn preview_pane(&self) -> impl IntoElement {
        let title = SharedString::from(
            self.hovered
                .as_deref()
                .map(file_name_of)
                .unwrap_or_else(|| "预览".to_string()),
        );

        div()
            .flex_1()
            .flex()
            .flex_col()
            .bg(self.theme.mantle)
            .child(
                div()
                    .px_3()
                    .py_2()
                    .bg(self.theme.crust)
                    .text_sm()
                    .text_color(self.theme.muted)
                    .child(title),
            )
            .child(
                div()
                    .flex_1()
                    .id("preview-content")
                    .overflow_y_scroll()
                    .px_3()
                    .py_2()
                    .child(self.preview_body()),
            )
    }

    fn preview_body(&self) -> AnyElement {
        match &self.preview {
            Preview::Empty => div()
                .text_sm()
                .text_color(self.theme.muted)
                .child("悬停文件以预览")
                .into_any_element(),
            Preview::Loading => div()
                .text_sm()
                .text_color(self.theme.muted)
                .child("加载中...")
                .into_any_element(),
            Preview::Dir => div().text_sm().child("目录").into_any_element(),
            Preview::Binary { size } => div()
                .text_sm()
                .child(SharedString::from(format!(
                    "二进制文件 · {}",
                    human_size(*size)
                )))
                .into_any_element(),
            Preview::Image { path } => img(std::path::PathBuf::from(path.clone()))
                .w_full()
                .h(px(400.0))
                .object_fit(ObjectFit::Contain)
                .into_any_element(),
            Preview::Code { text, highlights } => div()
                .text_xs()
                .child(
                    StyledText::new(text.clone()).with_highlights(highlights.clone()),
                )
                .into_any_element(),
            Preview::Text(t) => div()
                .text_xs()
                .child(SharedString::from(t.clone()))
                .into_any_element(),
        }
    }
}

fn parent_button(cx: &mut Context<Root>, theme: Theme) -> impl IntoElement {
    div()
        .px_2()
        .py_1()
        .bg(theme.surface0)
        .rounded_md()
        .cursor_pointer()
        .text_sm()
        .child("上级 ..")
        .id("btn-parent")
        .on_click(cx.listener(|this, _event, _window, _cx| {
            this.go_parent();
        }))
}

fn action_button(
    cx: &mut Context<Root>,
    theme: Theme,
    id: &'static str,
    label: &'static str,
    on_click: impl Fn(&mut Root, &mut Window, &mut Context<Root>) + 'static,
) -> impl IntoElement {
    div()
        .px_2()
        .py_1()
        .bg(theme.surface0)
        .rounded_md()
        .cursor_pointer()
        .text_sm()
        .child(label)
        .id(id)
        .on_click(cx.listener(move |this, _event, window, cx| {
            on_click(this, window, cx);
        }))
}

fn file_row(
    cx: &Context<Root>,
    theme: Theme,
    name: String,
    is_dir: bool,
    size: u64,
    hovered: bool,
) -> impl IntoElement {
    let icon = file_icon(&name, is_dir);
    let display = SharedString::from(if is_dir {
        format!("{} {}/", icon, name)
    } else {
        format!("{} {}", icon, name)
    });
    let meta = SharedString::from(if is_dir {
        String::new()
    } else {
        human_size(size)
    });
    let name_color = if is_dir { theme.blue } else { theme.text };
    let hover_name = name.clone();

    div()
        .w_full()
        .px_3()
        .py_1()
        .flex()
        .justify_between()
        .gap_3()
        .bg(if hovered { theme.surface0 } else { theme.base })
        .cursor_pointer()
        .id(SharedString::from(name.clone()))
        .on_hover(cx.listener(move |this, hovered, _window, _cx| {
            if *hovered {
                this.reveal(&hover_name);
            }
        }))
        .on_click(cx.listener(move |this, _event, _window, _cx| {
            if is_dir {
                this.enter(&name);
            }
        }))
        .child(div().text_sm().text_color(name_color).child(display))
        .child(div().text_xs().text_color(theme.muted).child(meta))
}

fn is_hovered(hovered: &Option<String>, cwd: &str, name: &str) -> bool {
    if let Some(h) = hovered {
        let full = std::path::Path::new(cwd).join(name);
        h.as_str() == full.to_string_lossy().as_ref()
    } else {
        false
    }
}

fn file_name_of(url: &str) -> String {
    std::path::Path::new(url)
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| url.to_string())
}

fn is_image_file(path: &str) -> bool {
    std::path::Path::new(path)
        .extension()
        .and_then(|s| s.to_str())
        .map(|ext| {
            matches!(
                ext.to_ascii_lowercase().as_str(),
                "png" | "jpg"
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
        color: Some(
            rgb(((c.r as u32) << 16) | ((c.g as u32) << 8) | (c.b as u32)).into(),
        ),
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
    Application::new().run(|cx: &mut App| {
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::centered(size(px(1000.0), px(650.0)), cx)),
                titlebar: Some(TitlebarOptions {
                    title: Some("yazi-gui".into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            |_window, cx| cx.new(|cx| Root::new(cx)),
        )
        .unwrap();
    });
}
