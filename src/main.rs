mod editor;

use caduceus_core::Document;
use editor::EditorView;
use gpui::{
    App, Application, Bounds, Focusable, TitlebarOptions, WindowBounds, WindowOptions, prelude::*,
    px, size,
};

fn initial_document() -> Document {
    let Some(path) = std::env::args_os().nth(1) else {
        return Document::new(
            "# Caduceus\n\nThe document core is connected.\nStart typing to edit this buffer.\n",
        );
    };

    Document::open(&path).unwrap_or_else(|error| {
        eprintln!(
            "failed to open {}: {error}",
            std::path::Path::new(&path).display()
        );
        std::process::exit(1);
    })
}

fn main() {
    let document = initial_document();

    Application::new().run(move |cx: &mut App| {
        editor::register_key_bindings(cx);
        cx.on_window_closed(|cx| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();

        let bounds = Bounds::centered(None, size(px(960.0), px(640.0)), cx);

        cx.open_window(
            WindowOptions {
                titlebar: Some(TitlebarOptions {
                    title: Some("Caduceus".into()),
                    ..Default::default()
                }),
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |window, cx| {
                let editor = cx.new(|cx| EditorView::new(document, cx));
                window.focus(&editor.focus_handle(cx));
                editor
            },
        )
        .expect("failed to open the Caduceus window");

        cx.activate(true);
    });
}
