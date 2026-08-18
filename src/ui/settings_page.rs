use super::super::*;
use super::components::{action_button, settings_line, shortcut_binding_view};

pub(crate) fn render(root: &Root, cx: &mut Context<Root>) -> AnyElement {
    let theme = root.theme;
    let language = root.language();
    let focus_handle = root.focus_handle.clone();
    let theme_label = match root.settings.theme {
        ThemeMode::Dark => root.tr("暗色"),
        ThemeMode::Light => root.tr("浅色"),
    };
    let autostart_label = if root.settings.autostart {
        root.tr("已开启")
    } else {
        root.tr("已关闭")
    };
    let shortcuts = if root.shortcuts_expanded {
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
        .into_iter()
        .map(|action| shortcut_setting_row(root, cx, action))
        .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    let shortcut_indicator = if root.shortcuts_expanded {
        "▾"
    } else {
        "▸"
    };
    let shortcuts_label =
        SharedString::from(format!("{} {}", shortcut_indicator, root.tr("快捷键")));

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
        .child(
            div()
                .w_full()
                .px_5()
                .py_3()
                .bg(theme.base)
                .border_1()
                .border_color(theme.border)
                .flex()
                .items_center()
                .gap_3()
                .child(action_button(
                    cx,
                    theme,
                    "settings-back",
                    root.tr("返回文件"),
                    |this, _window, cx| this.show_files(cx),
                ))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(div().text_xl().child(root.tr("设置")))
                        .child(
                            div()
                                .text_xs()
                                .text_color(theme.muted)
                                .child(root.tr("应用外观、行为和更新")),
                        ),
                ),
        )
        .child(root.input_bar(cx))
        .child(
            div()
                .flex_1()
                .id("settings-content")
                .overflow_y_scroll()
                .px_5()
                .py_5()
                .child(
                    div()
                        .w_full()
                        .max_w(px(860.0))
                        .flex()
                        .flex_col()
                        .gap_4()
                        .child(section_card(
                            theme,
                            root.tr("外观"),
                            div()
                                .w_full()
                                .flex()
                                .flex_col()
                                .gap_1()
                                .child(settings_line(
                                    cx,
                                    theme,
                                    root.tr("主题"),
                                    theme_label,
                                    "settings-theme",
                                    |this, _window, cx| this.toggle_theme(cx),
                                ))
                                .child(settings_line(
                                    cx,
                                    theme,
                                    root.tr("语言"),
                                    root.tr(language.label()),
                                    "settings-language",
                                    |this, _window, cx| this.cycle_language(cx),
                                )),
                        ))
                        .child(section_card(
                            theme,
                            root.tr("行为"),
                            div()
                                .w_full()
                                .flex()
                                .flex_col()
                                .gap_1()
                                .child(settings_line(
                                    cx,
                                    theme,
                                    root.tr("自启动"),
                                    autostart_label,
                                    "settings-autostart",
                                    |this, _window, cx| {
                                        this.set_autostart(!this.settings.autostart, cx)
                                    },
                                ))
                                .child(
                                    div()
                                        .py_1()
                                        .text_xs()
                                        .text_color(theme.muted)
                                        .child(root.tr("自启动默认关闭")),
                                ),
                        ))
                        .child(section_card(
                            theme,
                            SharedString::from(format!(
                                "{} {}",
                                if root.shortcuts_expanded {
                                    "▾"
                                } else {
                                    "▸"
                                },
                                root.tr("快捷键")
                            )),
                            div()
                                .w_full()
                                .flex()
                                .flex_col()
                                .gap_1()
                                .child(
                                    div()
                                        .py_1()
                                        .text_xs()
                                        .text_color(theme.muted)
                                        .cursor_pointer()
                                        .id("settings-shortcuts-toggle")
                                        .on_click(cx.listener(|this, _event, _window, cx| {
                                            this.toggle_shortcuts(cx);
                                        }))
                                        .child(shortcuts_label),
                                )
                                .children(shortcuts),
                        ))
                        .child(section_card(
                            theme,
                            root.tr("关于"),
                            div()
                                .w_full()
                                .flex()
                                .flex_col()
                                .gap_2()
                                .child(div().text_sm().child(format!(
                                    "{}: {}",
                                    root.tr("当前版本"),
                                    env!("CARGO_PKG_VERSION")
                                )))
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(theme.muted)
                                        .child(root.tr("关闭窗口时隐藏到托盘")),
                                )
                                .child(update_panel(root, cx)),
                        )),
                ),
        )
        .child(root.status_bar(
            cx,
            theme,
            SharedString::from(root.status.clone().unwrap_or_default()),
        ))
        .into_any_element()
}

fn section_card(
    theme: Theme,
    title: impl Into<SharedString>,
    body: impl IntoElement,
) -> AnyElement {
    div()
        .w_full()
        .p_4()
        .bg(theme.mantle)
        .border_1()
        .border_color(theme.border)
        .rounded_md()
        .flex()
        .flex_col()
        .gap_3()
        .child(div().text_lg().child(title.into()))
        .child(body)
        .into_any_element()
}

fn update_panel(root: &Root, cx: &mut Context<Root>) -> AnyElement {
    let theme = root.theme;
    let mut panel = div()
        .w_full()
        .p_3()
        .bg(theme.base)
        .border_1()
        .border_color(theme.border)
        .rounded_sm()
        .flex()
        .flex_col()
        .gap_2()
        .child(div().text_sm().child(root.tr("应用更新")));

    match &root.update.phase {
        UpdatePhase::Idle => {
            panel = panel.child(action_button(
                cx,
                theme,
                "settings-update-check",
                root.tr("检查更新"),
                |this, _window, cx| this.check_for_updates(cx),
            ));
        }
        UpdatePhase::Checking => {
            panel = panel.child(
                div()
                    .text_xs()
                    .text_color(theme.muted)
                    .child(root.tr("正在检查更新...")),
            );
        }
        UpdatePhase::UpToDate { version } => {
            panel = panel
                .child(
                    div()
                        .text_xs()
                        .child(format!("{}: {}", root.tr("已是最新版本"), version)),
                )
                .child(action_button(
                    cx,
                    theme,
                    "settings-update-check-again",
                    root.tr("再次检查"),
                    |this, _window, cx| this.check_for_updates(cx),
                ));
        }
        UpdatePhase::Available(release) => {
            panel = panel
                .child(div().text_xs().child(format!(
                    "{}: {}",
                    root.tr("发现新版本"),
                    release.tag_name
                )))
                .child(
                    div()
                        .flex()
                        .gap_2()
                        .child(action_button(
                            cx,
                            theme,
                            "settings-update-download",
                            root.tr("下载更新"),
                            |this, _window, cx| this.download_update(cx),
                        ))
                        .child(action_button(
                            cx,
                            theme,
                            "settings-update-release",
                            root.tr("打开发布页"),
                            {
                                let url = release.page_url.clone();
                                move |this, _window, cx| this.open_external_url(url.clone(), cx)
                            },
                        )),
                );
        }
        UpdatePhase::Downloading { progress, .. } => {
            panel = panel
                .child(download_progress(theme, *progress))
                .child(action_button(
                    cx,
                    theme,
                    "settings-update-cancel",
                    root.tr("取消下载"),
                    |this, _window, cx| this.cancel_update_download(cx),
                ));
        }
        UpdatePhase::Ready {
            release, progress, ..
        } => {
            panel = panel
                .child(download_progress(theme, *progress))
                .child(div().text_xs().child(format!(
                    "{}: {}",
                    root.tr("下载完成"),
                    release.tag_name
                )))
                .child(action_button(
                    cx,
                    theme,
                    "settings-update-restart",
                    root.tr("重启完成更新"),
                    |this, _window, cx| this.restart_update(cx),
                ));
        }
        UpdatePhase::Restarting => {
            panel = panel.child(
                div()
                    .text_xs()
                    .text_color(theme.muted)
                    .child(root.tr("正在重启完成更新...")),
            );
        }
        UpdatePhase::Failed(error) => {
            panel = panel
                .child(
                    div()
                        .text_xs()
                        .text_color(theme.muted)
                        .child(SharedString::from(error.clone())),
                )
                .child(action_button(
                    cx,
                    theme,
                    "settings-update-retry",
                    root.tr("重新检查"),
                    |this, _window, cx| this.check_for_updates(cx),
                ));
        }
    }
    panel.into_any_element()
}

fn download_progress(theme: Theme, progress: update::DownloadProgress) -> AnyElement {
    let fraction = if progress.total > 0 {
        (progress.downloaded as f32 / progress.total as f32).clamp(0.0, 1.0)
    } else {
        0.0
    };
    div()
        .w_full()
        .flex()
        .flex_col()
        .gap_1()
        .child(
            div()
                .w_full()
                .h(px(6.0))
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
        .child(div().text_xs().text_color(theme.muted).child(format!(
            "{:.0}% · {} / {} · {}/s",
            fraction * 100.0,
            human_size(progress.downloaded),
            human_size(progress.total),
            human_size(progress.bytes_per_second),
        )))
        .into_any_element()
}

fn shortcut_setting_row(root: &Root, cx: &mut Context<Root>, action: ShortcutAction) -> AnyElement {
    let theme = root.theme;
    let label = SharedString::from(root.tr(action.label()));
    let value = action.shortcut(&root.settings.shortcuts);
    let recording = matches!(
        root.pending,
        Some(PendingOp::EditShortcut(editing)) if editing == action
    );
    div()
        .w_full()
        .py_1()
        .flex()
        .items_center()
        .gap_2()
        .child(div().flex_1().text_sm().child(label))
        .child(
            div()
                .min_w(px(150.0))
                .px_2()
                .py_1()
                .bg(theme.surface0)
                .border_1()
                .border_color(theme.border)
                .rounded_sm()
                .cursor_pointer()
                .id(action.id())
                .hover(|style| style.bg(theme.hover))
                .on_click(cx.listener(move |this, _event, window, cx| {
                    this.start_shortcut_edit(action, window, cx);
                }))
                .child(shortcut_binding_view(
                    theme,
                    value,
                    recording,
                    root.language(),
                )),
        )
        .into_any_element()
}
