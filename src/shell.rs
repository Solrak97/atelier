use std::path::{Path, PathBuf};

use atelier_core::{Document, Project, ProjectRegistry, RecentProject};
use gpui::{
    App, Context, Decorations, Entity, FocusHandle, Focusable, KeyBinding, MouseButton,
    PathPromptOptions, Pixels, Point, PromptLevel, Window, actions, div, prelude::*, rgb,
};

use crate::context_menu::{self, ContextMenu};
use crate::menu_bar::{self, MenuBarMenu};
use crate::welcome;
use crate::workspace::WorkspaceView;

actions!(
    atelier_shell,
    [OpenFolder, CloseProject, CloseDocument, Quit]
);

pub fn register_key_bindings(cx: &mut App) {
    cx.bind_keys([KeyBinding::new("ctrl-o", OpenFolder, None)]);
}

enum Session {
    Welcome,
    Workspace(Entity<WorkspaceView>),
}

#[derive(Clone)]
enum LeaveIntent {
    Welcome,
    OpenFolder,
    OpenPath {
        path: PathBuf,
        file: Option<PathBuf>,
    },
}

pub struct AppShell {
    registry: ProjectRegistry,
    session: Session,
    message: Option<String>,
    welcome_menu: Option<Point<Pixels>>,
    bar_menu: Option<MenuBarMenu>,
    focus_handle: FocusHandle,
}

impl AppShell {
    pub fn new(
        registry: ProjectRegistry,
        initial: Option<(Project, Option<Document>)>,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut shell = Self {
            registry,
            session: Session::Welcome,
            message: None,
            welcome_menu: None,
            bar_menu: None,
            focus_handle: cx.focus_handle(),
        };
        if let Some((project, document)) = initial {
            shell.activate_project(project, document, cx);
        }
        shell
    }

    pub fn recent_projects(&self) -> &[RecentProject] {
        self.registry.projects()
    }

    pub fn message(&self) -> Option<&str> {
        self.message.as_deref()
    }

    pub fn open_welcome_menu(
        &mut self,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !matches!(self.session, Session::Welcome) {
            return;
        }
        cx.stop_propagation();
        window.prevent_default();
        self.welcome_menu = Some(position);
        cx.notify();
    }

    pub fn welcome_menu_overlay(&self, cx: &mut Context<Self>) -> Option<impl IntoElement + use<>> {
        let position = self.welcome_menu?;
        Some(context_menu::overlay(
            ContextMenu {
                position,
                items: vec!["Open Folder"],
            },
            cx,
            |shell, cx| {
                shell.welcome_menu = None;
                cx.notify();
            },
            |shell, index, window, cx| {
                shell.welcome_menu = None;
                if index == 0 {
                    shell.open_folder(window, cx);
                }
                cx.notify();
            },
        ))
    }

    pub fn bar_menu(&self) -> Option<MenuBarMenu> {
        self.bar_menu
    }

    pub fn toggle_bar_menu(&mut self, menu: MenuBarMenu, cx: &mut Context<Self>) {
        self.welcome_menu = None;
        if self.bar_menu == Some(menu) {
            self.bar_menu = None;
        } else {
            self.bar_menu = Some(menu);
        }
        cx.notify();
    }

    pub fn dismiss_bar_menu(&mut self, cx: &mut Context<Self>) {
        self.bar_menu = None;
        cx.notify();
    }

    pub fn choose_bar_item(
        &mut self,
        menu: MenuBarMenu,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.bar_menu = None;
        match menu {
            MenuBarMenu::File => match index {
                0 => self.open_folder(window, cx),
                1 => self.close_active_document(window, cx),
                2 => self.close_project(window, cx),
                3 => window.dispatch_action(Box::new(crate::editor::Save), cx),
                4 => cx.quit(),
                _ => {}
            },
            MenuBarMenu::Edit => {
                let action: Box<dyn gpui::Action> = match index {
                    0 => Box::new(crate::editor::Undo),
                    1 => Box::new(crate::editor::Redo),
                    2 => Box::new(crate::editor::Cut),
                    3 => Box::new(crate::editor::Copy),
                    4 => Box::new(crate::editor::Paste),
                    5 => Box::new(crate::editor::SelectAll),
                    6 => Box::new(crate::editor::Find),
                    7 => Box::new(crate::editor::FindNext),
                    8 => Box::new(crate::editor::Replace),
                    9 => Box::new(crate::editor::ToggleComment),
                    10 => Box::new(crate::editor::Indent),
                    11 => Box::new(crate::editor::Outdent),
                    12 => Box::new(crate::editor::GoToLine),
                    13 => Box::new(crate::editor::GoToDefinition),
                    14 => Box::new(crate::editor::FindReferences),
                    15 => Box::new(crate::editor::GoBack),
                    _ => {
                        cx.notify();
                        return;
                    }
                };
                window.dispatch_action(action, cx);
            }
            MenuBarMenu::View if index == 0 => self.close_project(window, cx),
            MenuBarMenu::Help if index == 0 => {
                let answer = window.prompt(
                    PromptLevel::Info,
                    "Atelier",
                    Some("A focused native IDE without built-in AI."),
                    &["OK"],
                    cx,
                );
                cx.spawn_in(window, async move |_, _| {
                    let _ = answer.await;
                })
                .detach();
            }
            _ => {}
        }
        cx.notify();
    }

    fn close_active_document(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(workspace) = self.workspace().cloned() else {
            return;
        };
        workspace.update(cx, |workspace, cx| {
            workspace.close_active_document(window, cx);
        });
    }

    #[cfg(test)]
    pub fn is_welcome(&self) -> bool {
        matches!(self.session, Session::Welcome)
    }

    #[cfg(test)]
    pub fn last_open(&self) -> Option<&Path> {
        self.registry.last_open()
    }

    pub fn workspace(&self) -> Option<&Entity<WorkspaceView>> {
        match &self.session {
            Session::Workspace(workspace) => Some(workspace),
            Session::Welcome => None,
        }
    }

    pub fn focus_session(&self, window: &mut Window, cx: &mut App) {
        let handle = match &self.session {
            Session::Welcome => self.focus_handle.clone(),
            Session::Workspace(workspace) => workspace
                .read(cx)
                .active_focus_handle(cx)
                .unwrap_or_else(|| self.focus_handle.clone()),
        };
        window.focus(&handle, cx);
    }

    pub fn open_folder(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.leave(LeaveIntent::OpenFolder, window, cx);
    }

    pub fn close_project(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if matches!(self.session, Session::Welcome) {
            return;
        }
        self.leave(LeaveIntent::Welcome, window, cx);
    }

    pub fn open_recent(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(project) = self.registry.projects().get(index).cloned() else {
            return;
        };
        self.leave(
            LeaveIntent::OpenPath {
                path: project.path,
                file: None,
            },
            window,
            cx,
        );
    }

    #[cfg(test)]
    pub fn open_path(
        &mut self,
        path: &Path,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> std::io::Result<()> {
        self.commit_open_path(path, None, window, cx)
    }

    fn open_folder_action(&mut self, _: &OpenFolder, window: &mut Window, cx: &mut Context<Self>) {
        self.open_folder(window, cx);
    }

    fn close_project_action(
        &mut self,
        _: &CloseProject,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.close_project(window, cx);
    }

    fn close_document_action(
        &mut self,
        _: &CloseDocument,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.close_active_document(window, cx);
    }

    fn quit_action(&mut self, _: &Quit, _: &mut Window, cx: &mut Context<Self>) {
        cx.quit();
    }

    fn leave(&mut self, intent: LeaveIntent, window: &mut Window, cx: &mut Context<Self>) {
        if self.has_unsaved_changes(cx) {
            let answer = window.prompt(
                PromptLevel::Warning,
                "Save changes before leaving this project?",
                Some("Unsaved changes will be lost if discarded."),
                &["Save", "Discard", "Cancel"],
                cx,
            );
            cx.spawn_in(window, async move |this, cx| {
                let choice = answer.await.ok();
                this.update_in(cx, |shell, window, cx| match choice {
                    Some(0) => {
                        if shell.save_all(cx).is_ok() {
                            shell.commit_leave(intent, window, cx);
                        }
                    }
                    Some(1) => shell.commit_leave(intent, window, cx),
                    _ => {}
                })
                .ok();
            })
            .detach();
            return;
        }

        self.commit_leave(intent, window, cx);
    }

    fn commit_leave(&mut self, intent: LeaveIntent, window: &mut Window, cx: &mut Context<Self>) {
        match intent {
            LeaveIntent::Welcome => self.show_welcome(window, cx),
            LeaveIntent::OpenFolder => self.prompt_for_folder(window, cx),
            LeaveIntent::OpenPath { path, file } => {
                if let Err(error) = self.commit_open_path(&path, file.as_deref(), window, cx) {
                    let _ = self.registry.remove(&path);
                    self.message = Some(format!("Could not open {}: {error}", path.display()));
                    cx.notify();
                }
            }
        }
    }

    fn prompt_for_folder(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let receiver = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("Open Folder".into()),
        });
        cx.spawn_in(window, async move |this, cx| {
            let result = receiver.await;
            this.update_in(cx, |shell, window, cx| match result {
                Ok(Ok(Some(paths))) => {
                    if let Some(path) = paths.into_iter().next()
                        && let Err(error) = shell.commit_open_path(&path, None, window, cx)
                    {
                        shell.message = Some(error.to_string());
                        cx.notify();
                    }
                }
                Ok(Ok(None)) => {}
                Ok(Err(error)) => {
                    shell.message = Some(format!("Could not open folder picker: {error}"));
                    cx.notify();
                }
                Err(_) => {}
            })
            .ok();
        })
        .detach();
    }

    fn commit_open_path(
        &mut self,
        path: &Path,
        initial_file: Option<&Path>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> std::io::Result<()> {
        let (project, mut document) = Project::open_from_path(path)?;
        if document.is_none()
            && let Some(file) = initial_file
        {
            document = Some(Document::open(file)?);
        }
        if let Err(error) = project.ensure_metadata_dir() {
            self.message = Some(format!(
                "Opened {}, but could not create .atelier/: {error}",
                project.root().display()
            ));
        } else {
            self.message = None;
        }
        if let Err(error) = self.registry.record(project.root()) {
            let warning = format!("Could not remember project: {error}");
            self.message = Some(match self.message.take() {
                Some(existing) => format!("{existing}; {warning}"),
                None => warning,
            });
        }
        self.activate_project(project, document, cx);
        self.focus_session(window, cx);
        cx.notify();
        Ok(())
    }

    fn activate_project(
        &mut self,
        project: Project,
        document: Option<Document>,
        cx: &mut Context<Self>,
    ) {
        let workspace = cx.new(|cx| WorkspaceView::new(project, document, cx));
        cx.observe(&workspace, |_, _, cx| cx.notify()).detach();
        self.session = Session::Workspace(workspace);
        self.welcome_menu = None;
    }

    fn show_welcome(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Err(error) = self.registry.forget_last_open() {
            eprintln!("failed to clear last project: {error}");
        }
        self.session = Session::Welcome;
        self.message = None;
        self.welcome_menu = None;
        window.focus(&self.focus_handle, cx);
        cx.notify();
    }

    fn has_unsaved_changes(&self, cx: &App) -> bool {
        self.workspace()
            .is_some_and(|workspace| workspace.read(cx).has_unsaved_changes(cx))
    }

    fn save_all(&mut self, cx: &mut Context<Self>) -> std::io::Result<()> {
        let Some(workspace) = self.workspace().cloned() else {
            return Ok(());
        };
        workspace.update(cx, |workspace, cx| workspace.save_all(cx))
    }
}

impl Focusable for AppShell {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        match &self.session {
            Session::Welcome => self.focus_handle.clone(),
            Session::Workspace(workspace) => workspace
                .read(cx)
                .active_focus_handle(cx)
                .unwrap_or_else(|| self.focus_handle.clone()),
        }
    }
}

impl Render for AppShell {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let client_side = matches!(window.window_decorations(), Decorations::Client { .. });

        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(0x111318))
            .text_color(rgb(0xd7dae0))
            .when(client_side, |root| {
                root.border_1().border_color(rgb(0x2a2e35)).on_mouse_down(
                    MouseButton::Left,
                    |event, window, _| {
                        if let Some(edge) =
                            menu_bar::resize_edge(event.position, window.viewport_size())
                        {
                            window.start_window_resize(edge);
                        }
                    },
                )
            })
            .track_focus(&self.focus_handle)
            .key_context("App")
            .on_action(cx.listener(Self::open_folder_action))
            .on_action(cx.listener(Self::close_project_action))
            .on_action(cx.listener(Self::close_document_action))
            .on_action(cx.listener(Self::quit_action))
            .child(menu_bar::render(self, window, cx))
            .child(div().min_h_0().flex_1().child(match &self.session {
                Session::Welcome => welcome::render(self, cx).into_any_element(),
                Session::Workspace(workspace) => workspace.clone().into_any_element(),
            }))
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    use atelier_core::AppPaths;
    use gpui::TestAppContext;

    use super::*;

    struct TempHome(PathBuf);

    impl TempHome {
        fn new() -> Self {
            let unique = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path =
                std::env::temp_dir().join(format!("atelier-shell-{}-{unique}", std::process::id()));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }

        fn paths(&self) -> AppPaths {
            AppPaths::from_dirs(
                self.0.join("config"),
                self.0.join("data"),
                self.0.join("cache"),
            )
            .unwrap()
        }

        fn project(&self, name: &str) -> PathBuf {
            let path = self.0.join(name);
            fs::create_dir_all(&path).unwrap();
            path
        }
    }

    impl Drop for TempHome {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn open_shell(
        cx: &mut TestAppContext,
        registry: ProjectRegistry,
        initial: Option<(Project, Option<Document>)>,
    ) -> gpui::WindowHandle<AppShell> {
        cx.update(|cx| {
            cx.open_window(Default::default(), |_, cx| {
                cx.new(|cx| AppShell::new(registry, initial, cx))
            })
            .unwrap()
        })
    }

    #[gpui::test]
    fn welcome_opens_a_folder_then_returns_after_close(cx: &mut TestAppContext) {
        let home = TempHome::new();
        let project_dir = home.project("demo");
        fs::write(project_dir.join("notes.txt"), "hi").unwrap();
        let paths = home.paths();
        let registry = ProjectRegistry::empty(&paths);
        let window = open_shell(cx, registry, None);

        window
            .update(cx, |shell, _, _| {
                assert!(shell.is_welcome());
                assert!(shell.recent_projects().is_empty());
            })
            .unwrap();

        window
            .update(cx, |shell, window, cx| {
                shell.open_folder(window, cx);
            })
            .unwrap();
        assert!(cx.did_prompt_for_paths());
        let opened = project_dir.clone();
        cx.simulate_path_prompt_response(move |_| Some(vec![opened]));
        cx.run_until_parked();

        window
            .update(cx, |shell, _, _| {
                assert!(!shell.is_welcome());
                assert_eq!(shell.recent_projects().len(), 1);
                assert_eq!(shell.recent_projects()[0].name, "demo");
                assert_eq!(
                    shell.last_open(),
                    Some(fs::canonicalize(&project_dir).unwrap().as_path())
                );
            })
            .unwrap();
        assert!(project_dir.join(".atelier").is_dir());

        window
            .update(cx, |shell, window, cx| {
                shell.close_project(window, cx);
            })
            .unwrap();
        cx.run_until_parked();

        window
            .update(cx, |shell, _, _| {
                assert!(shell.is_welcome());
                assert_eq!(shell.recent_projects().len(), 1);
                assert_eq!(shell.last_open(), None);
            })
            .unwrap();
    }

    #[gpui::test]
    fn last_open_project_is_restored(cx: &mut TestAppContext) {
        let home = TempHome::new();
        let project_dir = home.project("resume");
        fs::write(project_dir.join("notes.txt"), "hi").unwrap();
        let mut registry = ProjectRegistry::empty(&home.paths());
        registry.record(&project_dir).unwrap();
        let project = Project::open(registry.last_open().unwrap()).unwrap();
        let window = open_shell(cx, registry, Some((project, None)));

        window
            .update(cx, |shell, _, _| {
                assert!(!shell.is_welcome());
                assert_eq!(shell.recent_projects()[0].name, "resume");
                assert_eq!(
                    shell.last_open(),
                    Some(fs::canonicalize(&project_dir).unwrap().as_path())
                );
            })
            .unwrap();
    }

    #[gpui::test]
    fn cli_path_opens_a_recorded_project(cx: &mut TestAppContext) {
        let home = TempHome::new();
        let project_dir = home.project("from-cli");
        let file = project_dir.join("main.rs");
        fs::write(&file, "fn main() {}").unwrap();
        let paths = home.paths();
        let registry = ProjectRegistry::empty(&paths);
        let window = open_shell(cx, registry, None);

        window
            .update(cx, |shell, window, cx| {
                shell.open_path(&file, window, cx).unwrap();
            })
            .unwrap();
        cx.run_until_parked();

        window
            .update(cx, |shell, _, _| {
                assert!(!shell.is_welcome());
                assert_eq!(shell.recent_projects()[0].name, "from-cli");
                assert!(home.paths().data_dir().join("projects.toml").is_file());
            })
            .unwrap();
    }

    #[gpui::test]
    fn closing_a_dirty_project_can_save_or_cancel(cx: &mut TestAppContext) {
        let home = TempHome::new();
        let project_dir = home.project("dirty");
        let file = project_dir.join("file.txt");
        fs::write(&file, "keep").unwrap();
        let (project, _) = Project::open_from_path(&project_dir).unwrap();
        let window = open_shell(
            cx,
            ProjectRegistry::empty(&home.paths()),
            Some((project, None)),
        );

        window
            .update(cx, |shell, window, cx| {
                shell
                    .workspace()
                    .unwrap()
                    .update(cx, |workspace, cx| workspace.open_file(&file, window, cx));
            })
            .unwrap();
        cx.run_until_parked();
        cx.simulate_input(window.into(), "!");

        window
            .update(cx, |shell, window, cx| {
                shell.close_project(window, cx);
            })
            .unwrap();
        assert!(cx.has_pending_prompt());
        cx.simulate_prompt_answer("Cancel");
        cx.run_until_parked();
        window
            .update(cx, |shell, _, _| {
                assert!(!shell.is_welcome());
            })
            .unwrap();
        assert_eq!(fs::read_to_string(&file).unwrap(), "keep");

        window
            .update(cx, |shell, window, cx| {
                shell.close_project(window, cx);
            })
            .unwrap();
        cx.simulate_prompt_answer("Save");
        cx.run_until_parked();
        window
            .update(cx, |shell, _, _| {
                assert!(shell.is_welcome());
            })
            .unwrap();
        assert_eq!(fs::read_to_string(&file).unwrap(), "!keep");
    }

    #[gpui::test]
    fn closing_a_dirty_project_can_discard(cx: &mut TestAppContext) {
        let home = TempHome::new();
        let project_dir = home.project("scratch");
        let file = project_dir.join("file.txt");
        fs::write(&file, "keep").unwrap();
        let (project, _) = Project::open_from_path(&project_dir).unwrap();
        let window = open_shell(
            cx,
            ProjectRegistry::empty(&home.paths()),
            Some((project, None)),
        );

        window
            .update(cx, |shell, window, cx| {
                shell
                    .workspace()
                    .unwrap()
                    .update(cx, |workspace, cx| workspace.open_file(&file, window, cx));
            })
            .unwrap();
        cx.run_until_parked();
        cx.simulate_input(window.into(), "!");

        window
            .update(cx, |shell, window, cx| {
                shell.close_project(window, cx);
            })
            .unwrap();
        cx.simulate_prompt_answer("Discard");
        cx.run_until_parked();
        window
            .update(cx, |shell, _, _| {
                assert!(shell.is_welcome());
            })
            .unwrap();
        assert_eq!(fs::read_to_string(&file).unwrap(), "keep");
    }

    #[gpui::test]
    fn missing_recent_project_is_dropped(cx: &mut TestAppContext) {
        let home = TempHome::new();
        let vanished = home.project("vanished");
        let paths = home.paths();
        let mut registry = ProjectRegistry::empty(&paths);
        registry.record(&vanished).unwrap();
        fs::remove_dir_all(&vanished).unwrap();
        let window = open_shell(cx, registry, None);

        window
            .update(cx, |shell, window, cx| {
                assert_eq!(shell.recent_projects().len(), 1);
                shell.open_recent(0, window, cx);
            })
            .unwrap();
        cx.run_until_parked();

        window
            .update(cx, |shell, _, _| {
                assert!(shell.is_welcome());
                assert!(shell.recent_projects().is_empty());
                assert!(
                    shell
                        .message()
                        .is_some_and(|message| message.contains("Could not open"))
                );
            })
            .unwrap();
    }
}
