use gpui::{
    Context, Decorations, MouseButton, Point, ResizeEdge, Size, Window, WindowControlArea, div,
    prelude::*, px, rgb,
};

use crate::context_menu;
use crate::shell::AppShell;

pub const TITLEBAR_HEIGHT: f32 = 36.0;
const RESIZE_EDGE: f32 = 5.0;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum MenuBarMenu {
    File,
    Edit,
    View,
    Help,
}

impl MenuBarMenu {
    fn label(self) -> &'static str {
        match self {
            Self::File => "File",
            Self::Edit => "Edit",
            Self::View => "View",
            Self::Help => "Help",
        }
    }

    fn items(self) -> Vec<&'static str> {
        match self {
            Self::File => vec![
                "Open Folder",
                "-",
                "Close",
                "Close Project",
                "-",
                "Save",
                "-",
                "Quit",
            ],
            Self::Edit => vec![
                "Undo",
                "Redo",
                "-",
                "Cut",
                "Copy",
                "Paste",
                "Select All",
                "-",
                "Find",
                "Find Next",
                "Replace",
                "-",
                "Toggle Comment",
                "Indent",
                "Outdent",
                "-",
                "Go to Line",
                "-",
                "Go to Definition",
                "Find References",
                "Go Back",
            ],
            Self::View => vec!["Welcome"],
            Self::Help => vec!["About Atelier"],
        }
    }
}

pub fn resize_edge(pos: Point<gpui::Pixels>, size: Size<gpui::Pixels>) -> Option<ResizeEdge> {
    let edge = px(RESIZE_EDGE);
    let at_top = pos.y < edge;
    let at_bottom = pos.y > size.height - edge;
    let at_left = pos.x < edge;
    let at_right = pos.x > size.width - edge;
    match (at_top, at_bottom, at_left, at_right) {
        (true, _, true, _) => Some(ResizeEdge::TopLeft),
        (true, _, _, true) => Some(ResizeEdge::TopRight),
        (_, true, true, _) => Some(ResizeEdge::BottomLeft),
        (_, true, _, true) => Some(ResizeEdge::BottomRight),
        (true, _, _, _) => Some(ResizeEdge::Top),
        (_, true, _, _) => Some(ResizeEdge::Bottom),
        (_, _, true, _) => Some(ResizeEdge::Left),
        (_, _, _, true) => Some(ResizeEdge::Right),
        _ => None,
    }
}

pub fn render(
    shell: &AppShell,
    window: &mut Window,
    cx: &mut Context<AppShell>,
) -> impl IntoElement {
    let open = shell.bar_menu();
    let client_side = matches!(window.window_decorations(), Decorations::Client { .. });
    let controls = window.window_controls();

    div()
        .id("titlebar")
        .h(px(TITLEBAR_HEIGHT))
        .flex_none()
        .flex()
        .items_center()
        .px_1()
        .border_b_1()
        .border_color(rgb(0x2a2e35))
        .bg(rgb(0x15181d))
        .child(title(
            MenuBarMenu::File,
            0,
            open == Some(MenuBarMenu::File),
            cx,
        ))
        .child(title(
            MenuBarMenu::Edit,
            1,
            open == Some(MenuBarMenu::Edit),
            cx,
        ))
        .child(title(
            MenuBarMenu::View,
            2,
            open == Some(MenuBarMenu::View),
            cx,
        ))
        .child(title(
            MenuBarMenu::Help,
            3,
            open == Some(MenuBarMenu::Help),
            cx,
        ))
        .child(
            div()
                .id("titlebar-drag")
                .min_w_0()
                .flex_1()
                .h_full()
                .flex()
                .items_center()
                .px_3()
                .text_size(px(13.0))
                .text_color(rgb(0xaeb4bf))
                .window_control_area(WindowControlArea::Drag)
                .child("Atelier")
                .on_mouse_down(MouseButton::Left, |event, window, cx| {
                    cx.stop_propagation();
                    if event.click_count >= 2 {
                        window.zoom_window();
                    } else {
                        window.start_window_move();
                    }
                })
                .on_mouse_down(MouseButton::Right, |event, window, cx| {
                    cx.stop_propagation();
                    window.show_window_menu(event.position);
                }),
        )
        .when(client_side, |bar| {
            bar.child(
                div()
                    .flex()
                    .h_full()
                    .when(controls.minimize, |buttons| {
                        buttons.child(window_button(
                            "titlebar-min",
                            "–",
                            WindowControlArea::Min,
                            |window| window.minimize_window(),
                        ))
                    })
                    .when(controls.maximize, |buttons| {
                        buttons.child(window_button(
                            "titlebar-max",
                            "□",
                            WindowControlArea::Max,
                            |window| window.zoom_window(),
                        ))
                    })
                    .child(window_button(
                        "titlebar-close",
                        "×",
                        WindowControlArea::Close,
                        |window| window.remove_window(),
                    )),
            )
        })
}

fn title(
    menu: MenuBarMenu,
    index: usize,
    is_open: bool,
    cx: &mut Context<AppShell>,
) -> impl IntoElement {
    div()
        .id(("menu-bar-title", index))
        .relative()
        .h_full()
        .px_3()
        .flex()
        .items_center()
        .text_size(px(13.0))
        .cursor_pointer()
        .when(is_open, |title| {
            title
                .bg(rgb(0x1c2128))
                .border_l_1()
                .border_r_1()
                .border_color(rgb(0x2a2e35))
        })
        .when(!is_open, |title| title.hover(|style| style.bg(rgb(0x242932))))
        .child(menu.label())
        .on_click(cx.listener(move |shell, _, _, cx| {
            shell.toggle_bar_menu(menu, cx);
        }))
        .when(is_open, |title| {
            title.child(context_menu::dropdown(
                menu.items(),
                px(TITLEBAR_HEIGHT),
                cx,
                |shell, cx| {
                    shell.dismiss_bar_menu(cx);
                },
                move |shell, index, window, cx| {
                    shell.choose_bar_item(menu, index, window, cx);
                },
            ))
        })
}

fn window_button(
    id: &'static str,
    label: &'static str,
    area: WindowControlArea,
    on_click: impl Fn(&mut Window) + 'static,
) -> impl IntoElement {
    div()
        .id(id)
        .w(px(40.0))
        .h_full()
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(14.0))
        .text_color(rgb(0xaeb4bf))
        .cursor_pointer()
        .window_control_area(area)
        .hover(|style| {
            if matches!(area, WindowControlArea::Close) {
                style.bg(rgb(0xe06c75)).text_color(rgb(0xf0f2f5))
            } else {
                style.bg(rgb(0x242932)).text_color(rgb(0xf0f2f5))
            }
        })
        .on_click(move |_, window, cx| {
            cx.stop_propagation();
            on_click(window);
        })
        .child(label)
}
