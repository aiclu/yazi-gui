use gpui::*;

mod yazi;
use yazi::{FileEntry, YaziClient, YaziEvent};

const START_DIR: &str = "D:\\Projects\\gui_for_yazi";

struct Root {
    client: Option<YaziClient>,
    cwd: String,
    files: Vec<FileEntry>,
    hovered: Option<String>,
}

impl Root {
    fn new(cx: &mut Context<Self>) -> Self {
        let mut root = Self {
            client: None,
            cwd: START_DIR.to_string(),
            files: Vec::new(),
            hovered: None,
        };
        root.start_yazi(cx);
        root
    }

    fn start_yazi(&mut self, cx: &mut Context<Self>) {
        match YaziClient::spawn(std::path::Path::new(START_DIR)) {
            Ok((client, rx)) => {
                self.client = Some(client);
                cx.spawn(async move |weak, cx| {
                    while let Ok(event) = rx.recv() {
                        weak.update(cx, |this, cx| {
                            this.on_event(event);
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

    fn on_event(&mut self, event: YaziEvent) {
        match event {
            YaziEvent::Cd { url, .. } => {
                if let Some(u) = url {
                    self.cwd = u;
                }
            }
            YaziEvent::Hover { url, .. } => {
                self.hovered = url;
            }
            YaziEvent::GuiFiles { cwd, files, hovered } => {
                self.cwd = cwd;
                self.files = files;
                self.hovered = hovered;
            }
            _ => {}
        }
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
}

impl Render for Root {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let cwd = SharedString::from(self.cwd.clone());
        let status = SharedString::from(format!("{} 项", self.files.len()));

        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(0x1e1e2e))
            .text_color(rgb(0xcdd6f4))
            // 顶部：路径 + 上级按钮
            .child(
                div()
                    .w_full()
                    .px_3()
                    .py_2()
                    .bg(rgb(0x11111b))
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(div().flex_1().text_sm().child(cwd))
                    .child(parent_button(cx)),
            )
            // 文件列表
            .child(
                div()
                    .flex_1()
                    .id("file-list")
                    .overflow_y_scroll()
                    .children(self.files.iter().map(|f| {
                        let name = f.name.clone();
                        let is_dir = f.is_dir;
                        let size = f.size;
                        let hovered = is_hovered(&self.hovered, &self.cwd, &name);
                        file_row(cx, name, is_dir, size, hovered)
                    })),
            )
            // 底部状态栏
            .child(
                div()
                    .w_full()
                    .px_3()
                    .py_1()
                    .bg(rgb(0x11111b))
                    .text_xs()
                    .text_color(rgb(0x6c7086))
                    .child(status),
            )
    }
}

fn parent_button(cx: &mut Context<Root>) -> impl IntoElement {
    div()
        .px_2()
        .py_1()
        .bg(rgb(0x313244))
        .rounded_md()
        .cursor_pointer()
        .text_sm()
        .child("上级 ..")
        .id("btn-parent")
        .on_click(cx.listener(|this, _event, _window, _cx| {
            this.go_parent();
        }))
}

fn file_row(
    cx: &Context<Root>,
    name: String,
    is_dir: bool,
    size: u64,
    hovered: bool,
) -> impl IntoElement {
    let display = SharedString::from(if is_dir {
        format!("{}/", name)
    } else {
        name.clone()
    });
    let meta = SharedString::from(if is_dir {
        String::new()
    } else {
        human_size(size)
    });
    let name_color = if is_dir {
        rgb(0x89b4fa)
    } else {
        rgb(0xcdd6f4)
    };

    div()
        .w_full()
        .px_3()
        .py_1()
        .flex()
        .justify_between()
        .gap_3()
        .bg(if hovered { rgb(0x313244) } else { rgb(0x1e1e2e) })
        .cursor_pointer()
        .id(SharedString::from(name.clone()))
        .on_click(cx.listener(move |this, _event, _window, _cx| {
            if is_dir {
                this.enter(&name);
            }
        }))
        .child(div().text_sm().text_color(name_color).child(display))
        .child(div().text_xs().text_color(rgb(0x6c7086)).child(meta))
}

fn is_hovered(hovered: &Option<String>, cwd: &str, name: &str) -> bool {
    if let Some(h) = hovered {
        let full = std::path::Path::new(cwd).join(name);
        h.as_str() == full.to_string_lossy().as_ref()
    } else {
        false
    }
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
                window_bounds: Some(WindowBounds::centered(size(px(900.0), px(600.0)), cx)),
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
