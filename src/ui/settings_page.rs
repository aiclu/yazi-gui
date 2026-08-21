use super::super::*;
use super::components::{action_button, settings_line, shortcut_binding_view};
use super::{Icon, UiIntent, icon};

pub(crate) fn render(
    view: &UiProjection,
    cx: &mut Context<Root>,
    titlebar: AnyElement,
    input_bar: AnyElement,
    status_bar: AnyElement,
) -> AnyElement {
    let theme = view.theme;
    let language = view.language;
    let focus_handle = view.focus_handle.clone();
    let theme_label = match view.settings.theme {
        ThemeMode::Dark => view.tr("暗色"),
        ThemeMode::Light => view.tr("浅色"),
    };
    let autostart_label = if view.settings.autostart {
        view.tr("已开启")
    } else {
        view.tr("已关闭")
    };
    let shortcuts = if view.shortcuts_expanded {
        [
            ShortcutAction::Open,
            ShortcutAction::Search,
            ShortcutAction::Back,
            ShortcutAction::Forward,
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
        .map(|action| shortcut_setting_row(view, cx, action))
        .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    let shortcut_indicator = if view.shortcuts_expanded {
        Icon::ChevronDown
    } else {
        Icon::ChevronRight
    };

    div()
        .size_full()
        .flex()
        .flex_col()
        .bg(theme.base)
        .text_color(theme.text)
        .id("root")
        .track_focus(&focus_handle)
        .on_mouse_down(
            MouseButton::Navigate(NavigationDirection::Back),
            cx.listener(|this, _event, _window, cx| {
                this.go_back(cx);
                cx.stop_propagation();
            }),
        )
        .on_mouse_up(
            MouseButton::Navigate(NavigationDirection::Back),
            cx.listener(|_this, _event, _window, cx| {
                cx.stop_propagation();
            }),
        )
        .on_mouse_down(
            MouseButton::Navigate(NavigationDirection::Forward),
            cx.listener(|this, _event, _window, cx| {
                this.go_forward(cx);
                cx.stop_propagation();
            }),
        )
        .on_mouse_up(
            MouseButton::Navigate(NavigationDirection::Forward),
            cx.listener(|_this, _event, _window, cx| {
                cx.stop_propagation();
            }),
        )
        .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
            this.on_input_key(event, window, cx);
        }))
        .child(titlebar)
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
                    view.tr("返回文件"),
                    UiIntent::ShowFiles,
                ))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(div().text_xl().child(view.tr("设置")))
                        .child(
                            div()
                                .text_xs()
                                .text_color(theme.muted)
                                .child(view.tr("应用外观、行为和更新")),
                        ),
                ),
        )
        .child(input_bar)
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
                            view.tr("外观"),
                            div()
                                .w_full()
                                .flex()
                                .flex_col()
                                .gap_1()
                                .child(settings_line(
                                    cx,
                                    theme,
                                    view.tr("主题"),
                                    theme_label,
                                    "settings-theme",
                                    UiIntent::ToggleTheme,
                                ))
                                .child(settings_line(
                                    cx,
                                    theme,
                                    view.tr("语言"),
                                    view.tr(language.label()),
                                    "settings-language",
                                    UiIntent::CycleLanguage,
                                )),
                        ))
                        .child(section_card(
                            theme,
                            view.tr("行为"),
                            div()
                                .w_full()
                                .flex()
                                .flex_col()
                                .gap_1()
                                .child(settings_line(
                                    cx,
                                    theme,
                                    view.tr("自启动"),
                                    autostart_label,
                                    "settings-autostart",
                                    UiIntent::SetAutostart(!view.settings.autostart),
                                ))
                                .child(
                                    div()
                                        .py_1()
                                        .text_xs()
                                        .text_color(theme.muted)
                                        .child(view.tr("自启动默认关闭")),
                                ),
                        ))
                        .child(section_card_with_title(
                            theme,
                            div()
                                .w_full()
                                .py_1()
                                .text_lg()
                                .cursor_pointer()
                                .id("settings-shortcuts-toggle")
                                .on_click(cx.listener(|this, _event, window, cx| {
                                    this.dispatch_ui_intent(UiIntent::ToggleShortcuts, window, cx);
                                }))
                                .flex()
                                .items_center()
                                .gap_1()
                                .child(icon(shortcut_indicator, 14.0, theme.text))
                                .child(view.tr("快捷键")),
                            div().w_full().flex().flex_col().gap_1().children(shortcuts),
                        ))
                        .child(section_card(
                            theme,
                            view.tr("关于"),
                            div()
                                .w_full()
                                .flex()
                                .flex_col()
                                .gap_2()
                                .child(div().text_sm().child(format!(
                                    "{}: {}",
                                    view.tr("当前版本"),
                                    env!("CARGO_PKG_VERSION")
                                )))
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(theme.muted)
                                        .child(view.tr("关闭窗口时隐藏到托盘")),
                                )
                                .child(update_panel(view, cx)),
                        )),
                ),
        )
        .child(status_bar)
        .into_any_element()
}

fn section_card(
    theme: Theme,
    title: impl Into<SharedString>,
    body: impl IntoElement,
) -> AnyElement {
    section_card_with_title(theme, div().text_lg().child(title.into()), body)
}

fn section_card_with_title(
    theme: Theme,
    title: impl IntoElement,
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
        .child(title)
        .child(body)
        .into_any_element()
}

fn update_panel(view: &UiProjection, cx: &mut Context<Root>) -> AnyElement {
    let theme = view.theme;
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
        .child(div().text_sm().child(view.tr("应用更新")));

    match &view.update.phase {
        UpdatePhase::Idle => {
            panel = panel.child(action_button(
                cx,
                theme,
                "settings-update-check",
                view.tr("检查更新"),
                UiIntent::CheckUpdates,
            ));
        }
        UpdatePhase::Checking => {
            panel = panel.child(
                div()
                    .text_xs()
                    .text_color(theme.muted)
                    .child(view.tr("正在检查更新...")),
            );
        }
        UpdatePhase::UpToDate { version } => {
            panel = panel
                .child(
                    div()
                        .text_xs()
                        .child(format!("{}: {}", view.tr("已是最新版本"), version)),
                )
                .child(action_button(
                    cx,
                    theme,
                    "settings-update-check-again",
                    view.tr("再次检查"),
                    UiIntent::CheckUpdates,
                ));
        }
        UpdatePhase::Available(release) => {
            panel = panel
                .child(div().text_xs().child(format!(
                    "{}: {}",
                    view.tr("发现新版本"),
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
                            view.tr("下载更新"),
                            UiIntent::DownloadUpdate,
                        ))
                        .child(action_button(
                            cx,
                            theme,
                            "settings-update-release",
                            view.tr("打开发布页"),
                            UiIntent::OpenExternalUrl(release.page_url.clone()),
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
                    view.tr("取消下载"),
                    UiIntent::CancelUpdateDownload,
                ));
        }
        UpdatePhase::Ready {
            release, progress, ..
        } => {
            panel = panel
                .child(download_progress(theme, *progress))
                .child(div().text_xs().child(format!(
                    "{}: {}",
                    view.tr("下载完成"),
                    release.tag_name
                )))
                .child(action_button(
                    cx,
                    theme,
                    "settings-update-restart",
                    view.tr("重启完成更新"),
                    UiIntent::RestartUpdate,
                ));
        }
        UpdatePhase::Restarting => {
            panel = panel.child(
                div()
                    .text_xs()
                    .text_color(theme.muted)
                    .child(view.tr("正在重启完成更新...")),
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
                    view.tr("重新检查"),
                    UiIntent::CheckUpdates,
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

fn shortcut_setting_row(
    view: &UiProjection,
    cx: &mut Context<Root>,
    action: ShortcutAction,
) -> AnyElement {
    let theme = view.theme;
    let label = SharedString::from(view.tr(action.label()));
    let value = action.shortcut(&view.settings.shortcuts);
    let recording = matches!(
        view.pending,
        Some(PendingOp::EditShortcut(editing)) if *editing == action
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
                    this.dispatch_ui_intent(UiIntent::StartShortcutEdit(action.id()), window, cx);
                }))
                .child(shortcut_binding_view(
                    theme,
                    value,
                    recording,
                    view.language,
                )),
        )
        .into_any_element()
}
