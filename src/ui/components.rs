use super::super::*;

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
) -> AnyElement {
    div()
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
        .on_click(cx.listener(move |this, _e, _w, cx| {
            this.switch_tab(i, cx);
        }))
        .on_mouse_down(
            MouseButton::Middle,
            cx.listener(move |this, _e, _w, cx| {
                this.close_tab(i, cx);
                cx.stop_propagation();
            }),
        )
        .child(div().text_sm().child(name))
        .child(
            div()
                .px_1()
                .cursor_pointer()
                .child("×")
                .id(SharedString::from(format!("tab-close-{}", i)))
                .on_click(cx.listener(move |this, _e, _w, cx| {
                    this.close_tab(i, cx);
                    cx.stop_propagation();
                })),
        )
        .into_any_element()
}

pub(crate) fn new_tab_button(cx: &mut Context<Root>, theme: Theme) -> impl IntoElement {
    div()
        .px_2()
        .py_1()
        .bg(theme.surface0)
        .border_1()
        .border_color(theme.border)
        .rounded_md()
        .cursor_pointer()
        .text_sm()
        .hover(|style| style.bg(theme.hover))
        .child("+")
        .id("btn-new-tab")
        .on_click(cx.listener(|this, _e, _w, cx| {
            this.new_tab(cx);
        }))
}

pub(crate) fn refresh_button(
    cx: &mut Context<Root>,
    theme: Theme,
    language: Language,
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
        .child(settings::translate(language, "刷新"))
        .id("btn-refresh")
        .hover(|style| style.bg(theme.hover))
        .on_click(cx.listener(|this, _event, _window, cx| {
            this.refresh_current(cx);
        }))
}

pub(crate) fn parent_button(
    cx: &mut Context<Root>,
    theme: Theme,
    language: Language,
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
        .child(settings::translate(language, "上级 .."))
        .id("btn-parent")
        .hover(|style| style.bg(theme.hover))
        .on_click(cx.listener(|this, _event, _window, cx| {
            this.go_parent(cx);
        }))
}

pub(crate) fn computer_button(
    cx: &mut Context<Root>,
    theme: Theme,
    language: Language,
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
        .child(settings::translate(language, "此电脑"))
        .id("btn-computer")
        .hover(|style| style.bg(theme.hover))
        .on_click(cx.listener(|this, _event, _window, cx| {
            this.show_computer_view(cx);
        }))
}

pub(crate) fn action_button(
    cx: &mut Context<Root>,
    theme: Theme,
    id: &'static str,
    label: impl Into<SharedString>,
    on_click: impl Fn(&mut Root, &mut Window, &mut Context<Root>) + 'static,
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
            on_click(this, window, cx);
        }))
}

pub(crate) fn command_button(
    cx: &mut Context<Root>,
    theme: Theme,
    id: &'static str,
    icon: &'static str,
    label: impl Into<SharedString>,
    on_click: impl Fn(&mut Root, &mut Window, &mut Context<Root>) + 'static,
) -> impl IntoElement {
    div()
        .px_2()
        .py_1()
        .bg(theme.base)
        .border_1()
        .border_color(theme.border)
        .rounded_sm()
        .cursor_pointer()
        .flex()
        .items_center()
        .gap_1()
        .text_sm()
        .child(div().text_base().child(icon))
        .child(label.into())
        .id(id)
        .hover(|style| style.bg(theme.hover))
        .on_click(cx.listener(move |this, _event, window, cx| {
            on_click(this, window, cx);
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
    on_click: impl Fn(&mut Root, &mut Window, &mut Context<Root>) + 'static,
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
            on_click(this, window, cx);
            cx.stop_propagation();
        }))
}

pub(crate) fn settings_line(
    cx: &mut Context<Root>,
    theme: Theme,
    label: String,
    value: String,
    id: &'static str,
    on_click: impl Fn(&mut Root, &mut Window, &mut Context<Root>) + 'static,
) -> AnyElement {
    div()
        .w_full()
        .py_1()
        .flex()
        .items_center()
        .gap_2()
        .child(div().flex_1().text_sm().child(label))
        .child(action_button(cx, theme, id, value, on_click))
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
            this.exec_menu_action(action, window, cx);
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
    flexible: bool,
    width: f32,
) -> AnyElement {
    let label: SharedString = label.into();
    let active = sort.field == field;
    let arrow = if active {
        match sort.direction {
            SortDirection::Ascending => " ↑",
            SortDirection::Descending => " ↓",
        }
    } else {
        ""
    };
    let mut element = div()
        .cursor_pointer()
        .text_xs()
        .text_color(if active { theme.text } else { theme.muted })
        .id(SharedString::from(format!("sort-{:?}", field)))
        .on_click(cx.listener(move |this, _event, _window, cx| {
            this.toggle_sort(field, cx);
            cx.stop_propagation();
        }))
        .child(SharedString::from(format!("{}{}", label, arrow)));
    if flexible {
        element = element.flex_1();
    } else {
        element = element.w(px(width));
    }
    element.into_any_element()
}

pub(crate) fn file_row(
    cx: &mut Context<Root>,
    theme: Theme,
    name: String,
    is_dir: bool,
    size: u64,
    mtime: f64,
    selected: bool,
    inline: bool,
    inline_input: Option<AnyElement>,
) -> impl IntoElement {
    let icon = file_icon(&name, is_dir);
    let display = SharedString::from(if is_dir {
        format!("{} {}/", icon, name)
    } else {
        format!("{} {}", icon, name)
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
            .flex_1()
            .px_1()
            .bg(theme.surface0)
            .rounded_sm()
            .child(input)
    } else {
        div()
            .flex_1()
            .text_sm()
            .text_color(name_color)
            .child(display)
    };

    let mut row = div()
        .w_full()
        .px_3()
        .py_1()
        .flex()
        .justify_between()
        .gap_3()
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
            .on_click(cx.listener(move |this, event: &ClickEvent, _window, cx| {
                let modifiers = event.modifiers();
                let click_count = event.click_count();
                this.click_file(&click_name, is_dir, modifiers, click_count, cx);
                cx.stop_propagation();
            }))
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |this, event: &MouseDownEvent, _window, cx| {
                    this.open_menu(Some(right_name.clone()), event.position, cx);
                    cx.stop_propagation();
                }),
            );
    }
    row.child(name_cell)
        .child(
            div()
                .w(px(135.0))
                .text_xs()
                .text_color(theme.muted)
                .child(modified),
        )
        .child(
            div()
                .w(px(80.0))
                .text_xs()
                .text_color(theme.muted)
                .child(meta),
        )
}
