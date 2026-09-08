use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use atelier_core::{Document, Project, ProjectEntry, ProjectEntryKind, is_unsupported_text};
use gpui::{
    App, Context, Entity, Focusable, MouseButton, MouseUpEvent, Pixels, Point, PromptLevel, Window,
    div, prelude::*, px, rgb,
};

use crate::context_menu::{self, ContextMenu};
use crate::editor::{EditorView, Save};
use crate::icons;
use crate::scrollbar::VerticalScroll;
use crate::shell::{CloseDocument, CloseProject, OpenFolder};

pub struct WorkspaceView {
    project: Project,
    open_documents: Vec<OpenDocument>,
    active_document: Option<usize>,
    message: Option<String>,
    tree_scroll: VerticalScroll,
    collapsed_directories: HashSet<PathBuf>,
    menu: Option<WorkspaceMenu>,
}

enum OpenDocument {
    Text {
        path: PathBuf,
        editor: Entity<EditorView>,
    },
    Unsupported {
        path: PathBuf,
        title: String,
        detail: String,
    },
}

impl OpenDocument {
    fn path(&self) -> &Path {
        match self {
            Self::Text { path, .. } | Self::Unsupported { path, .. } => path,
        }
    }

    fn editor(&self) -> Option<&Entity<EditorView>> {
        match self {
            Self::Text { editor, .. } => Some(editor),
            Self::Unsupported { .. } => None,
        }
    }

    fn is_modified(&self, cx: &App) -> bool {
        self.editor()
            .is_some_and(|editor| editor.read(cx).is_modified())
    }

    fn unsupported(&self) -> Option<(&Path, &str, &str)> {
        match self {
            Self::Unsupported {
                path,
                title,
                detail,
            } => Some((path, title, detail)),
            Self::Text { .. } => None,
        }
    }
}

struct FlatEntry {
    path: PathBuf,
    name: String,
    depth: usize,
    is_directory: bool,
}

enum WorkspaceMenu {
    Project {
        position: Point<Pixels>,
    },
    Tab {
        position: Point<Pixels>,
        index: usize,
    },
}

impl WorkspaceView {
    pub fn new(
        project: Project,
        initial_document: Option<Document>,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut workspace = Self {
            project,
            open_documents: Vec::new(),
            active_document: None,
            message: None,
            tree_scroll: VerticalScroll::new(),
            collapsed_directories: HashSet::new(),
            menu: None,
        };

        if let Some(document) = initial_document {
            workspace.add_document(document, cx);
        }
        workspace
    }

    pub fn focus_active(&self, window: &mut Window, cx: &mut App) {
        if let Some(editor) = self
            .active_document
            .and_then(|index| self.open_documents.get(index))
            .and_then(OpenDocument::editor)
        {
            window.focus(&editor.focus_handle(cx), cx);
        }
    }

    fn add_document(&mut self, document: Document, cx: &mut Context<Self>) -> Entity<EditorView> {
        let path = document
            .path()
            .expect("project documents must have a file path")
            .to_path_buf();
        let editor = cx.new(|cx| EditorView::new(document, cx));
        cx.observe(&editor, |_, _, cx| cx.notify()).detach();
        self.open_documents.push(OpenDocument::Text {
            path,
            editor: editor.clone(),
        });
        self.active_document = Some(self.open_documents.len() - 1);
        editor
    }

    pub(crate) fn open_file(&mut self, path: &Path, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(index) = self
            .open_documents
            .iter()
            .position(|document| paths_match(document.path(), path))
        {
            self.activate_document(index, window, cx);
            return;
        }

        match Document::open(path) {
            Ok(document) => {
                let editor = self.add_document(document, cx);
                self.message = None;
                self.focus_editor_later(&editor, window, cx);
            }
            Err(error) => {
                self.open_unsupported(path, &error, cx);
            }
        }
        cx.notify();
    }

    fn open_unsupported(
        &mut self,
        path: &Path,
        error: &std::io::Error,
        cx: &mut Context<Self>,
    ) {
        let path = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
        let name = path
            .file_name()
            .unwrap_or(path.as_os_str())
            .to_string_lossy()
            .into_owned();
        let (title, detail) = if is_unsupported_text(error) {
            (
                format!("{name} is not a text file"),
                "Atelier can only open UTF-8 text. Binary and other encodings stay on disk."
                    .to_owned(),
            )
        } else {
            (format!("Could not open {name}"), error.to_string())
        };
        self.open_documents.push(OpenDocument::Unsupported {
            path,
            title,
            detail,
        });
        self.active_document = Some(self.open_documents.len() - 1);
        self.message = None;
        cx.notify();
    }

    fn activate_document(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(document) = self.open_documents.get(index) else {
            return;
        };
        self.active_document = Some(index);
        self.message = None;
        if let Some(editor) = document.editor().cloned() {
            self.focus_editor_later(&editor, window, cx);
        }
        cx.notify();
    }

    fn focus_editor_later(
        &self,
        editor: &Entity<EditorView>,
        window: &Window,
        cx: &mut Context<Self>,
    ) {
        let editor = editor.clone();
        cx.defer_in(window, move |_, window, cx| {
            window.focus(&editor.focus_handle(cx), cx);
        });
    }

    fn save_document(&mut self, index: usize, cx: &mut Context<Self>) -> std::io::Result<()> {
        let Some(document) = self.open_documents.get(index) else {
            return Ok(());
        };
        let path = document.path().to_path_buf();
        let Some(editor) = document.editor().cloned() else {
            return Ok(());
        };
        match editor.update(cx, |editor, cx| editor.save(cx)) {
            Ok(()) => {
                self.message = None;
                cx.notify();
                Ok(())
            }
            Err(error) => {
                self.message = Some(format!("Could not save {}: {error}", path.display()));
                cx.notify();
                Err(error)
            }
        }
    }

    fn save_active(&mut self, _: &Save, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(index) = self.active_document {
            let _ = self.save_document(index, cx);
        }
    }

    fn close_active_document_action(
        &mut self,
        _: &CloseDocument,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.close_active_document(window, cx);
    }

    pub fn close_active_document(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(index) = self.active_document {
            self.close_document(index, window, cx);
        }
    }

    fn close_document(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(document) = self.open_documents.get(index) else {
            return;
        };
        if document.is_modified(cx) {
            let name = document
                .path()
                .file_name()
                .unwrap_or(document.path().as_os_str())
                .to_string_lossy()
                .into_owned();
            let answer = window.prompt(
                PromptLevel::Warning,
                &format!("Save changes to {name}?"),
                Some("Unsaved changes will be lost if discarded."),
                &["Save", "Discard", "Cancel"],
                cx,
            );
            cx.spawn_in(window, async move |this, cx| {
                let choice = answer.await.ok();
                this.update_in(cx, |workspace, window, cx| match choice {
                    Some(0) => {
                        if workspace.save_document(index, cx).is_ok() {
                            workspace.finish_close_document(index, window, cx);
                        }
                    }
                    Some(1) => workspace.finish_close_document(index, window, cx),
                    _ => {}
                })
                .ok();
            })
            .detach();
            return;
        }

        self.finish_close_document(index, window, cx);
    }

    fn finish_close_document(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if index >= self.open_documents.len() {
            return;
        }

        let closed_active_document = self.active_document == Some(index);
        self.open_documents.remove(index);
        self.active_document = match self.active_document {
            None => None,
            Some(_) if self.open_documents.is_empty() => None,
            Some(active) if active > index => Some(active - 1),
            Some(active) if active == index => Some(index.min(self.open_documents.len() - 1)),
            Some(active) => Some(active),
        };
        self.message = None;
        if closed_active_document {
            self.focus_active(window, cx);
        }
        cx.notify();
    }

    pub fn has_unsaved_changes(&self, cx: &App) -> bool {
        self.open_documents
            .iter()
            .any(|document| document.is_modified(cx))
    }

    pub fn save_all(&mut self, cx: &mut Context<Self>) -> std::io::Result<()> {
        for index in 0..self.open_documents.len() {
            self.save_document(index, cx)?;
        }
        Ok(())
    }

    pub fn active_focus_handle(&self, cx: &App) -> Option<gpui::FocusHandle> {
        self.active_document
            .and_then(|index| self.open_documents.get(index))
            .and_then(OpenDocument::editor)
            .map(|editor| editor.focus_handle(cx))
    }

    fn flattened_entries(&self) -> Vec<FlatEntry> {
        fn append(
            entries: &[ProjectEntry],
            depth: usize,
            collapsed: &HashSet<PathBuf>,
            output: &mut Vec<FlatEntry>,
        ) {
            for entry in entries {
                let is_directory = entry.kind() == ProjectEntryKind::Directory;
                output.push(FlatEntry {
                    path: entry.path().to_path_buf(),
                    name: entry.name().to_owned(),
                    depth,
                    is_directory,
                });
                if is_directory && !collapsed.contains(entry.path()) {
                    append(entry.children(), depth + 1, collapsed, output);
                }
            }
        }

        let mut entries = Vec::new();
        append(
            self.project.entries(),
            0,
            &self.collapsed_directories,
            &mut entries,
        );
        entries
    }

    fn toggle_directory(&mut self, path: &Path, cx: &mut Context<Self>) {
        if !self.collapsed_directories.remove(path) {
            self.collapsed_directories.insert(path.to_path_buf());
        }
        cx.notify();
    }

    fn open_menu(&mut self, menu: WorkspaceMenu, window: &mut Window, cx: &mut Context<Self>) {
        cx.stop_propagation();
        window.prevent_default();
        self.menu = Some(menu);
        cx.notify();
    }

    fn dismiss_menu(&mut self, cx: &mut Context<Self>) {
        self.menu = None;
        cx.notify();
    }

    fn choose_menu_item(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(menu) = self.menu.take() else {
            return;
        };
        match menu {
            WorkspaceMenu::Project { .. } => match index {
                0 => window.dispatch_action(Box::new(OpenFolder), cx),
                1 => window.dispatch_action(Box::new(CloseProject), cx),
                _ => {}
            },
            WorkspaceMenu::Tab { index: tab, .. } if index == 0 => {
                self.close_document(tab, window, cx);
            }
            WorkspaceMenu::Tab { .. } => {}
        }
        cx.notify();
    }

    fn menu_overlay(&self, cx: &mut Context<Self>) -> Option<impl IntoElement + use<>> {
        let menu = self.menu.as_ref()?;
        let spec = match menu {
            WorkspaceMenu::Project { position } => ContextMenu {
                position: *position,
                items: vec!["Open Folder", "Close Project"],
            },
            WorkspaceMenu::Tab { position, .. } => ContextMenu {
                position: *position,
                items: vec!["Close"],
            },
        };
        Some(context_menu::overlay(
            spec,
            cx,
            |workspace, cx| workspace.dismiss_menu(cx),
            |workspace, index, window, cx| workspace.choose_menu_item(index, window, cx),
        ))
    }
}

fn paths_match(open: &Path, clicked: &Path) -> bool {
    open == clicked || fs::canonicalize(clicked).is_ok_and(|canonical| canonical == open)
}

impl Render for WorkspaceView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let root_name = self
            .project
            .root()
            .file_name()
            .unwrap_or_else(|| self.project.root().as_os_str())
            .to_string_lossy()
            .into_owned();
        let entries = self.flattened_entries();
        let active = self
            .active_document
            .and_then(|index| self.open_documents.get(index));
        let active_editor = active.and_then(OpenDocument::editor).cloned();
        let active_notice = active.and_then(OpenDocument::unsupported).map(
            |(path, title, detail)| (path.to_path_buf(), title.to_owned(), detail.to_owned()),
        );
        let has_active_pane = active_editor.is_some() || active_notice.is_some();

        let menu_overlay = self.menu_overlay(cx);
        let sidebar = div()
            .id("project-sidebar")
            .w(px(250.0))
            .h_full()
            .flex_none()
            .flex()
            .flex_col()
            .border_r_1()
            .border_color(rgb(0x2a2e35))
            .bg(rgb(0x15181d))
            .on_mouse_down(MouseButton::Right, |_, window, cx| {
                cx.stop_propagation();
                window.prevent_default();
            })
            .on_mouse_up(
                MouseButton::Right,
                cx.listener(|workspace, event: &MouseUpEvent, window, cx| {
                    workspace.open_menu(
                        WorkspaceMenu::Project {
                            position: event.position,
                        },
                        window,
                        cx,
                    );
                }),
            )
            .child(
                div()
                    .id("project-header")
                    .h(px(38.0))
                    .flex_none()
                    .flex()
                    .items_center()
                    .px_3()
                    .border_b_1()
                    .border_color(rgb(0x2a2e35))
                    .gap_2()
                    .text_size(px(12.0))
                    .text_color(rgb(0xaeb4bf))
                    .child(icons::ExplorerIcon::FolderOpen.paint(14.0, rgb(0xc4a574)))
                    .child(div().min_w_0().flex_1().overflow_hidden().child(root_name)),
            )
            .child(
                div()
                    .relative()
                    .min_h_0()
                    .flex_1()
                    .child(
                        div()
                            .id("project-tree")
                            .size_full()
                            .overflow_y_scroll()
                            .track_scroll(self.tree_scroll.handle())
                            .py_2()
                            .children(entries.into_iter().enumerate().map(|(index, entry)| {
                                let path = entry.path.clone();
                                let is_directory = entry.is_directory;
                                let expanded = is_directory
                                    && !self.collapsed_directories.contains(&path);
                                let (glyph, color) = if is_directory {
                                    icons::folder_icon(expanded)
                                } else {
                                    icons::file_icon(&path)
                                };
                                div()
                                    .id(("project-entry", index))
                                    .h(px(24.0))
                                    .flex()
                                    .items_center()
                                    .gap_1()
                                    .pl(px(10.0 + entry.depth as f32 * 14.0))
                                    .pr_2()
                                    .text_size(px(13.0))
                                    .text_color(if is_directory {
                                        rgb(0xaeb4bf)
                                    } else {
                                        rgb(0xd7dae0)
                                    })
                                    .cursor_pointer()
                                    .hover(|style| style.bg(rgb(0x242932)))
                                    .child(if is_directory {
                                        icons::chevron(expanded).into_any_element()
                                    } else {
                                        icons::chevron_slot().into_any_element()
                                    })
                                    .child(glyph.paint(14.0, color))
                                    .child(entry.name)
                                    .on_mouse_up(
                                        MouseButton::Left,
                                        cx.listener(
                                            move |workspace, _: &MouseUpEvent, window, cx| {
                                                if is_directory {
                                                    workspace.toggle_directory(&path, cx);
                                                } else {
                                                    workspace.open_file(&path, window, cx);
                                                }
                                            },
                                        ),
                                    )
                            })),
                    )
                    .child(self.tree_scroll.bar("project-tree-scrollbar", cx)),
            )
            .when_some(self.message.clone(), |sidebar, message| {
                sidebar.child(
                    div()
                        .p_2()
                        .border_t_1()
                        .border_color(rgb(0x2a2e35))
                        .text_size(px(11.0))
                        .text_color(rgb(0xe06c75))
                        .child(message),
                )
            });
        let editor =
            div()
                .min_w_0()
                .h_full()
                .flex_1()
                .flex()
                .flex_col()
                .child(
                    div()
                        .h(px(36.0))
                        .flex_none()
                        .flex()
                        .overflow_hidden()
                        .border_b_1()
                        .border_color(rgb(0x2a2e35))
                        .bg(rgb(0x15181d))
                        .children(self.open_documents.iter().enumerate().map(
                            |(index, document)| {
                                let is_active = self.active_document == Some(index);
                                let mut name = document
                                    .path()
                                    .file_name()
                                    .unwrap_or(document.path().as_os_str())
                                    .to_string_lossy()
                                    .into_owned();
                                if document.is_modified(cx) {
                                    name.push_str(" ●");
                                }
                                div()
                                    .id(("document-tab", index))
                                    .h_full()
                                    .flex()
                                    .items_center()
                                    .px_3()
                                    .border_r_1()
                                    .border_color(rgb(0x2a2e35))
                                    .bg(if is_active {
                                        rgb(0x20242b)
                                    } else {
                                        rgb(0x15181d)
                                    })
                                    .text_size(px(12.0))
                                    .child(div().min_w_0().flex_1().overflow_hidden().child(name))
                                    .cursor_pointer()
                                    .hover(|style| style.bg(rgb(0x242932)))
                                    .on_mouse_down(MouseButton::Right, |_, window, cx| {
                                        cx.stop_propagation();
                                        window.prevent_default();
                                    })
                                    .on_mouse_up(
                                        MouseButton::Right,
                                        cx.listener(
                                            move |workspace, event: &MouseUpEvent, window, cx| {
                                                workspace.open_menu(
                                                    WorkspaceMenu::Tab {
                                                        position: event.position,
                                                        index,
                                                    },
                                                    window,
                                                    cx,
                                                );
                                            },
                                        ),
                                    )
                                    .on_mouse_up(
                                        MouseButton::Left,
                                        cx.listener(
                                            move |workspace, _: &MouseUpEvent, window, cx| {
                                                workspace.activate_document(index, window, cx);
                                            },
                                        ),
                                    )
                            },
                        )),
                )
                .child(
                    div()
                        .min_h_0()
                        .flex_1()
                        .when_some(active_editor, |content, editor| content.child(editor))
                        .when_some(active_notice, |content, (path, title, detail)| {
                            let (icon, color) = icons::file_icon(&path);
                            content.child(
                                div()
                                    .size_full()
                                    .flex()
                                    .flex_col()
                                    .items_center()
                                    .justify_center()
                                    .gap_3()
                                    .px_8()
                                    .bg(rgb(0x111318))
                                    .child(icon.paint(28.0, color))
                                    .child(
                                        div()
                                            .text_size(px(16.0))
                                            .text_color(rgb(0xd19a66))
                                            .child(title),
                                    )
                                    .child(
                                        div()
                                            .max_w(px(420.0))
                                            .text_size(px(13.0))
                                            .text_color(rgb(0x8f96a3))
                                            .child(detail),
                                    ),
                            )
                        })
                        .when(!has_active_pane, |content| {
                            content
                                .flex()
                                .items_center()
                                .justify_center()
                                .text_color(rgb(0x777f8c))
                                .child("Choose a file from the project tree")
                        }),
                );

        div()
            .relative()
            .size_full()
            .child(
                div()
                    .size_full()
                    .flex()
                    .bg(rgb(0x111318))
                    .text_color(rgb(0xd7dae0))
                    .key_context("Workspace")
                    .on_action(cx.listener(Self::save_active))
                    .on_action(cx.listener(Self::close_active_document_action))
                    .child(sidebar)
                    .child(editor),
            )
            .when_some(menu_overlay, |root, overlay| root.child(overlay))
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    use gpui::{Modifiers, TestAppContext, VisualTestContext, point, size};

    use super::*;

    struct TempWorkspace(PathBuf);

    impl TempWorkspace {
        fn new() -> Self {
            let unique = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path = std::env::temp_dir()
                .join(format!("atelier-workspace-{}-{unique}", std::process::id()));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TempWorkspace {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }

    #[gpui::test]
    fn opens_switches_and_closes_files_without_reentrant_updates(cx: &mut TestAppContext) {
        let temp = TempWorkspace::new();
        let first = temp.0.join("first.txt");
        let second = temp.0.join("second.txt");
        let third = temp.0.join("third.txt");
        fs::write(&first, "first").unwrap();
        fs::write(&second, "second").unwrap();
        fs::write(&third, "third").unwrap();
        let project = Project::open(&temp.0).unwrap();

        let window = cx.update(|cx| {
            cx.open_window(Default::default(), |_, cx| {
                cx.new(|cx| WorkspaceView::new(project, None, cx))
            })
            .unwrap()
        });

        window
            .update(cx, |workspace, window, cx| {
                workspace.open_file(&first, window, cx);
            })
            .unwrap();
        cx.run_until_parked();

        window
            .update(cx, |workspace, window, cx| {
                workspace.open_file(&second, window, cx);
            })
            .unwrap();
        cx.run_until_parked();

        window
            .update(cx, |workspace, window, cx| {
                workspace.open_file(&third, window, cx);
            })
            .unwrap();
        cx.run_until_parked();

        window
            .update(cx, |workspace, window, cx| {
                workspace.close_document(1, window, cx);
            })
            .unwrap();
        cx.run_until_parked();

        window
            .update(cx, |workspace, _, _| {
                assert_eq!(workspace.open_documents.len(), 2);
                assert_eq!(workspace.active_document, Some(1));
                assert_eq!(
                    workspace.open_documents[1].path(),
                    fs::canonicalize(&third).unwrap()
                );
            })
            .unwrap();

        window
            .update(cx, |workspace, window, cx| {
                workspace.activate_document(0, window, cx);
            })
            .unwrap();
        cx.run_until_parked();

        window
            .update(cx, |workspace, window, cx| {
                workspace.close_document(0, window, cx);
            })
            .unwrap();
        cx.run_until_parked();

        window
            .update(cx, |workspace, window, cx| {
                assert_eq!(workspace.open_documents.len(), 1);
                assert_eq!(workspace.active_document, Some(0));
                assert_eq!(
                    workspace.open_documents[0].path(),
                    fs::canonicalize(&third).unwrap()
                );
                workspace.close_document(0, window, cx);
            })
            .unwrap();
        cx.run_until_parked();

        window
            .update(cx, |workspace, _, _| {
                assert!(workspace.open_documents.is_empty());
                assert_eq!(workspace.active_document, None);
            })
            .unwrap();
    }

    #[gpui::test]
    fn collapsing_a_folder_hides_its_children(cx: &mut TestAppContext) {
        let temp = TempWorkspace::new();
        fs::create_dir(temp.0.join("src")).unwrap();
        fs::write(temp.0.join("src/main.rs"), "fn main() {}").unwrap();
        fs::write(temp.0.join("README.md"), "# project").unwrap();
        let project = Project::open(&temp.0).unwrap();
        let window = cx.update(|cx| {
            cx.open_window(Default::default(), |_, cx| {
                cx.new(|cx| WorkspaceView::new(project, None, cx))
            })
            .unwrap()
        });

        window
            .update(cx, |workspace, _, cx| {
                let names: Vec<String> = workspace
                    .flattened_entries()
                    .into_iter()
                    .map(|entry| entry.name)
                    .collect();
                assert_eq!(names, ["src", "main.rs", "README.md"]);

                let src = workspace
                    .flattened_entries()
                    .into_iter()
                    .find(|entry| entry.is_directory)
                    .expect("src folder")
                    .path;
                workspace.toggle_directory(&src, cx);

                let names: Vec<String> = workspace
                    .flattened_entries()
                    .into_iter()
                    .map(|entry| entry.name)
                    .collect();
                assert_eq!(names, ["src", "README.md"]);

                workspace.toggle_directory(&src, cx);
                let names: Vec<String> = workspace
                    .flattened_entries()
                    .into_iter()
                    .map(|entry| entry.name)
                    .collect();
                assert_eq!(names, ["src", "main.rs", "README.md"]);
            })
            .unwrap();
    }

    #[gpui::test]
    fn unsupported_files_warn_in_the_editor_pane(cx: &mut TestAppContext) {
        let temp = TempWorkspace::new();
        let text = temp.0.join("notes.txt");
        let binary = temp.0.join("icon.bin");
        fs::write(&text, "hello").unwrap();
        fs::write(&binary, [0x89, b'P', b'N', b'G', 0, 1, 2]).unwrap();
        let project = Project::open(&temp.0).unwrap();
        let window = cx.update(|cx| {
            cx.open_window(Default::default(), |_, cx| {
                cx.new(|cx| WorkspaceView::new(project, None, cx))
            })
            .unwrap()
        });

        window
            .update(cx, |workspace, window, cx| {
                workspace.open_file(&binary, window, cx);
                assert_eq!(workspace.open_documents.len(), 1);
                assert_eq!(workspace.active_document, Some(0));
                assert_eq!(workspace.message, None);
                let notice = workspace.open_documents[0]
                    .unsupported()
                    .expect("unsupported tab");
                assert!(notice.1.contains("icon.bin"));
                assert!(notice.2.contains("UTF-8"));

                workspace.open_file(&text, window, cx);
                assert_eq!(workspace.open_documents.len(), 2);
                assert!(workspace.open_documents[1].editor().is_some());
            })
            .unwrap();
    }

    #[gpui::test]
    fn saves_the_active_document_and_clears_the_modified_marker(cx: &mut TestAppContext) {
        let temp = TempWorkspace::new();
        let file = temp.0.join("notes.txt");
        fs::write(&file, "original").unwrap();
        let project = Project::open(&temp.0).unwrap();
        let window = cx.update(|cx| {
            cx.open_window(Default::default(), |_, cx| {
                cx.new(|cx| WorkspaceView::new(project, None, cx))
            })
            .unwrap()
        });

        window
            .update(cx, |workspace, window, cx| {
                workspace.open_file(&file, window, cx);
            })
            .unwrap();
        cx.run_until_parked();
        cx.simulate_input(window.into(), "!");

        window
            .update(cx, |workspace, _, cx| {
                assert!(workspace.open_documents[0].is_modified(cx));
                workspace.save_document(0, cx).unwrap();
                assert!(!workspace.open_documents[0].is_modified(cx));
            })
            .unwrap();

        assert_eq!(fs::read_to_string(&file).unwrap(), "!original");
    }

    #[gpui::test]
    fn close_prompt_can_save_discard_or_cancel(cx: &mut TestAppContext) {
        let temp = TempWorkspace::new();
        let file = temp.0.join("modified.txt");
        fs::write(&file, "original").unwrap();
        let project = Project::open(&temp.0).unwrap();
        let window = cx.update(|cx| {
            cx.open_window(Default::default(), |_, cx| {
                cx.new(|cx| WorkspaceView::new(project, None, cx))
            })
            .unwrap()
        });

        window
            .update(cx, |workspace, window, cx| {
                workspace.open_file(&file, window, cx);
            })
            .unwrap();
        cx.run_until_parked();
        cx.simulate_input(window.into(), "x");

        window
            .update(cx, |workspace, window, cx| {
                workspace.close_document(0, window, cx);
            })
            .unwrap();
        assert!(cx.has_pending_prompt());
        cx.simulate_prompt_answer("Cancel");
        cx.run_until_parked();

        window
            .update(cx, |workspace, _, cx| {
                assert_eq!(workspace.open_documents.len(), 1);
                assert!(workspace.open_documents[0].is_modified(cx));
            })
            .unwrap();
        assert_eq!(fs::read_to_string(&file).unwrap(), "original");

        window
            .update(cx, |workspace, window, cx| {
                workspace.close_document(0, window, cx);
            })
            .unwrap();
        cx.simulate_prompt_answer("Save");
        cx.run_until_parked();

        window
            .update(cx, |workspace, _, _| {
                assert!(workspace.open_documents.is_empty());
            })
            .unwrap();
        assert_eq!(fs::read_to_string(&file).unwrap(), "xoriginal");
    }

    #[gpui::test]
    fn discarding_a_modified_document_closes_without_writing(cx: &mut TestAppContext) {
        let temp = TempWorkspace::new();
        let file = temp.0.join("scratch.txt");
        fs::write(&file, "keep").unwrap();
        let project = Project::open(&temp.0).unwrap();
        let window = cx.update(|cx| {
            cx.open_window(Default::default(), |_, cx| {
                cx.new(|cx| WorkspaceView::new(project, None, cx))
            })
            .unwrap()
        });

        window
            .update(cx, |workspace, window, cx| {
                workspace.open_file(&file, window, cx);
            })
            .unwrap();
        cx.run_until_parked();
        cx.simulate_input(window.into(), "!");

        window
            .update(cx, |workspace, window, cx| {
                workspace.close_document(0, window, cx);
            })
            .unwrap();
        cx.simulate_prompt_answer("Discard");
        cx.run_until_parked();

        window
            .update(cx, |workspace, _, _| {
                assert!(workspace.open_documents.is_empty());
            })
            .unwrap();
        assert_eq!(fs::read_to_string(&file).unwrap(), "keep");
    }

    #[gpui::test]
    fn right_click_shows_a_project_menu(cx: &mut TestAppContext) {
        let temp = TempWorkspace::new();
        let project = Project::open(&temp.0).unwrap();
        let window = cx.open_window(size(px(800.0), px(600.0)), |_, cx| {
            WorkspaceView::new(project, None, cx)
        });
        let mut cx = VisualTestContext::from_window(window.into(), cx);
        let position = point(px(20.0), px(18.0));
        cx.simulate_mouse_move(position, None, Modifiers::default());
        cx.simulate_mouse_down(position, MouseButton::Right, Modifiers::default());
        cx.simulate_mouse_up(position, MouseButton::Right, Modifiers::default());

        window
            .update(&mut cx, |workspace, _, _| {
                assert!(workspace.menu.is_some(), "right-click should open a menu");
            })
            .unwrap();
        assert!(
            cx.debug_bounds("CONTEXT_MENU").is_some(),
            "context menu should paint"
        );
    }
}
