use std::path::{Path, PathBuf};

use caduceus_core::{Document, Project, ProjectEntry, ProjectEntryKind};
use gpui::{
    App, Context, Entity, Focusable, MouseButton, MouseUpEvent, Window, div, prelude::*, px, rgb,
};

use crate::editor::EditorView;

pub struct WorkspaceView {
    project: Project,
    open_documents: Vec<OpenDocument>,
    active_document: Option<usize>,
    message: Option<String>,
}

struct OpenDocument {
    path: PathBuf,
    editor: Entity<EditorView>,
}

struct FlatEntry {
    path: PathBuf,
    name: String,
    depth: usize,
    is_directory: bool,
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
        };

        if let Some(document) = initial_document {
            workspace.add_document(document, cx);
        }
        workspace
    }

    pub fn focus_active(&self, window: &mut Window, cx: &App) {
        if let Some(document) = self
            .active_document
            .and_then(|index| self.open_documents.get(index))
        {
            window.focus(&document.editor.focus_handle(cx));
        }
    }

    fn add_document(&mut self, document: Document, cx: &mut Context<Self>) -> Entity<EditorView> {
        let path = document
            .path()
            .expect("project documents must have a file path")
            .to_path_buf();
        let editor = cx.new(|cx| EditorView::new(document, cx));
        cx.observe(&editor, |_, _, cx| cx.notify()).detach();
        self.open_documents.push(OpenDocument {
            path,
            editor: editor.clone(),
        });
        self.active_document = Some(self.open_documents.len() - 1);
        editor
    }

    fn open_file(&mut self, path: &Path, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(index) = self
            .open_documents
            .iter()
            .position(|document| document.path == path)
        {
            self.activate_document(index, window, cx);
            return;
        }

        match Document::open(path) {
            Ok(document) => {
                self.add_document(document, cx);
                self.message = None;
                cx.defer_in(window, |workspace, window, cx| {
                    workspace.focus_active(window, cx);
                });
            }
            Err(error) => {
                self.message = Some(format!("Could not open {}: {error}", path.display()));
            }
        }
        cx.notify();
    }

    fn activate_document(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(document) = self.open_documents.get(index) {
            self.active_document = Some(index);
            let editor = document.editor.clone();
            self.message = None;
            cx.notify();
            cx.defer_in(window, move |_, window, cx| {
                window.focus(&editor.focus_handle(cx));
            });
        }
    }

    fn flattened_entries(&self) -> Vec<FlatEntry> {
        fn append(entries: &[ProjectEntry], depth: usize, output: &mut Vec<FlatEntry>) {
            for entry in entries {
                output.push(FlatEntry {
                    path: entry.path().to_path_buf(),
                    name: entry.name().to_owned(),
                    depth,
                    is_directory: entry.kind() == ProjectEntryKind::Directory,
                });
                append(entry.children(), depth + 1, output);
            }
        }

        let mut entries = Vec::new();
        append(self.project.entries(), 0, &mut entries);
        entries
    }
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
        let active_document = self
            .active_document
            .and_then(|index| self.open_documents.get(index))
            .map(|document| document.editor.clone());
        let has_active_document = active_document.is_some();

        div()
            .size_full()
            .flex()
            .bg(rgb(0x111318))
            .text_color(rgb(0xd7dae0))
            .child(
                div()
                    .w(px(250.0))
                    .h_full()
                    .flex_none()
                    .flex()
                    .flex_col()
                    .border_r_1()
                    .border_color(rgb(0x2a2e35))
                    .bg(rgb(0x15181d))
                    .child(
                        div()
                            .h(px(38.0))
                            .flex_none()
                            .flex()
                            .items_center()
                            .px_3()
                            .border_b_1()
                            .border_color(rgb(0x2a2e35))
                            .text_size(px(12.0))
                            .text_color(rgb(0xaeb4bf))
                            .child(root_name),
                    )
                    .child(
                        div()
                            .id("project-tree")
                            .flex_1()
                            .overflow_y_scroll()
                            .py_2()
                            .children(entries.into_iter().map(|entry| {
                                let path = entry.path.clone();
                                let label = if entry.is_directory {
                                    format!("▾ {}", entry.name)
                                } else {
                                    entry.name
                                };
                                div()
                                    .h(px(24.0))
                                    .flex()
                                    .items_center()
                                    .pl(px(10.0 + entry.depth as f32 * 14.0))
                                    .pr_2()
                                    .text_size(px(13.0))
                                    .text_color(if entry.is_directory {
                                        rgb(0xaeb4bf)
                                    } else {
                                        rgb(0xd7dae0)
                                    })
                                    .child(label)
                                    .when(!entry.is_directory, |row| {
                                        row.cursor_pointer()
                                            .hover(|style| style.bg(rgb(0x242932)))
                                            .on_mouse_up(
                                            MouseButton::Left,
                                            cx.listener(
                                                move |workspace, _: &MouseUpEvent, window, cx| {
                                                    workspace.open_file(&path, window, cx);
                                                },
                                            ),
                                        )
                                    })
                            })),
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
                    }),
            )
            .child(
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
                                        .path
                                        .file_name()
                                        .unwrap_or(document.path.as_os_str())
                                        .to_string_lossy()
                                        .into_owned();
                                    if document.editor.read(cx).is_modified() {
                                        name.push_str(" ●");
                                    }
                                    div()
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
                                        .child(name)
                                        .cursor_pointer()
                                        .hover(|style| style.bg(rgb(0x242932)))
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
                            .when_some(active_document, |content, editor| content.child(editor))
                            .when(!has_active_document, |content| {
                                content
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .text_color(rgb(0x777f8c))
                                    .child("Choose a file from the project tree")
                            }),
                    ),
            )
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    use gpui::TestAppContext;

    use super::*;

    struct TempWorkspace(PathBuf);

    impl TempWorkspace {
        fn new() -> Self {
            let unique = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "caduceus-workspace-{}-{unique}",
                std::process::id()
            ));
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
    fn opens_and_switches_files_without_reentrant_updates(cx: &mut TestAppContext) {
        let temp = TempWorkspace::new();
        let first = temp.0.join("first.txt");
        let second = temp.0.join("second.txt");
        fs::write(&first, "first").unwrap();
        fs::write(&second, "second").unwrap();
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
                workspace.activate_document(0, window, cx);
            })
            .unwrap();
        cx.run_until_parked();

        window
            .update(cx, |workspace, _, _| {
                assert_eq!(workspace.open_documents.len(), 2);
                assert_eq!(workspace.active_document, Some(0));
                assert_eq!(
                    workspace.open_documents[0].path,
                    fs::canonicalize(first).unwrap()
                );
            })
            .unwrap();
    }
}
