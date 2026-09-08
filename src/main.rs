mod editor;
mod workspace;

use std::{io, path::PathBuf};

use caduceus_core::{Document, Project};
use gpui::{
    App, Application, Bounds, TitlebarOptions, WindowBounds, WindowOptions, prelude::*, px, size,
};
use workspace::WorkspaceView;

fn startup() -> io::Result<(Project, Option<Document>)> {
    let path = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or(std::env::current_dir()?);

    if path.is_dir() {
        return Ok((Project::open(path)?, None));
    }

    let document = Document::open(&path)?;
    let root = document
        .path()
        .and_then(|path| path.parent())
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "file has no parent"))?;
    Ok((Project::open(root)?, Some(document)))
}

fn main() {
    let (project, document) = startup().unwrap_or_else(|error| {
        eprintln!("failed to open project: {error}");
        std::process::exit(1);
    });

    Application::new().run(move |cx: &mut App| {
        editor::register_key_bindings(cx);
        cx.on_window_closed(|cx| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();

        let bounds = Bounds::centered(None, size(px(1100.0), px(720.0)), cx);

        let window = cx
            .open_window(
                WindowOptions {
                    titlebar: Some(TitlebarOptions {
                        title: Some("Caduceus".into()),
                        ..Default::default()
                    }),
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                |_, cx| cx.new(|cx| WorkspaceView::new(project, document, cx)),
            )
            .expect("failed to open the Caduceus window");

        window
            .update(cx, |workspace, window, cx| {
                workspace.focus_active(window, cx);
            })
            .expect("failed to focus the initial document");

        cx.activate(true);
    });
}
