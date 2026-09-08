use gpui::{Context, MouseButton, MouseUpEvent, div, prelude::*, px, rgb};

use crate::shell::AppShell;

pub fn render(shell: &AppShell, cx: &mut Context<AppShell>) -> impl IntoElement {
    let projects = shell.recent_projects().to_vec();
    let message = shell.message().map(str::to_owned);
    let empty = projects.is_empty();

    div()
        .size_full()
        .flex()
        .flex_col()
        .items_center()
        .bg(rgb(0x111318))
        .text_color(rgb(0xd7dae0))
        .child(
            div()
                .w(px(560.0))
                .mt(px(72.0))
                .flex()
                .flex_col()
                .gap_4()
                .child(
                    div()
                        .text_size(px(28.0))
                        .text_color(rgb(0xf0f2f5))
                        .child("Atelier"),
                )
                .child(
                    div()
                        .text_size(px(14.0))
                        .text_color(rgb(0xaeb4bf))
                        .child("Open a folder to start editing, or pick a recent project."),
                )
                .child(
                    div()
                        .id("open-folder")
                        .h(px(34.0))
                        .px_3()
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded_md()
                        .bg(rgb(0x2b6cb0))
                        .text_size(px(13.0))
                        .cursor_pointer()
                        .hover(|style| style.bg(rgb(0x3182ce)))
                        .child("Open Folder")
                        .on_mouse_up(
                            MouseButton::Left,
                            cx.listener(|shell, _: &MouseUpEvent, window, cx| {
                                shell.open_folder(window, cx);
                            }),
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
}
