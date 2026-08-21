use super::super::*;
use super::icons::{Icon, icon};
use super::{MenuAction, UiIntent};

pub(crate) fn tab_name(cwd: &str) -> String {
    let name = file_name_of(cwd);
    if name.is_empty() {
        cwd.to_string()
    } else {
        name
    }
}

pub(crate) fn tab_button(
    cx: &mut Context<Root>,
    theme: Theme,
    i: usize,
    name: SharedString,
    active: bool,
    width: f32,
) -> AnyElement {
    div()
        .w(px(width))
        .flex_shrink_0()
        .px_2()
        .py_1()
        .bg(if active { theme.mantle } else { theme.surface0 })
        .border_1()
        .border_color(theme.border)
        .rounded_md()
        .cursor_pointer()
        .flex()
        .items_center()
        .gap_2()
        .id(SharedString::from(format!("tab-{}", i)))
        .hover(|style| style.bg(theme.hover))
        .on_click(cx.listener(move |this, _e, window, cx| {
            this.dispatch_ui_intent(UiIntent::SwitchTab(i), window, cx);
        }))
        .on_mouse_down(
            MouseButton::Middle,
            cx.listener(move |this, _e, window, cx| {
                this.dispatch_ui_intent(UiIntent::CloseTab(i), window, cx);
                cx.stop_propagation();
            }),
        )
        .child(
            div()
                .flex_1()
                .min_w(px(0.0))
                .text_sm()
                .whitespace_nowrap()
                .overflow_hidden()
                .text_ellipsis()
                .child(name),
        )
        .child(
            div()
                .w(px(18.0))
                .flex_shrink_0()
                .px_1()
                .cursor_pointer()
                .child(icon(Icon::Close, 13.0, theme.muted))
                .id(SharedString::from(format!("tab-close-{}", i)))
                .on_click(cx.listener(move |this, _e, window, cx| {
                    this.dispatch_ui_intent(UiIntent::CloseTab(i), window, cx);
                    cx.stop_propagation();
                })),
        )
        .into_any_element()
}

pub(crate) fn tab_scroll_button(
    cx: &mut Context<Root>,
    theme: Theme,
    id: &'static str,
    glyph: Icon,
    tooltip: impl Into<SharedString>,
    enabled: bool,
    intent: UiIntent,
) -> impl IntoElement {
    let tooltip = tooltip.into();
    div()
        .w(px(32.0))
        .h(px(30.0))
        .flex_shrink_0()
        .flex()
        .items_center()
        .justify_center()
        .bg(if enabled { theme.surface0 } else { theme.crust })
        .border_1()
        .border_color(theme.border)
        .rounded_sm()
        .text_base()
        .text_color(if enabled { theme.text } else { theme.muted })
        .cursor(if enabled {
            CursorStyle::PointingHand
        } else {
            CursorStyle::Arrow
        })
        .id(id)
        .hover(|style| style.bg(theme.hover))
        .tooltip(move |_window, cx| {
            cx.new(|_| TooltipView {
                text: tooltip.clone(),
                theme,
            })
            .into()
        })
        .on_click(cx.listener(move |this, _event, window, cx| {
            if enabled {
                this.dispatch_ui_intent(intent.clone(), window, cx);
            }
            cx.stop_propagation();
        }))
        .child(icon(
            glyph,
            16.0,
            if enabled { theme.text } else { theme.muted },
        ))
}

struct TooltipView {
    text: SharedString,
    theme: Theme,
}

impl Render for TooltipView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .px_2()
            .py_1()
            .bg(self.theme.crust)
            .border_1()
            .border_color(self.theme.border)
            .rounded_sm()
            .text_xs()
            .text_color(self.theme.text)
            .child(self.text.clone())
    }
}

pub(crate) fn icon_button(
    cx: &mut Context<Root>,
    theme: Theme,
    id: &'static str,
    glyph: Icon,
    tooltip: impl Into<SharedString>,
    intent: UiIntent,
) -> impl IntoElement {
    let tooltip = tooltip.into();
    div()
        .w(px(32.0))
        .h(px(30.0))
        .flex()
        .items_center()
        .justify_center()
        .bg(theme.surface0)
        .border_1()
        .border_color(theme.border)
        .rounded_sm()
        .cursor_pointer()
        .text_base()
        .child(icon(
            glyph,
            16.0,
            match glyph {
                Icon::Trash => theme.danger,
                Icon::StarFilled => theme.blue,
                _ => theme.text,
            },
        ))
        .id(id)
        .hover(|style| style.bg(theme.hover))
        .tooltip(move |_window, cx| {
            cx.new(|_| TooltipView {
                text: tooltip.clone(),
                theme,
            })
            .into()
        })
        .on_click(cx.listener(move |this, _event, window, cx| {
            this.dispatch_ui_intent(intent.clone(), window, cx);
            cx.stop_propagation();
        }))
}

pub(crate) fn window_control_button(
    cx: &mut Context<Root>,
    theme: Theme,
    id: &'static str,
    glyph: Icon,
    tooltip: impl Into<SharedString>,
    intent: UiIntent,
) -> impl IntoElement {
    let tooltip = tooltip.into();
    div()
        .w(px(46.0))
        .h_full()
        .flex()
        .items_center()
        .justify_center()
        .text_base()
        .text_color(theme.text)
        .id(id)
        .hover(|style| style.bg(theme.hover))
        .tooltip(move |_window, cx| {
            cx.new(|_| TooltipView {
                text: tooltip.clone(),
                theme,
            })
            .into()
        })
        .on_click(cx.listener(move |this, _event, window, cx| {
            this.dispatch_ui_intent(intent.clone(), window, cx);
            cx.stop_propagation();
        }))
        .child(icon(glyph, 16.0, theme.text))
}

pub(crate) fn resize_handle(
    cx: &Context<Root>,
    theme: Theme,
    target: ResizeTarget,
) -> impl IntoElement {
    div()
        .w(px(6.0))
        .h_full()
        .flex_shrink_0()
        .bg(theme.border)
        .cursor(CursorStyle::ResizeLeftRight)
        .hover(|style| style.bg(theme.blue))
        .id(SharedString::from(format!("resize-{target:?}")))
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |this, event: &MouseDownEvent, _window, cx| {
                this.dispatch_ui_intent(
                    UiIntent::BeginResize {
                        target,
                        x: f32::from(event.position.x),
                    },
                    _window,
                    cx,
                );
                cx.stop_propagation();
            }),
        )
        .on_mouse_up_out(
            MouseButton::Left,
            cx.listener(|this, _event, window, cx| {
                this.dispatch_ui_intent(UiIntent::EndResize, window, cx)
            }),
        )
        .on_drag(target, |_target: &ResizeTarget, _position, _window, cx| {
            cx.new(|_| ResizeGhost)
        })
        .on_drag_move(cx.listener(
            move |this, event: &DragMoveEvent<ResizeTarget>, _window, cx| {
                this.dispatch_ui_intent(
                    UiIntent::MoveResize(f32::from(event.event.position.x)),
                    _window,
                    cx,
                );
            },
        ))
}

pub(crate) fn new_tab_button(
    cx: &mut Context<Root>,
    theme: Theme,
    language: Language,
) -> impl IntoElement {
    icon_button(
        cx,
        theme,
        "btn-new-tab",
        Icon::Add,
        settings::translate(language, "新建标签页"),
        UiIntent::NewTab,
    )
}

pub(crate) fn refresh_button(
    cx: &mut Context<Root>,
    theme: Theme,
    language: Language,
) -> impl IntoElement {
    icon_button(
        cx,
        theme,
        "btn-refresh",
        Icon::Refresh,
        settings::translate(language, "刷新"),
        UiIntent::Refresh,
    )
}

pub(crate) fn back_button(
    cx: &mut Context<Root>,
    theme: Theme,
    language: Language,
    enabled: bool,
) -> impl IntoElement {
    tab_scroll_button(
        cx,
        theme,
        "btn-back",
        Icon::ArrowLeft,
        settings::translate(language, "后退"),
        enabled,
        UiIntent::Back,
    )
}

pub(crate) fn forward_button(
    cx: &mut Context<Root>,
    theme: Theme,
    language: Language,
    enabled: bool,
) -> impl IntoElement {
    tab_scroll_button(
        cx,
        theme,
        "btn-forward",
        Icon::ArrowRight,
        settings::translate(language, "前进"),
        enabled,
        UiIntent::Forward,
    )
}

pub(crate) fn parent_button(
    cx: &mut Context<Root>,
    theme: Theme,
    language: Language,
) -> impl IntoElement {
    icon_button(
        cx,
        theme,
        "btn-parent",
        Icon::ArrowUp,
        settings::translate(language, "上级"),
        UiIntent::Parent,
    )
}

pub(crate) fn computer_button(
    cx: &mut Context<Root>,
    theme: Theme,
    language: Language,
) -> impl IntoElement {
    icon_button(
        cx,
        theme,
        "btn-computer",
        Icon::Computer,
        settings::translate(language, "此电脑"),
        UiIntent::Computer,
    )
}

pub(crate) fn action_button(
    cx: &mut Context<Root>,
    theme: Theme,
    id: &'static str,
    label: impl Into<SharedString>,
    intent: UiIntent,
) -> impl IntoElement {
    div()
        .px_2()
        .py_1()
        .bg(theme.surface0)
        .border_1()
        .border_color(theme.border)
        .rounded_md()
        .cursor_pointer()
        .text_sm()
        .child(label.into())
        .id(id)
        .hover(|style| style.bg(theme.hover))
        .on_click(cx.listener(move |this, _event, window, cx| {
            this.dispatch_ui_intent(intent.clone(), window, cx);
        }))
}

pub(crate) fn toolbar_divider(theme: Theme) -> impl IntoElement {
    div().h(px(20.0)).w(px(1.0)).bg(theme.border)
}

pub(crate) fn shortcut_binding_view(
    theme: Theme,
    value: &str,
    recording: bool,
    language: Language,
) -> AnyElement {
    if recording {
        return div()
            .text_xs()
            .text_color(theme.blue)
            .child(settings::translate(language, "按下快捷键..."))
            .into_any_element();
    }
    let value = normalize_shortcut(value);
    if value.is_empty() {
        return div()
            .text_sm()
            .text_color(theme.muted)
            .child("—")
            .into_any_element();
    }
    div()
        .flex()
        .items_center()
        .gap_1()
        .children(value.split('+').map(|part| {
            div()
                .px_1()
                .py(px(1.0))
                .bg(theme.mantle)
                .rounded_sm()
                .text_xs()
                .child(part.to_ascii_uppercase())
        }))
        .into_any_element()
}

pub(crate) fn dialog_button(
    cx: &mut Context<Root>,
    theme: Theme,
    id: &'static str,
    label: impl Into<SharedString>,
    intent: UiIntent,
) -> impl IntoElement {
    div()
        .px_2()
        .py_1()
        .bg(theme.surface0)
        .rounded_md()
        .cursor_pointer()
        .text_sm()
        .child(label.into())
        .id(id)
        .on_click(cx.listener(move |this, _event, window, cx| {
            this.dispatch_ui_intent(intent.clone(), window, cx);
            cx.stop_propagation();
        }))
}

pub(crate) fn settings_line(
    cx: &mut Context<Root>,
    theme: Theme,
    label: String,
    value: String,
    id: &'static str,
    intent: UiIntent,
) -> AnyElement {
    div()
        .w_full()
        .py_1()
        .flex()
        .items_center()
        .gap_2()
        .child(div().flex_1().text_sm().child(label))
        .child(action_button(cx, theme, id, value, intent))
        .into_any_element()
}

pub(crate) fn menu_items_for(
    target: &Option<String>,
    computer_view: bool,
    target_is_dir: bool,
    is_favorite: bool,
    language: Language,
) -> Vec<(String, MenuAction)> {
    if computer_view {
        return target
            .as_ref()
            .map(|_| {
                vec![
                    (settings::translate(language, "打开"), MenuAction::Open),
                    (
                        settings::translate(
                            language,
                            if is_favorite {
                                "取消收藏"
                            } else {
                                "收藏"
                            },
                        ),
                        MenuAction::Favorite,
                    ),
                ]
            })
            .unwrap_or_default();
    }
    match target {
        Some(_) => {
            let mut items = vec![
                (settings::translate(language, "打开"), MenuAction::Open),
                (settings::translate(language, "重命名"), MenuAction::Rename),
                (settings::translate(language, "删除"), MenuAction::Delete),
                (settings::translate(language, "复制"), MenuAction::Copy),
                (settings::translate(language, "剪切"), MenuAction::Cut),
                (settings::translate(language, "粘贴"), MenuAction::Paste),
                (
                    settings::translate(language, "新建文件"),
                    MenuAction::NewFile,
                ),
                (
                    settings::translate(language, "新建文件夹"),
                    MenuAction::NewDir,
                ),
            ];
            if target_is_dir {
                items.insert(
                    1,
                    (
                        settings::translate(
                            language,
                            if is_favorite {
                                "取消收藏"
                            } else {
                                "收藏"
                            },
                        ),
                        MenuAction::Favorite,
                    ),
                );
            }
            items
        }
        None => vec![
            (settings::translate(language, "粘贴"), MenuAction::Paste),
            (
                settings::translate(language, "新建文件"),
                MenuAction::NewFile,
            ),
            (
                settings::translate(language, "新建文件夹"),
                MenuAction::NewDir,
            ),
        ],
    }
}

pub(crate) fn menu_item(
    cx: &mut Context<Root>,
    theme: Theme,
    label: String,
    action: MenuAction,
) -> AnyElement {
    let id = SharedString::from(label.clone());
    div()
        .px_3()
        .py_1()
        .cursor_pointer()
        .hover(|s| s.bg(theme.surface0))
        .text_sm()
        .child(label)
        .id(id)
        .on_click(cx.listener(move |this, _event, window, cx| {
            this.dispatch_ui_intent(UiIntent::ExecuteMenu(action), window, cx);
            cx.stop_propagation();
        }))
        .into_any_element()
}

pub(crate) fn sort_header(
    cx: &Context<Root>,
    theme: Theme,
    label: impl Into<SharedString>,
    field: SortField,
    sort: SortState,
    width: f32,
) -> AnyElement {
    let label: SharedString = label.into();
    let active = sort.field == field;
    let sort_icon = if active {
        Some(match sort.direction {
            SortDirection::Ascending => Icon::ArrowUp,
            SortDirection::Descending => Icon::ArrowDown,
        })
    } else {
        None
    };
    let mut header = div()
        .w(px(width))
        .flex_shrink_0()
        .flex()
        .items_center()
        .gap_1()
        .cursor_pointer()
        .text_xs()
        .whitespace_nowrap()
        .overflow_hidden()
        .text_color(if active { theme.text } else { theme.muted })
        .id(SharedString::from(format!("sort-{:?}", field)))
        .on_click(cx.listener(move |this, _event, window, cx| {
            this.dispatch_ui_intent(UiIntent::ToggleSort(field), window, cx);
            cx.stop_propagation();
        }))
        .child(label);
    if let Some(sort_icon) = sort_icon {
        header = header.child(icon(sort_icon, 12.0, theme.text));
    }
    header.into_any_element()
}

fn column_spacer() -> impl IntoElement {
    div().w(px(6.0)).flex_shrink_0()
}

pub(crate) fn file_row(
    cx: &mut Context<Root>,
    theme: Theme,
    layout: LayoutState,
    name: String,
    is_dir: bool,
    size: u64,
    mtime: f64,
    selected: bool,
    inline: bool,
    inline_input: Option<AnyElement>,
) -> impl IntoElement {
    let entry_icon = file_icon(&name, is_dir);
    let display_name = SharedString::from(if is_dir {
        format!("{}/", name)
    } else {
        name.clone()
    });
    let modified = SharedString::from(format_mtime(mtime));
    let meta = SharedString::from(if is_dir {
        String::new()
    } else {
        human_size(size)
    });
    let name_color = if is_dir { theme.blue } else { theme.text };
    let click_name = name.clone();
    let right_name = name.clone();
    let row_id = if inline {
        if is_dir {
            "inline-new-dir".to_string()
        } else if name.is_empty() {
            "inline-new-file".to_string()
        } else {
            format!("inline-rename-{}", name)
        }
    } else {
        name.clone()
    };
    let name_cell = if let Some(input) = inline_input {
        div()
            .w(px(layout.name_width))
            .flex_shrink_0()
            .px_1()
            .bg(theme.surface0)
            .rounded_sm()
            .child(input)
    } else {
        div()
            .w(px(layout.name_width))
            .flex_shrink_0()
            .flex()
            .items_center()
            .gap_1()
            .text_sm()
            .whitespace_nowrap()
            .overflow_hidden()
            .text_color(name_color)
            .child(icon(entry_icon, 16.0, name_color))
            .child(
                div()
                    .min_w(px(0.0))
                    .overflow_hidden()
                    .text_ellipsis()
                    .child(display_name),
            )
    };

    let mut row = div()
        .w_full()
        .px_3()
        .py_1()
        .flex()
        .gap_0()
        .bg(if selected { theme.selected } else { theme.base })
        .cursor_pointer()
        .id(SharedString::from(row_id));
    if !inline {
        row = row
            .hover(|style| {
                style.bg(if selected {
                    theme.selected
                } else {
                    theme.hover
                })
            })
            .on_click(cx.listener(move |this, event: &ClickEvent, window, cx| {
                let modifiers = event.modifiers();
                let click_count = event.click_count();
                this.dispatch_ui_intent(
                    UiIntent::ClickFile {
                        name: click_name.clone(),
                        is_dir,
                        modifiers,
                        click_count,
                    },
                    window,
                    cx,
                );
                cx.stop_propagation();
            }))
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                    this.dispatch_ui_intent(
                        UiIntent::OpenMenu {
                            target: Some(right_name.clone()),
                            position: event.position,
                        },
                        window,
                        cx,
                    );
                    cx.stop_propagation();
                }),
            );
    }
    row.child(name_cell)
        .child(column_spacer())
        .child(
            div()
                .w(px(layout.modified_width))
                .flex_shrink_0()
                .text_xs()
                .whitespace_nowrap()
                .overflow_hidden()
                .text_color(theme.muted)
                .child(modified),
        )
        .child(column_spacer())
        .child(
            div()
                .w(px(layout.size_width))
                .flex_shrink_0()
                .text_xs()
                .whitespace_nowrap()
                .overflow_hidden()
                .text_right()
                .text_color(theme.muted)
                .child(meta),
        )
        .child(column_spacer())
}
