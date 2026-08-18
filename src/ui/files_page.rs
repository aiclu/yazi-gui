use super::super::*;
use super::components::{file_row, resize_handle, sort_header};

pub(crate) fn file_list(root: &Root, cx: &Context<Root>) -> impl IntoElement {
    let theme = root.theme;
    let layout = root.layout;
    let computer_view = root.cur().computer_view;
    let search_active = root
        .cur()
        .search
        .as_ref()
        .is_some_and(|search| !search.query.is_empty());

    let mut entries: Vec<(String, bool, u64, f64, bool)> = if computer_view {
        let mut drive_entries: Vec<FileEntry> = root
            .drive_roots
            .iter()
            .map(|name| FileEntry {
                name: name.clone(),
                is_dir: true,
                is_hidden: false,
                size: 0,
                mtime: 0.0,
            })
            .collect();
        sort_files(&mut drive_entries, root.cur().sort);
        drive_entries
            .into_iter()
            .map(|file| (file.name, file.is_dir, file.size, file.mtime, false))
            .collect()
    } else if search_active {
        root.cur()
            .search
            .as_ref()
            .map(|search| {
                search
                    .results
                    .iter()
                    .map(|file| (file.name.clone(), file.is_dir, file.size, file.mtime, false))
                    .collect()
            })
            .unwrap_or_default()
    } else {
        root.cur()
            .files
            .iter()
            .map(|f| (f.name.clone(), f.is_dir, f.size, f.mtime, false))
            .collect()
    };
    let new_item = matches!(root.pending, Some(PendingOp::NewFile | PendingOp::NewDir));
    if new_item {
        entries.push((String::new(), false, 0, 0.0, true));
    }

    let mut horizontal_viewport = div()
        .flex_1()
        .w_full()
        .min_w(px(0.0))
        .h_full()
        .id("file-horizontal-viewport")
        .overflow_x_scroll()
        .scrollbar_width(px(0.0))
        .track_scroll(&root.file_scroll);
    horizontal_viewport.style().restrict_scroll_to_axis = Some(true);

    let table = div()
        .w_full()
        .min_w(px(layout.file_list_min_width()))
        .flex_shrink_0()
        .h_full()
        .flex()
        .flex_col()
        .child(file_header(root, cx, theme))
        .child(
            div().flex_1().w_full().h_full().child(
                uniform_list(
                    "file-list-items",
                    entries.len(),
                    cx.processor(move |this, range: Range<usize>, _window, list_cx| {
                        range
                            .filter_map(|ix| {
                                let (name, is_dir, size, mtime, new_item) =
                                    entries.get(ix)?.clone();
                                let selected = this.cur().selected.iter().any(|item| item == &name);
                                let inline_rename = matches!(
                                    this.pending.as_ref(),
                                    Some(PendingOp::Rename { path })
                                        if Path::new(path)
                                            == &Path::new(&this.cur().cwd).join(&name)
                                );
                                let inline = new_item || inline_rename;
                                let inline_input = inline.then(|| {
                                    let placeholder = if new_item
                                        && matches!(this.pending, Some(PendingOp::NewDir))
                                    {
                                        this.tr("输入名称...")
                                    } else {
                                        this.tr("输入名称...")
                                    };
                                    this.input_field(list_cx, placeholder)
                                });
                                Some(
                                    file_row(
                                        list_cx,
                                        theme,
                                        layout,
                                        name,
                                        is_dir,
                                        size,
                                        mtime,
                                        selected,
                                        inline,
                                        inline_input,
                                    )
                                    .into_any_element(),
                                )
                            })
                            .collect()
                    }),
                )
                .h_full(),
            ),
        );
    horizontal_viewport = horizontal_viewport.child(table);

    div()
        .flex_1()
        .min_w(px(0.0))
        .h_full()
        .flex()
        .flex_col()
        .id("file-list")
        .on_click(cx.listener(|this, _event, _window, cx| {
            this.click_blank(cx);
        }))
        .on_mouse_down(
            MouseButton::Right,
            cx.listener(|this, event: &MouseDownEvent, _window, cx| {
                this.open_menu(None, event.position, cx);
                cx.stop_propagation();
            }),
        )
        .child(horizontal_viewport)
        .child(horizontal_scrollbar(root, cx, theme))
}

fn horizontal_scrollbar(root: &Root, cx: &Context<Root>, theme: Theme) -> AnyElement {
    let viewport_width = f32::from(root.file_scroll.bounds().size.width);
    let max_offset = f32::from(root.file_scroll.max_offset().width);
    let Some((thumb_width, travel)) = horizontal_scrollbar_metrics(viewport_width, max_offset)
    else {
        return div().h(px(0.0)).flex_shrink_0().into_any_element();
    };
    let offset = (-f32::from(root.file_scroll.offset().x)).clamp(0.0, max_offset);
    let thumb_left = if max_offset == 0.0 {
        0.0
    } else {
        offset / max_offset * travel
    };
    let thumb = div()
        .absolute()
        .left(px(thumb_left))
        .top(px(2.0))
        .w(px(thumb_width))
        .h(px(8.0))
        .bg(theme.border)
        .rounded_sm()
        .cursor(CursorStyle::ResizeLeftRight)
        .hover(|style| style.bg(theme.blue))
        .id("file-horizontal-scroll-thumb")
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(|this, event: &MouseDownEvent, _window, cx| {
                this.begin_file_scroll_drag(f32::from(event.position.x), cx);
                cx.stop_propagation();
            }),
        )
        .on_mouse_up_out(
            MouseButton::Left,
            cx.listener(|this, _event, _window, cx| this.end_file_scroll_drag(cx)),
        )
        .on_drag(
            HorizontalScrollDrag,
            |_drag: &HorizontalScrollDrag, _position, _window, cx| {
                cx.new(|_| HorizontalScrollGhost)
            },
        )
        .on_drag_move(cx.listener(
            |this, event: &DragMoveEvent<HorizontalScrollDrag>, _window, cx| {
                this.move_file_scroll_drag(f32::from(event.event.position.x), cx);
            },
        ));

    div()
        .w_full()
        .h(px(12.0))
        .flex_shrink_0()
        .bg(theme.crust)
        .border_t_1()
        .border_color(theme.border)
        .child(thumb)
        .into_any_element()
}

pub(crate) fn file_header(root: &Root, cx: &Context<Root>, theme: Theme) -> AnyElement {
    let sort = root.cur().sort;
    let layout = root.layout;
    div()
        .w_full()
        .px_3()
        .py_1()
        .bg(theme.mantle)
        .border_1()
        .border_color(theme.border)
        .flex()
        .items_center()
        .gap_0()
        .child(sort_header(
            cx,
            theme,
            root.tr("名称"),
            SortField::Name,
            sort,
            layout.name_width,
        ))
        .child(resize_handle(cx, theme, ResizeTarget::NameColumn))
        .child(sort_header(
            cx,
            theme,
            root.tr("修改时间"),
            SortField::Modified,
            sort,
            layout.modified_width,
        ))
        .child(resize_handle(cx, theme, ResizeTarget::ModifiedColumn))
        .child(sort_header(
            cx,
            theme,
            root.tr("大小"),
            SortField::Size,
            sort,
            layout.size_width,
        ))
        .child(resize_handle(cx, theme, ResizeTarget::SizeColumn))
        .into_any_element()
}

pub(crate) fn preview_pane(root: &Root) -> impl IntoElement {
    let tab = root.cur();
    let title = SharedString::from(match tab.selected.len() {
        0 => root.tr("预览"),
        1 => tab.selected[0].clone(),
        n => format!("{} {} {}", root.tr("已选"), n, root.tr("项")),
    });

    div()
        .w(px(root.layout.preview_width))
        .flex_shrink_0()
        .h_full()
        .flex()
        .flex_col()
        .bg(root.theme.mantle)
        .border_1()
        .border_color(root.theme.border)
        .child(
            div()
                .px_3()
                .py_2()
                .bg(root.theme.crust)
                .border_1()
                .border_color(root.theme.border)
                .text_sm()
                .text_color(root.theme.muted)
                .child(title),
        )
        .child(
            div()
                .w_full()
                .flex_shrink_0()
                .h_full()
                .id("preview-content")
                .overflow_y_scroll()
                .px_3()
                .py_2()
                .child(preview_body(root)),
        )
}

pub(crate) fn preview_body(root: &Root) -> AnyElement {
    match &root.cur().preview {
        Preview::Empty => div()
            .text_sm()
            .text_color(root.theme.muted)
            .child(root.tr("单击选中文件以预览"))
            .into_any_element(),
        Preview::Loading => div()
            .text_sm()
            .text_color(root.theme.muted)
            .child(root.tr("加载中..."))
            .into_any_element(),
        Preview::Dir => div().text_sm().child(root.tr("目录")).into_any_element(),
        Preview::Binary { size } => div()
            .text_sm()
            .child(SharedString::from(format!(
                "{} · {}",
                root.tr("二进制文件"),
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
            .child(StyledText::new(text.clone()).with_highlights(highlights.clone()))
            .into_any_element(),
        Preview::Text(text) => div()
            .text_xs()
            .child(SharedString::from(text.clone()))
            .into_any_element(),
    }
}
