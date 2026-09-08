mod analysis;
mod context_menu;
mod editor;
mod icons;
mod languages;
mod menu_bar;
mod scrollbar;
mod shell;
mod welcome;
mod workspace;

use std::{io, path::PathBuf, sync::Arc};

use atelier_core::{AppPaths, Project, ProjectRegistry};
use gpui::{
    App, Bounds, TitlebarOptions, WindowBounds, WindowDecorations, WindowOptions, prelude::*, px,
    size,
};
use shell::AppShell;

fn app_icon() -> Option<Arc<image::RgbaImage>> {
    const PNG: &[u8] = include_bytes!("../assets/icon.png");
    image::load_from_memory(PNG)
        .ok()
        .map(|icon| Arc::new(icon.to_rgba8()))
}

fn load_registry(paths: &AppPaths) -> ProjectRegistry {
    match ProjectRegistry::load(paths) {
        Ok(registry) => registry,
        Err(error) => {
            eprintln!("failed to read project registry: {error}");
            ProjectRegistry::empty(paths)
        }
    }
}

fn initial_project(
    registry: &mut ProjectRegistry,
) -> io::Result<Option<(Project, Option<atelier_core::Document>)>> {
    if let Some(path) = std::env::args_os().nth(1).map(PathBuf::from) {
        return open_project_path(path);
    }
    restore_last_project(registry)
}

fn restore_last_project(
    registry: &mut ProjectRegistry,
) -> io::Result<Option<(Project, Option<atelier_core::Document>)>> {
    let Some(path) = registry.last_open().map(PathBuf::from) else {
        return Ok(None);
    };
    match open_project_path(path.clone()) {
        Ok(opened) => Ok(opened),
        Err(error) => {
            eprintln!(
                "could not restore last project {}: {error}",
                path.display()
            );
            if !path.is_dir()
                && let Err(clear_error) = registry.remove(&path)
            {
                eprintln!("failed to drop missing last project: {clear_error}");
            }
            if let Err(clear_error) = registry.forget_last_open() {
                eprintln!("failed to clear last project: {clear_error}");
            }
            Ok(None)
        }
    }
}

fn open_project_path(
    path: PathBuf,
) -> io::Result<Option<(Project, Option<atelier_core::Document>)>> {
    let (project, document) = Project::open_from_path(path)?;
    if let Err(error) = project.ensure_metadata_dir() {
        eprintln!(
            "opened {}, but could not create .atelier/: {error}",
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
    let initial = match initial_project(&mut registry) {
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
                        title: Some("Atelier".into()),
                        ..Default::default()
                    }),
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    window_decorations: Some(WindowDecorations::Client),
                    app_id: Some("atelier".into()),
                    icon: app_icon(),
                    ..Default::default()
                },
                move |_, cx| cx.new(|cx| AppShell::new(registry, initial, cx)),
            )
            .expect("failed to open the Atelier window");

        window
            .update(cx, |shell, window, cx| {
                shell.focus_session(window, cx);
            })
            .expect("failed to focus the initial session");

        cx.activate(true);
    });
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    #[test]
    fn app_icon_is_a_square_png() {
        let icon = image::load_from_memory(include_bytes!("../assets/icon.png")).unwrap();
        assert_eq!(icon.width(), icon.height());
        assert!(icon.width() >= 128);
    }

    #[test]
    fn desktop_entry_and_hicolor_icons_exist() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let desktop = std::fs::read_to_string(root.join("assets/atelier.desktop")).unwrap();
        assert!(desktop.contains("Name=Atelier"));
        assert!(desktop.contains("Icon=atelier"));
        assert!(desktop.contains("Exec=atelier %F"));
        assert!(root.join("assets/icons/hicolor/scalable/apps/atelier.svg").is_file());
        for size in [16, 24, 32, 48, 64, 128, 256, 512] {
            let icon = root.join(format!("assets/icons/hicolor/{size}x{size}/apps/atelier.png"));
            assert!(icon.is_file(), "missing {}", icon.display());
        }
    }
}
