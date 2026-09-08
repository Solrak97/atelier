use std::rc::Rc;

use gpui::{
    Anchor, AnchoredPositionMode, Context, MouseButton, MouseDownEvent, Pixels, Point, Render,
    Window, anchored, deferred, div, point, prelude::*, px, rgb,
};

pub struct ContextMenu {
    pub position: Point<Pixels>,
    pub items: Vec<&'static str>,
}

pub fn overlay<V, Dismiss, Select>(
    menu: ContextMenu,
    cx: &mut Context<V>,
    on_dismiss: Dismiss,
    on_select: Select,
) -> impl IntoElement + use<V, Dismiss, Select>
where
    V: Render,
    Dismiss: Fn(&mut V, &mut Context<V>) + 'static,
    Select: Fn(&mut V, usize, &mut Window, &mut Context<V>) + 'static,
{
    deferred(
        anchored()
            .position(menu.position)
            .snap_to_window_with_margin(px(8.0))
            .child(panel(
                menu.items,
                MenuKind::Popup,
                cx,
                on_dismiss,
                on_select,
            )),
    )
    .with_priority(1)
}

pub fn dropdown<V, Dismiss, Select>(
    items: Vec<&'static str>,
    y_offset: Pixels,
    cx: &mut Context<V>,
    on_dismiss: Dismiss,
    on_select: Select,
) -> impl IntoElement + use<V, Dismiss, Select>
where
    V: Render,
    Dismiss: Fn(&mut V, &mut Context<V>) + 'static,
    Select: Fn(&mut V, usize, &mut Window, &mut Context<V>) + 'static,
{
    deferred(
        anchored()
            .anchor(Anchor::TopLeft)
            .position_mode(AnchoredPositionMode::Local)
            .offset(point(px(0.0), y_offset))
            .snap_to_window()
            .child(panel(items, MenuKind::List, cx, on_dismiss, on_select)),
    )
    .with_priority(1)
}

enum MenuKind {
    Popup,
    List,
}

fn panel<V, Dismiss, Select>(
    items: Vec<&'static str>,
    kind: MenuKind,
    cx: &mut Context<V>,
    on_dismiss: Dismiss,
    on_select: Select,
) -> impl IntoElement + use<V, Dismiss, Select>
where
    V: Render,
    Dismiss: Fn(&mut V, &mut Context<V>) + 'static,
    Select: Fn(&mut V, usize, &mut Window, &mut Context<V>) + 'static,
{
    let on_dismiss = Rc::new(on_dismiss);
    let on_select = Rc::new(on_select);
    let is_list = matches!(kind, MenuKind::List);

    let mut action_index = 0usize;
    let mut children = Vec::new();
    for label in items {
        if is_list && label == "-" {
            children.push(
                div()
                    .h(px(9.0))
                    .w_full()
                    .flex()
                    .items_center()
                    .px_2()
                    .child(div().h(px(1.0)).w_full().bg(rgb(0x2a2e35)))
                    .into_any_element(),
            );
            continue;
        }

        let index = action_index;
        action_index += 1;
        let on_select = on_select.clone();
        children.push(
            div()
                .id(("context-menu-item", index))
                .w_full()
                .h(px(if is_list { 26.0 } else { 28.0 }))
                .px_3()
                .flex()
                .items_center()
                .text_size(px(13.0))
                .cursor_pointer()
                .hover(|style| style.bg(rgb(0x2b6cb0)).text_color(rgb(0xf0f2f5)))
                .child(label)
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_mouse_up(
                    MouseButton::Left,
                    cx.listener(move |view, _, window, cx| {
                        cx.stop_propagation();
                        on_select(view, index, window, cx);
                    }),
                )
                .into_any_element(),
        );
    }

    div()
        .id("context-menu")
        .occlude()
        .debug_selector(|| "CONTEXT_MENU".into())
        .flex()
        .flex_col()
        .min_w(px(if is_list { 220.0 } else { 196.0 }))
        .when(is_list, |menu| {
            menu.py_1()
                .border_l_1()
                .border_r_1()
                .border_b_1()
                .border_color(rgb(0x2a2e35))
                .bg(rgb(0x1c2128))
        })
        .when(!is_list, |menu| {
            menu.py_1()
                .rounded_md()
                .border_1()
                .border_color(rgb(0x2a2e35))
                .bg(rgb(0x1c2128))
                .shadow_md()
        })
        .text_color(rgb(0xd7dae0))
        .on_mouse_down_out({
            let on_dismiss = on_dismiss.clone();
            cx.listener(move |view, _: &MouseDownEvent, _, cx| {
                on_dismiss(view, cx);
            })
        })
        .children(children)
}
