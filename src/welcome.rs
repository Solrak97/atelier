use std::sync::Arc;

use gpui::{
    Context, Image, ImageFormat, MouseButton, MouseUpEvent, div, img, prelude::*, px, rgb,
};

use crate::shell::AppShell;

fn mark() -> impl IntoElement {
    img(Arc::new(Image::from_bytes(
        ImageFormat::Png,
        include_bytes!("../assets/icon.png").to_vec(),
    )))
    .size(px(48.0))
    .flex_none()
}

pub fn render(shell: &AppShell, cx: &mut Context<AppShell>) -> impl IntoElement {
    let projects = shell.recent_projects().to_vec();
    let message = shell.message().map(str::to_owned);
    let empty = projects.is_empty();
    let menu_overlay = shell.welcome_menu_overlay(cx);

    div()
        .id("welcome")
        .relative()
        .size_full()
        .flex()
        .flex_col()
        .items_center()
        .bg(rgb(0x111318))
        .text_color(rgb(0xd7dae0))
        .on_mouse_down(MouseButton::Right, |_, window, cx| {
            cx.stop_propagation();
            window.prevent_default();
        })
        .on_mouse_up(
            MouseButton::Right,
            cx.listener(|shell, event: &MouseUpEvent, window, cx| {
                shell.open_welcome_menu(event.position, window, cx);
            }),
        )
        .child(
            div()
                .w(px(560.0))
                .mt(px(72.0))
                .flex()
                .flex_col()
                .gap_4()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_3()
                        .child(mark())
                        .child(
                            div()
                                .text_size(px(28.0))
                                .text_color(rgb(0xf0f2f5))
                                .child("Atelier"),
                        ),
                )
                .child(
                    div().text_size(px(14.0)).text_color(rgb(0xaeb4bf)).child(
                        "Use File → Open Folder, or pick a recent project. Ctrl+O also works.",
                    ),
                )
                .when_some(message, |this, message| {
                    this.child(
                        div()
                            .text_size(px(13.0))
                            .text_color(rgb(0xe06c75))
                            .child(message),
                    )
                })
                .child(
                    div()
                        .mt_4()
                        .text_size(px(12.0))
                        .text_color(rgb(0x8f96a3))
                        .child(if empty {
                            "No recent projects".to_owned()
                        } else {
                            "Recent projects".to_owned()
                        }),
                )
                .children(projects.into_iter().enumerate().map(|(index, project)| {
                    div()
                        .id(("recent-project", index))
                        .h(px(52.0))
                        .px_3()
                        .flex()
                        .flex_col()
                        .justify_center()
                        .rounded_md()
                        .cursor_pointer()
                        .hover(|style| style.bg(rgb(0x1c2128)))
                        .child(
                            div()
                                .text_size(px(14.0))
                                .text_color(rgb(0xf0f2f5))
                                .child(project.name),
                        )
                        .child(
                            div()
                                .text_size(px(12.0))
                                .text_color(rgb(0x8f96a3))
                                .child(project.path.display().to_string()),
                        )
                        .on_mouse_up(
                            MouseButton::Left,
                            cx.listener(move |shell, _: &MouseUpEvent, window, cx| {
                                shell.open_recent(index, window, cx);
                            }),
                        )
                })),
        )
        .when_some(menu_overlay, |root, overlay| root.child(overlay))
}
