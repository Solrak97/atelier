mod editor;
mod languages;
mod scrollbar;
mod shell;
mod welcome;
mod workspace;

use std::{io, path::PathBuf};

use caduceus_core::{AppPaths, Project, ProjectRegistry};
use gpui::{App, Bounds, TitlebarOptions, WindowBounds, WindowOptions, prelude::*, px, size};
use shell::AppShell;

fn load_registry(paths: &AppPaths) -> ProjectRegistry {
    match ProjectRegistry::load(paths) {
        Ok(registry) => registry,
        Err(error) => {
            eprintln!("failed to read project registry: {error}");
            ProjectRegistry::empty(paths)
        }
    }
}

fn initial_project() -> io::Result<Option<(Project, Option<caduceus_core::Document>)>> {
    let Some(path) = std::env::args_os().nth(1).map(PathBuf::from) else {
        return Ok(None);
    };
    let (project, document) = Project::open_from_path(path)?;
    if let Err(error) = project.ensure_metadata_dir() {
        eprintln!(
            "opened {}, but could not create .caduceus/: {error}",
            project.root().display()
        );
    }
    Ok(Some((project, document)))
}

fn main() {
    let paths = AppPaths::standard().unwrap_or_else(|error| {
        eprintln!("failed to create application directories: {error}");
        std::process::exit(1);
    });
    let mut registry = load_registry(&paths);
    let initial = match initial_project() {
        Ok(Some((project, document))) => {
            if let Err(error) = registry.record(project.root()) {
                eprintln!("failed to remember project: {error}");
            }
            Some((project, document))
        }
        Ok(None) => None,
        Err(error) => {
            eprintln!("failed to open project: {error}");
            std::process::exit(1);
        }
    };

    gpui_platform::application().run(move |cx: &mut App| {
        editor::register_key_bindings(cx);
        shell::register_key_bindings(cx);
        cx.on_window_closed(|cx, _window_id| {
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
                move |_, cx| cx.new(|cx| AppShell::new(registry, initial, cx)),
            )
            .expect("failed to open the Caduceus window");

        window
            .update(cx, |shell, window, cx| {
                shell.focus_session(window, cx);
            })
            .expect("failed to focus the initial session");

        cx.activate(true);
    });
}
