use gpui::*;

mod yazi;
use yazi::{YaziClient, YaziEvent};

const START_DIR: &str = "D:\\Projects\\gui_for_yazi";

struct Root {
    client: Option<YaziClient>,
    cwd: String,
    hovered: Option<String>,
    log: Vec<String>,
}

impl Root {
    fn new(cx: &mut Context<Self>) -> Self {
        let mut root = Self {
            client: None,
            cwd: START_DIR.to_string(),
            hovered: None,
            log: Vec::new(),
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
                self.log.push(format!("[error] failed to start yazi: {e}"));
            }
        }
    }

    fn on_event(&mut self, event: YaziEvent) {
        match &event {
            YaziEvent::Cd { url, .. } => {
                if let Some(u) = url {
                    self.cwd = u.clone();
                }
            }
            YaziEvent::Hover { url, .. } => {
                self.hovered = url.clone();
            }
            _ => {}
        }
        self.log.push(event.label());
        if self.log.len() > 500 {
            let drop = self.log.len() - 500;
            self.log.drain(0..drop);
        }
    }
}

impl Render for Root {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let cwd = SharedString::from(self.cwd.clone());
        let hover = SharedString::from(format!("hover: {:?}", self.hovered));

        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(0x1e1e2e))
            .text_color(rgb(0xcdd6f4))
            .child(
                div()
                    .w_full()
                    .px_3()
                    .py_2()
                    .bg(rgb(0x11111b))
                    .text_sm()
                    .child(cwd),
            )
            .child(div().px_3().py_1().text_sm().child(hover))
            .child(
                div()
                    .px_3()
                    .py_2()
                    .flex()
                    .gap_2()
                    .child(nav_button(cx, "btn-d", "cd D:\\", "D:\\"))
                    .child(nav_button(cx, "btn-c", "cd C:\\", "C:\\"))
                    .child(nav_button(cx, "btn-parent", "cd ..", "..")),
            )
            .child(
                div()
                    .flex_1()
                    .px_3()
                    .py_2()
                    .children(self.log.iter().rev().map(|l| {
                        div()
                            .text_xs()
                            .text_color(rgb(0x6c7086))
                            .child(SharedString::from(l.clone()))
                    }))
                    .id("log")
                    .overflow_y_scroll(),
            )
    }
}

fn nav_button(
    cx: &mut Context<Root>,
    id: &'static str,
    label: &'static str,
    target: &'static str,
) -> impl IntoElement {
    div()
        .px_3()
        .py_1()
        .bg(rgb(0x313244))
        .rounded_md()
        .cursor_pointer()
        .text_sm()
        .child(label)
        .id(id)
        .on_click(cx.listener(move |this, _event, _window, _cx| {
            if let Some(client) = &this.client {
                let _ = client.send(&["cd", target]);
            }
        }))
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
