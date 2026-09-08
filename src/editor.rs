use std::{io, ops::Range};

use atelier_core::{
    ByteOffset, ByteRange, DefinitionLookup, Document, Editor, ReferenceLookup,
    Revision, SearchQuery,
};
use gpui::{
    App, Bounds, ClipboardItem, Context, CursorStyle, Element, ElementId, ElementInputHandler,
    Entity, EntityInputHandler, FocusHandle, Focusable, GlobalElementId, Hsla, KeyBinding,
    LayoutId, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, PaintQuad, Pixels, Point,
    ShapedLine, Style, TextRun, UTF16Selection, UnderlineStyle, Window, actions, div, fill, point,
    prelude::*, px, relative, rgb, rgba, size,
};

use crate::analysis::SyntaxSession;
use crate::context_menu::{self, ContextMenu};
use crate::languages::{HighlightKind, HighlightSpan, LanguageExtension, registry};
use crate::scrollbar::VerticalScroll;

actions!(
    atelier_editor,
    [
        Backspace,
        Delete,
        Enter,
        Left,
        Right,
        Up,
        Down,
        SelectLeft,
        SelectRight,
        SelectUp,
        SelectDown,
        SelectAll,
        Copy,
        Cut,
        Paste,
        Undo,
        Redo,
        Save,
        Indent,
        Outdent,
        ToggleComment,
        DeleteLine,
        DuplicateLine,
        MoveLineUp,
        MoveLineDown,
        Home,
        End,
        SelectHome,
        SelectEnd,
        WordLeft,
        WordRight,
        SelectWordLeft,
        SelectWordRight,
        DocumentStart,
        DocumentEnd,
        SelectDocumentStart,
        SelectDocumentEnd,
        PageUp,
        PageDown,
        SelectPageUp,
        SelectPageDown,
        Find,
        FindNext,
        FindPrevious,
        Replace,
        ReplaceAll,
        ToggleFindCase,
        ToggleFindWord,
        Escape,
        GoToLine,
        GoToDefinition,
        FindReferences,
        GoBack,
    ]
);

pub fn register_key_bindings(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("backspace", Backspace, Some("Editor")),
        KeyBinding::new("delete", Delete, Some("Editor")),
        KeyBinding::new("enter", Enter, Some("Editor")),
        KeyBinding::new("left", Left, Some("Editor")),
        KeyBinding::new("right", Right, Some("Editor")),
        KeyBinding::new("up", Up, Some("Editor")),
        KeyBinding::new("down", Down, Some("Editor")),
        KeyBinding::new("shift-left", SelectLeft, Some("Editor")),
        KeyBinding::new("shift-right", SelectRight, Some("Editor")),
        KeyBinding::new("shift-up", SelectUp, Some("Editor")),
        KeyBinding::new("shift-down", SelectDown, Some("Editor")),
        KeyBinding::new("ctrl-a", SelectAll, Some("Editor")),
        KeyBinding::new("ctrl-c", Copy, Some("Editor")),
        KeyBinding::new("ctrl-x", Cut, Some("Editor")),
        KeyBinding::new("ctrl-v", Paste, Some("Editor")),
        KeyBinding::new("ctrl-z", Undo, Some("Editor")),
        KeyBinding::new("ctrl-shift-z", Redo, Some("Editor")),
        KeyBinding::new("ctrl-y", Redo, Some("Editor")),
        KeyBinding::new("ctrl-s", Save, Some("Editor")),
        KeyBinding::new("ctrl-s", Save, Some("Workspace")),
        KeyBinding::new("tab", Indent, Some("Editor")),
        KeyBinding::new("shift-tab", Outdent, Some("Editor")),
        KeyBinding::new("ctrl-/", ToggleComment, Some("Editor")),
        KeyBinding::new("ctrl-shift-k", DeleteLine, Some("Editor")),
        KeyBinding::new("ctrl-shift-d", DuplicateLine, Some("Editor")),
        KeyBinding::new("alt-up", MoveLineUp, Some("Editor")),
        KeyBinding::new("alt-down", MoveLineDown, Some("Editor")),
        KeyBinding::new("home", Home, Some("Editor")),
        KeyBinding::new("end", End, Some("Editor")),
        KeyBinding::new("shift-home", SelectHome, Some("Editor")),
        KeyBinding::new("shift-end", SelectEnd, Some("Editor")),
        KeyBinding::new("ctrl-left", WordLeft, Some("Editor")),
        KeyBinding::new("ctrl-right", WordRight, Some("Editor")),
        KeyBinding::new("ctrl-shift-left", SelectWordLeft, Some("Editor")),
        KeyBinding::new("ctrl-shift-right", SelectWordRight, Some("Editor")),
        KeyBinding::new("ctrl-home", DocumentStart, Some("Editor")),
        KeyBinding::new("ctrl-end", DocumentEnd, Some("Editor")),
        KeyBinding::new("ctrl-shift-home", SelectDocumentStart, Some("Editor")),
        KeyBinding::new("ctrl-shift-end", SelectDocumentEnd, Some("Editor")),
        KeyBinding::new("pageup", PageUp, Some("Editor")),
        KeyBinding::new("pagedown", PageDown, Some("Editor")),
        KeyBinding::new("shift-pageup", SelectPageUp, Some("Editor")),
        KeyBinding::new("shift-pagedown", SelectPageDown, Some("Editor")),
        KeyBinding::new("ctrl-f", Find, Some("Editor")),
        KeyBinding::new("f3", FindNext, Some("Editor")),
        KeyBinding::new("shift-f3", FindPrevious, Some("Editor")),
        KeyBinding::new("ctrl-h", Replace, Some("Editor")),
        KeyBinding::new("ctrl-alt-enter", ReplaceAll, Some("Editor")),
        KeyBinding::new("alt-c", ToggleFindCase, Some("Editor")),
        KeyBinding::new("alt-w", ToggleFindWord, Some("Editor")),
        KeyBinding::new("escape", Escape, Some("Editor")),
        KeyBinding::new("ctrl-g", GoToLine, Some("Editor")),
        KeyBinding::new("f12", GoToDefinition, Some("Editor")),
        KeyBinding::new("shift-f12", FindReferences, Some("Editor")),
        KeyBinding::new("alt-left", GoBack, Some("Editor")),
    ]);
}

pub struct EditorView {
    editor: Editor,
    focus_handle: FocusHandle,
    marked_range: Option<Range<usize>>,
    cached_lines: Vec<CachedLine>,
    is_selecting: bool,
    language: Option<&'static dyn LanguageExtension>,
    analysis: Option<SyntaxSession>,
    highlighted_revision: Option<Revision>,
    highlight_spans: Vec<HighlightSpan>,
    save_error: Option<String>,
    navigation_message: Option<String>,
    jump_stack: Vec<ByteOffset>,
    scroll: VerticalScroll,
    menu: Option<Point<Pixels>>,
    find_open: bool,
    find_query: String,
    find_case: bool,
    find_word: bool,
}

impl EditorView {
    pub fn new(document: Document, cx: &mut Context<Self>) -> Self {
        let language = document
            .path()
            .and_then(|path| registry().extension_for_path(path));
        let analysis = language.and_then(|language| language.analysis_session().ok());
        let mut editor = Editor::new(document);
        if let Some(language) = language {
            editor.set_edit_rules(language.edit_rules());
        }
        Self {
            editor,
            focus_handle: cx.focus_handle(),
            marked_range: None,
            cached_lines: Vec::new(),
            is_selecting: false,
            language,
            analysis,
            highlighted_revision: None,
            highlight_spans: Vec::new(),
            save_error: None,
            navigation_message: None,
            jump_stack: Vec::new(),
            scroll: VerticalScroll::new(),
            menu: None,
            find_open: false,
            find_query: String::new(),
            find_case: false,
            find_word: false,
        }
    }

    pub fn is_modified(&self) -> bool {
        self.editor.document().is_modified()
    }

    pub fn save(&mut self, cx: &mut Context<Self>) -> io::Result<()> {
        if !self.is_modified() {
            self.save_error = None;
            cx.notify();
            return Ok(());
        }

        match self.editor.save() {
            Ok(()) => {
                self.save_error = None;
                cx.notify();
                Ok(())
            }
            Err(error) => {
                self.save_error = Some(error.to_string());
                cx.notify();
                Err(error)
            }
        }
    }

    fn save_action(&mut self, _: &Save, _: &mut Window, cx: &mut Context<Self>) {
        let _ = self.save(cx);
    }

    fn ensure_analysis(&mut self) {
        if let Some(analysis) = &mut self.analysis {
            let _ = analysis.sync(&self.editor.document().snapshot());
        }
    }

    fn jump_to(&mut self, range: ByteRange, cx: &mut Context<Self>) {
        let origin = self.editor.cursor();
        if origin < range.start || origin > range.end {
            self.jump_stack.push(origin);
        }
        self.editor
            .set_selection(range.start, range.end)
            .expect("symbol ranges must be valid document offsets");
        self.navigation_message = None;
        cx.notify();
    }

    fn set_navigation_message(&mut self, message: impl Into<String>, cx: &mut Context<Self>) {
        self.navigation_message = Some(message.into());
        cx.notify();
    }

    fn go_to_definition(&mut self, _: &GoToDefinition, _: &mut Window, cx: &mut Context<Self>) {
        self.ensure_analysis();
        let lookup = match &self.analysis {
            Some(analysis) => analysis.symbols().go_to_definition(self.editor.cursor()),
            None => {
                self.set_navigation_message("No language analysis for this file", cx);
                return;
            }
        };
        match lookup {
            DefinitionLookup::Found(range) => self.jump_to(range, cx),
            DefinitionLookup::Unresolved(name) => {
                self.set_navigation_message(format!("No definition for `{name}` in this file"), cx);
            }
            DefinitionLookup::Missing => {
                self.set_navigation_message("No symbol at the caret", cx);
            }
        }
    }

    fn find_references(&mut self, _: &FindReferences, _: &mut Window, cx: &mut Context<Self>) {
        self.ensure_analysis();
        let lookup = match &self.analysis {
            Some(analysis) => analysis.symbols().find_references(self.editor.cursor()),
            None => {
                self.set_navigation_message("No language analysis for this file", cx);
                return;
            }
        };
        match lookup {
            ReferenceLookup::Found(ranges) => {
                let cursor = self.editor.cursor();
                let Some(next) = ReferenceLookup::Found(ranges.clone()).next_after(cursor) else {
                    self.set_navigation_message("No references in this file", cx);
                    return;
                };
                let index = ranges.iter().position(|range| *range == next).unwrap_or(0) + 1;
                let total = ranges.len();
                self.jump_to(next, cx);
                self.navigation_message = Some(format!("Reference {index} of {total}"));
                cx.notify();
            }
            ReferenceLookup::Unresolved(name) => {
                self.set_navigation_message(format!("No definition for `{name}` in this file"), cx);
            }
            ReferenceLookup::Missing => {
                self.set_navigation_message("No symbol at the caret", cx);
            }
        }
    }

    fn go_back(&mut self, _: &GoBack, _: &mut Window, cx: &mut Context<Self>) {
        let Some(offset) = self.jump_stack.pop() else {
            self.set_navigation_message("Nothing to go back to", cx);
            return;
        };
        self.editor
            .set_selection(offset, offset)
            .expect("jump origin must remain valid");
        self.navigation_message = None;
        cx.notify();
    }

    fn note_edit(&mut self, cx: &mut Context<Self>) {
        self.marked_range = None;
        self.save_error = None;
        self.navigation_message = None;
        cx.notify();
    }

    fn backspace(&mut self, _: &Backspace, _: &mut Window, cx: &mut Context<Self>) {
        self.editor
            .backspace()
            .expect("selection must remain valid");
        self.note_edit(cx);
    }

    fn delete(&mut self, _: &Delete, _: &mut Window, cx: &mut Context<Self>) {
        self.editor
            .delete_forward()
            .expect("selection must remain valid");
        self.note_edit(cx);
    }

    fn enter(&mut self, _: &Enter, _: &mut Window, cx: &mut Context<Self>) {
        self.editor
            .insert("\n")
            .expect("selection must remain valid");
        self.note_edit(cx);
    }

    fn left(&mut self, _: &Left, _: &mut Window, cx: &mut Context<Self>) {
        self.editor.move_left(false);
        self.marked_range = None;
        cx.notify();
    }

    fn right(&mut self, _: &Right, _: &mut Window, cx: &mut Context<Self>) {
        self.editor.move_right(false);
        self.marked_range = None;
        cx.notify();
    }

    fn up(&mut self, _: &Up, _: &mut Window, cx: &mut Context<Self>) {
        self.editor.move_up(false);
        self.marked_range = None;
        cx.notify();
    }

    fn down(&mut self, _: &Down, _: &mut Window, cx: &mut Context<Self>) {
        self.editor.move_down(false);
        self.marked_range = None;
        cx.notify();
    }

    fn select_left(&mut self, _: &SelectLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.editor.move_left(true);
        self.marked_range = None;
        cx.notify();
    }

    fn select_right(&mut self, _: &SelectRight, _: &mut Window, cx: &mut Context<Self>) {
        self.editor.move_right(true);
        self.marked_range = None;
        cx.notify();
    }

    fn select_up(&mut self, _: &SelectUp, _: &mut Window, cx: &mut Context<Self>) {
        self.editor.move_up(true);
        self.marked_range = None;
        cx.notify();
    }

    fn select_down(&mut self, _: &SelectDown, _: &mut Window, cx: &mut Context<Self>) {
        self.editor.move_down(true);
        self.marked_range = None;
        cx.notify();
    }

    fn select_all(&mut self, _: &SelectAll, _: &mut Window, cx: &mut Context<Self>) {
        self.editor.select_all();
        self.marked_range = None;
        cx.notify();
    }

    fn copy(&mut self, _: &Copy, _: &mut Window, cx: &mut Context<Self>) {
        let range = self.editor.selection().range();
        if !range.is_empty() {
            let content = self.editor.document().text();
            cx.write_to_clipboard(ClipboardItem::new_string(
                content[range.start.get()..range.end.get()].to_owned(),
            ));
        }
    }

    fn cut(&mut self, _: &Cut, window: &mut Window, cx: &mut Context<Self>) {
        self.copy(&Copy, window, cx);
        if !self.editor.selection().is_empty() {
            self.editor.insert("").expect("selection must remain valid");
            self.note_edit(cx);
        }
    }

    fn paste(&mut self, _: &Paste, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
            self.editor
                .insert(text)
                .expect("selection must remain valid");
            self.note_edit(cx);
        }
    }

    fn undo(&mut self, _: &Undo, _: &mut Window, cx: &mut Context<Self>) {
        self.editor.undo().expect("undo must keep a valid selection");
        self.note_edit(cx);
    }

    fn redo(&mut self, _: &Redo, _: &mut Window, cx: &mut Context<Self>) {
        self.editor.redo().expect("redo must keep a valid selection");
        self.note_edit(cx);
    }

    fn indent(&mut self, _: &Indent, _: &mut Window, cx: &mut Context<Self>) {
        self.editor.indent().expect("indent must keep a valid selection");
        self.note_edit(cx);
    }

    fn outdent(&mut self, _: &Outdent, _: &mut Window, cx: &mut Context<Self>) {
        self.editor.outdent().expect("outdent must keep a valid selection");
        self.note_edit(cx);
    }

    fn toggle_comment(&mut self, _: &ToggleComment, _: &mut Window, cx: &mut Context<Self>) {
        self.editor
            .toggle_line_comment()
            .expect("comment toggle must keep a valid selection");
        if self.editor.edit_rules().line_comment().is_none() {
            self.set_navigation_message("No line comment for this language", cx);
            return;
        }
        self.note_edit(cx);
    }

    fn delete_line(&mut self, _: &DeleteLine, _: &mut Window, cx: &mut Context<Self>) {
        self.editor.delete_line().expect("delete line must keep a valid selection");
        self.note_edit(cx);
    }

    fn duplicate_line(&mut self, _: &DuplicateLine, _: &mut Window, cx: &mut Context<Self>) {
        self.editor
            .duplicate_line()
            .expect("duplicate line must keep a valid selection");
        self.note_edit(cx);
    }

    fn move_line_up(&mut self, _: &MoveLineUp, _: &mut Window, cx: &mut Context<Self>) {
        self.editor.move_line_up().expect("move line must keep a valid selection");
        self.note_edit(cx);
    }

    fn move_line_down(&mut self, _: &MoveLineDown, _: &mut Window, cx: &mut Context<Self>) {
        self.editor
            .move_line_down()
            .expect("move line must keep a valid selection");
        self.note_edit(cx);
    }

    fn home(&mut self, _: &Home, _: &mut Window, cx: &mut Context<Self>) {
        self.editor.move_line_start(false);
        cx.notify();
    }

    fn end(&mut self, _: &End, _: &mut Window, cx: &mut Context<Self>) {
        self.editor.move_line_end(false);
        cx.notify();
    }

    fn select_home(&mut self, _: &SelectHome, _: &mut Window, cx: &mut Context<Self>) {
        self.editor.move_line_start(true);
        cx.notify();
    }

    fn select_end(&mut self, _: &SelectEnd, _: &mut Window, cx: &mut Context<Self>) {
        self.editor.move_line_end(true);
        cx.notify();
    }

    fn word_left(&mut self, _: &WordLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.editor.move_word_left(false);
        cx.notify();
    }

    fn word_right(&mut self, _: &WordRight, _: &mut Window, cx: &mut Context<Self>) {
        self.editor.move_word_right(false);
        cx.notify();
    }

    fn select_word_left(&mut self, _: &SelectWordLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.editor.move_word_left(true);
        cx.notify();
    }

    fn select_word_right(&mut self, _: &SelectWordRight, _: &mut Window, cx: &mut Context<Self>) {
        self.editor.move_word_right(true);
        cx.notify();
    }

    fn document_start(&mut self, _: &DocumentStart, _: &mut Window, cx: &mut Context<Self>) {
        self.editor.move_document_start(false);
        cx.notify();
    }

    fn document_end(&mut self, _: &DocumentEnd, _: &mut Window, cx: &mut Context<Self>) {
        self.editor.move_document_end(false);
        cx.notify();
    }

    fn select_document_start(
        &mut self,
        _: &SelectDocumentStart,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.editor.move_document_start(true);
        cx.notify();
    }

    fn select_document_end(
        &mut self,
        _: &SelectDocumentEnd,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.editor.move_document_end(true);
        cx.notify();
    }

    fn page_up(&mut self, _: &PageUp, _: &mut Window, cx: &mut Context<Self>) {
        self.editor.page_up(self.visible_page_lines(), false);
        cx.notify();
    }

    fn page_down(&mut self, _: &PageDown, _: &mut Window, cx: &mut Context<Self>) {
        self.editor.page_down(self.visible_page_lines(), false);
        cx.notify();
    }

    fn select_page_up(&mut self, _: &SelectPageUp, _: &mut Window, cx: &mut Context<Self>) {
        self.editor.page_up(self.visible_page_lines(), true);
        cx.notify();
    }

    fn select_page_down(&mut self, _: &SelectPageDown, _: &mut Window, cx: &mut Context<Self>) {
        self.editor.page_down(self.visible_page_lines(), true);
        cx.notify();
    }

    fn visible_page_lines(&self) -> usize {
        self.cached_lines.len().clamp(1, 40)
    }

    fn search_query(&self) -> SearchQuery {
        SearchQuery {
            text: self.find_query.clone(),
            case_sensitive: self.find_case,
            whole_word: self.find_word,
        }
    }

    fn seed_find_query(&mut self) {
        let range = self.editor.selection().range();
        if !range.is_empty() {
            let content = self.content();
            if let Some(text) = content.get(range.start.get()..range.end.get())
                && !text.contains('\n')
            {
                self.find_query = text.to_owned();
            }
        }
    }

    fn find(&mut self, _: &Find, _: &mut Window, cx: &mut Context<Self>) {
        self.seed_find_query();
        self.find_open = true;
        if self.editor.find_next(&self.search_query()).is_none() {
            self.set_navigation_message(
                if self.find_query.is_empty() {
                    "Find: select text and press Ctrl+F".to_owned()
                } else {
                    format!("No matches for `{}`", self.find_query)
                },
                cx,
            );
        } else {
            self.set_navigation_message(format!("Find `{}`", self.find_query), cx);
        }
    }

    fn find_next(&mut self, _: &FindNext, _: &mut Window, cx: &mut Context<Self>) {
        self.seed_find_query();
        if self.editor.find_next(&self.search_query()).is_none() {
            self.set_navigation_message("No matches", cx);
        } else {
            cx.notify();
        }
    }

    fn find_previous(&mut self, _: &FindPrevious, _: &mut Window, cx: &mut Context<Self>) {
        self.seed_find_query();
        if self.editor.find_previous(&self.search_query()).is_none() {
            self.set_navigation_message("No matches", cx);
        } else {
            cx.notify();
        }
    }

    fn replace(&mut self, _: &Replace, window: &mut Window, cx: &mut Context<Self>) {
        let replacement = cx
            .read_from_clipboard()
            .and_then(|item| item.text())
            .unwrap_or_default();
        self.seed_find_query();
        self.find_open = true;
        match self.editor.replace_match(&self.search_query(), &replacement) {
            Ok(Some(_)) => self.note_edit(cx),
            Ok(None) => self.set_navigation_message("No match to replace", cx),
            Err(_) => self.set_navigation_message("Replace failed", cx),
        }
        let _ = window;
    }

    fn replace_all(&mut self, _: &ReplaceAll, _: &mut Window, cx: &mut Context<Self>) {
        let replacement = cx
            .read_from_clipboard()
            .and_then(|item| item.text())
            .unwrap_or_default();
        self.seed_find_query();
        match self.editor.replace_all(&self.search_query(), &replacement) {
            Ok(0) => self.set_navigation_message("No matches to replace", cx),
            Ok(count) => {
                self.set_navigation_message(format!("Replaced {count} matches"), cx);
                self.note_edit(cx);
            }
            Err(_) => self.set_navigation_message("Replace all failed", cx),
        }
    }

    fn toggle_find_case(&mut self, _: &ToggleFindCase, _: &mut Window, cx: &mut Context<Self>) {
        self.find_case = !self.find_case;
        self.set_navigation_message(
            if self.find_case {
                "Find: case sensitive"
            } else {
                "Find: ignore case"
            },
            cx,
        );
    }

    fn toggle_find_word(&mut self, _: &ToggleFindWord, _: &mut Window, cx: &mut Context<Self>) {
        self.find_word = !self.find_word;
        self.set_navigation_message(
            if self.find_word {
                "Find: whole word"
            } else {
                "Find: any match"
            },
            cx,
        );
    }

    fn escape(&mut self, _: &Escape, _: &mut Window, cx: &mut Context<Self>) {
        if self.find_open {
            self.find_open = false;
            cx.notify();
        }
    }

    fn go_to_line(&mut self, _: &GoToLine, _: &mut Window, cx: &mut Context<Self>) {
        let range = self.editor.selection().range();
        let content = self.content();
        let selected = content
            .get(range.start.get()..range.end.get())
            .unwrap_or("");
        if let Ok(line) = selected.trim().parse::<usize>() {
            self.editor.go_to_line(line);
            self.set_navigation_message(format!("Line {line}"), cx);
        } else {
            self.set_navigation_message("Select a line number, then press Ctrl+G", cx);
        }
    }

    fn on_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        window.focus(&self.focus_handle, cx);
        let offset = self.offset_for_position(event.position);
        if event.modifiers.control {
            self.editor
                .set_selection(offset, offset)
                .expect("painted text offsets must be valid");
            self.go_to_definition(&GoToDefinition, window, cx);
            self.is_selecting = false;
            return;
        }
        let anchor = if event.modifiers.shift {
            self.editor.selection().anchor()
        } else {
            offset
        };
        self.editor
            .set_selection(anchor, offset)
            .expect("painted text offsets must be valid");
        self.is_selecting = true;
        self.marked_range = None;
        cx.notify();
    }

    fn on_mouse_move(&mut self, event: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.is_selecting {
            let offset = self.offset_for_position(event.position);
            self.editor
                .set_selection(self.editor.selection().anchor(), offset)
                .expect("painted text offsets must be valid");
            cx.notify();
        }
    }

    fn on_mouse_up(&mut self, _: &MouseUpEvent, _: &mut Window, _: &mut Context<Self>) {
        self.is_selecting = false;
    }

    fn on_right_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        cx.stop_propagation();
        window.prevent_default();
        window.focus(&self.focus_handle, cx);
        let offset = self.offset_for_position(event.position);
        self.editor
            .set_selection(offset, offset)
            .expect("painted text offsets must be valid");
        self.is_selecting = false;
        self.marked_range = None;
        cx.notify();
    }

    fn on_right_mouse_up(
        &mut self,
        event: &MouseUpEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        cx.stop_propagation();
        window.prevent_default();
        self.menu = Some(event.position);
        cx.notify();
    }

    fn dismiss_menu(&mut self, cx: &mut Context<Self>) {
        self.menu = None;
        cx.notify();
    }

    fn choose_menu_item(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.menu = None;
        match index {
            0 => self.go_to_definition(&GoToDefinition, window, cx),
            1 => self.find_references(&FindReferences, window, cx),
            2 => self.go_back(&GoBack, window, cx),
            _ => {}
        }
        cx.notify();
    }

    fn menu_overlay(&self, cx: &mut Context<Self>) -> Option<impl IntoElement + use<>> {
        let position = self.menu?;
        Some(context_menu::overlay(
            ContextMenu {
                position,
                items: vec!["Go to Definition", "Find References", "Go Back"],
            },
            cx,
            |editor, cx| editor.dismiss_menu(cx),
            |editor, index, window, cx| editor.choose_menu_item(index, window, cx),
        ))
    }

    fn offset_for_position(&self, position: Point<Pixels>) -> ByteOffset {
        let Some(first) = self.cached_lines.first() else {
            return ByteOffset::default();
        };
        if position.y < first.bounds.top() {
            return first.start.into();
        }

        let line = self
            .cached_lines
            .iter()
            .find(|line| position.y < line.bounds.bottom())
            .or_else(|| self.cached_lines.last())
            .expect("cached lines is not empty");
        let index = line
            .layout
            .closest_index_for_x(position.x - line.bounds.left())
            .min(line.display_end - line.start);
        (line.start + index).into()
    }

    fn content(&self) -> String {
        self.editor.document().text()
    }

    fn byte_offset_from_utf16(content: &str, offset: usize) -> usize {
        let mut utf8_offset = 0;
        let mut utf16_offset = 0;
        for character in content.chars() {
            if utf16_offset >= offset {
                break;
            }
            utf8_offset += character.len_utf8();
            utf16_offset += character.len_utf16();
        }
        utf8_offset
    }

    fn byte_offset_to_utf16(content: &str, offset: usize) -> usize {
        let mut utf8_offset = 0;
        let mut utf16_offset = 0;
        for character in content.chars() {
            if utf8_offset >= offset {
                break;
            }
            utf8_offset += character.len_utf8();
            utf16_offset += character.len_utf16();
        }
        utf16_offset
    }

    fn range_from_utf16(content: &str, range: &Range<usize>) -> Range<usize> {
        Self::byte_offset_from_utf16(content, range.start)
            ..Self::byte_offset_from_utf16(content, range.end)
    }

    fn range_to_utf16(content: &str, range: &Range<usize>) -> Range<usize> {
        Self::byte_offset_to_utf16(content, range.start)
            ..Self::byte_offset_to_utf16(content, range.end)
    }

    fn point_for_offset(&self, offset: usize) -> Option<Point<Pixels>> {
        let line = self
            .cached_lines
            .iter()
            .enumerate()
            .find(|(index, line)| {
                offset < line.full_end
                    || (*index == self.cached_lines.len() - 1 && offset <= line.full_end)
            })
            .map(|(_, line)| line)?;
        let line_offset = offset
            .saturating_sub(line.start)
            .min(line.display_end - line.start);
        Some(point(
            line.bounds.left() + line.layout.x_for_index(line_offset),
            line.bounds.top(),
        ))
    }

    fn replace_range(&mut self, range: Range<usize>, text: &str) {
        self.editor
            .set_selection(range.start.into(), range.end.into())
            .expect("input method must provide valid text boundaries");
        self.editor
            .insert(text)
            .expect("input method must provide valid text boundaries");
    }
}

impl Focusable for EditorView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl EntityInputHandler for EditorView {
    fn text_for_range(
        &mut self,
        range_utf16: Range<usize>,
        actual_range: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        let content = self.content();
        let range = Self::range_from_utf16(&content, &range_utf16);
        actual_range.replace(Self::range_to_utf16(&content, &range));
        content.get(range).map(ToOwned::to_owned)
    }

    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        let content = self.content();
        let selection = self.editor.selection();
        let range = selection.range();
        Some(UTF16Selection {
            range: Self::range_to_utf16(&content, &(range.start.get()..range.end.get())),
            reversed: selection.anchor() > selection.head(),
        })
    }

    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        let content = self.content();
        self.marked_range
            .as_ref()
            .map(|range| Self::range_to_utf16(&content, range))
    }

    fn unmark_text(&mut self, _: &mut Window, _: &mut Context<Self>) {
        self.marked_range = None;
    }

    fn replace_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let content = self.content();
        let range = range_utf16
            .as_ref()
            .map(|range| Self::range_from_utf16(&content, range))
            .or_else(|| self.marked_range.clone())
            .unwrap_or_else(|| {
                let range = self.editor.selection().range();
                range.start.get()..range.end.get()
            });
        self.replace_range(range, new_text);
        self.note_edit(cx);
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        _: Option<Range<usize>>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let content = self.content();
        let range = range_utf16
            .as_ref()
            .map(|range| Self::range_from_utf16(&content, range))
            .or_else(|| self.marked_range.clone())
            .unwrap_or_else(|| {
                let range = self.editor.selection().range();
                range.start.get()..range.end.get()
            });
        let marked_start = range.start;
        self.replace_range(range, new_text);
        self.save_error = None;
        self.marked_range =
            (!new_text.is_empty()).then_some(marked_start..marked_start + new_text.len());
        cx.notify();
    }

    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        _: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let content = self.content();
        let range = Self::range_from_utf16(&content, &range_utf16);
        let start = self.point_for_offset(range.start)?;
        let end = self.point_for_offset(range.end)?;
        Some(Bounds::from_corners(
            start,
            point(end.x.max(start.x + px(1.0)), end.y + px(22.0)),
        ))
    }

    fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        let content = self.content();
        let byte_offset = self.offset_for_position(point).get();
        Some(Self::byte_offset_to_utf16(&content, byte_offset))
    }
}

impl Render for EditorView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let current_revision = self.editor.document().revision();
        if self.highlighted_revision != Some(current_revision) {
            self.highlight_spans = self
                .language
                .and_then(|language| language.highlight(&self.content()).ok())
                .unwrap_or_default();
            if let Some(analysis) = &mut self.analysis {
                let _ = analysis.sync(&self.editor.document().snapshot());
            }
            self.highlighted_revision = Some(current_revision);
        }
        let path = {
            let path = self
                .editor
                .document()
                .path()
                .map(|path| path.display().to_string())
                .unwrap_or_else(|| "Untitled".to_owned());
            if self.is_modified() {
                format!("{path} •")
            } else {
                path
            }
        };
        let revision = self.editor.document().revision().get();
        let language_label = self
            .language
            .map(LanguageExtension::name)
            .unwrap_or("Plain text");
        let parse_error_count = self
            .analysis
            .as_ref()
            .and_then(|analysis| analysis.syntax())
            .map(|tree| tree.errors().len())
            .unwrap_or(0);
        let parse_status = match parse_error_count {
            0 => None,
            1 => Some("1 parse error".to_owned()),
            count => Some(format!("{count} parse errors")),
        };
        let (status_label, status_color) = if let Some(error) = &self.save_error {
            (format!("Save failed: {error}"), rgb(0xe06c75))
        } else if let Some(message) = &self.navigation_message {
            (message.clone(), rgb(0x61afef))
        } else if let Some(parse_status) = parse_status {
            if self.is_modified() {
                (
                    format!("Unsaved changes · {parse_status}"),
                    rgb(0xe06c75),
                )
            } else {
                (parse_status, rgb(0xe06c75))
            }
        } else if self.is_modified() {
            ("Unsaved changes · Ctrl+S to save".to_owned(), rgb(0xd19a66))
        } else {
            ("Saved · Ctrl+S".to_owned(), rgb(0x8f96a3))
        };

        let menu_overlay = self.menu_overlay(cx);

        div()
            .id("editor")
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(0x111318))
            .text_color(rgb(0xd7dae0))
            .track_focus(&self.focus_handle(cx))
            .key_context("Editor")
            .on_action(cx.listener(Self::backspace))
            .on_action(cx.listener(Self::delete))
            .on_action(cx.listener(Self::enter))
            .on_action(cx.listener(Self::left))
            .on_action(cx.listener(Self::right))
            .on_action(cx.listener(Self::up))
            .on_action(cx.listener(Self::down))
            .on_action(cx.listener(Self::select_left))
            .on_action(cx.listener(Self::select_right))
            .on_action(cx.listener(Self::select_up))
            .on_action(cx.listener(Self::select_down))
            .on_action(cx.listener(Self::select_all))
            .on_action(cx.listener(Self::copy))
            .on_action(cx.listener(Self::cut))
            .on_action(cx.listener(Self::paste))
            .on_action(cx.listener(Self::undo))
            .on_action(cx.listener(Self::redo))
            .on_action(cx.listener(Self::save_action))
            .on_action(cx.listener(Self::indent))
            .on_action(cx.listener(Self::outdent))
            .on_action(cx.listener(Self::toggle_comment))
            .on_action(cx.listener(Self::delete_line))
            .on_action(cx.listener(Self::duplicate_line))
            .on_action(cx.listener(Self::move_line_up))
            .on_action(cx.listener(Self::move_line_down))
            .on_action(cx.listener(Self::home))
            .on_action(cx.listener(Self::end))
            .on_action(cx.listener(Self::select_home))
            .on_action(cx.listener(Self::select_end))
            .on_action(cx.listener(Self::word_left))
            .on_action(cx.listener(Self::word_right))
            .on_action(cx.listener(Self::select_word_left))
            .on_action(cx.listener(Self::select_word_right))
            .on_action(cx.listener(Self::document_start))
            .on_action(cx.listener(Self::document_end))
            .on_action(cx.listener(Self::select_document_start))
            .on_action(cx.listener(Self::select_document_end))
            .on_action(cx.listener(Self::page_up))
            .on_action(cx.listener(Self::page_down))
            .on_action(cx.listener(Self::select_page_up))
            .on_action(cx.listener(Self::select_page_down))
            .on_action(cx.listener(Self::find))
            .on_action(cx.listener(Self::find_next))
            .on_action(cx.listener(Self::find_previous))
            .on_action(cx.listener(Self::replace))
            .on_action(cx.listener(Self::replace_all))
            .on_action(cx.listener(Self::toggle_find_case))
            .on_action(cx.listener(Self::toggle_find_word))
            .on_action(cx.listener(Self::escape))
            .on_action(cx.listener(Self::go_to_line))
            .on_action(cx.listener(Self::go_to_definition))
            .on_action(cx.listener(Self::find_references))
            .on_action(cx.listener(Self::go_back))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .on_mouse_down(MouseButton::Right, cx.listener(Self::on_right_mouse_down))
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_up(MouseButton::Right, cx.listener(Self::on_right_mouse_up))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .child(
                div()
                    .h(px(38.0))
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_between()
                    .px_4()
                    .border_b_1()
                    .border_color(rgb(0x2a2e35))
                    .bg(rgb(0x181b20))
                    .text_size(px(13.0))
                    .child(path)
                    .child(format!("revision {revision}")),
            )
            .when(self.find_open, |root| {
                let case = if self.find_case { "case" } else { "any" };
                let word = if self.find_word { "word" } else { "text" };
                root.child(
                    div()
                        .h(px(28.0))
                        .flex_none()
                        .flex()
                        .items_center()
                        .px_4()
                        .gap_3()
                        .border_b_1()
                        .border_color(rgb(0x2a2e35))
                        .bg(rgb(0x181b20))
                        .text_size(px(12.0))
                        .text_color(rgb(0xaeb4bf))
                        .child(format!(
                            "Find `{}` · {case} · {word} · F3 next · Ctrl+H replace from clipboard",
                            if self.find_query.is_empty() {
                                "…"
                            } else {
                                self.find_query.as_str()
                            }
                        )),
                )
            })
            .child(
                div()
                    .relative()
                    .min_h_0()
                    .flex_1()
                    .w_full()
                    .child(
                        div()
                            .id("editor-surface")
                            .size_full()
                            .overflow_y_scroll()
                            .overflow_x_hidden()
                            .track_scroll(self.scroll.handle())
                            .cursor(CursorStyle::IBeam)
                            .text_size(px(15.0))
                            .line_height(px(22.0))
                            .child(EditorElement {
                                editor: cx.entity(),
                            }),
                    )
                    .child(self.scroll.bar("editor-scrollbar", cx)),
            )
            .child(
                div()
                    .h(px(26.0))
                    .flex_none()
                    .flex()
                    .items_center()
                    .px_4()
                    .border_t_1()
                    .border_color(rgb(0x2a2e35))
                    .bg(rgb(0x181b20))
                    .text_size(px(12.0))
                    .text_color(status_color)
                    .child(format!("{language_label} · {status_label}")),
            )
            .when_some(menu_overlay, |root, overlay| root.child(overlay))
    }
}

struct EditorElement {
    editor: Entity<EditorView>,
}

impl IntoElement for EditorElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

struct PaintedLine {
    start: usize,
    display_end: usize,
    full_end: usize,
    bounds: Bounds<Pixels>,
    layout: ShapedLine,
}

struct EditorPrepaint {
    lines: Vec<PaintedLine>,
    selections: Vec<PaintQuad>,
    decorations: Vec<PaintQuad>,
    line_numbers: Vec<(Point<Pixels>, ShapedLine)>,
    cursor: Option<PaintQuad>,
}

struct CachedLine {
    start: usize,
    display_end: usize,
    full_end: usize,
    bounds: Bounds<Pixels>,
    layout: ShapedLine,
}

struct VisualLine<'a> {
    text: &'a str,
    start: usize,
    display_end: usize,
    full_end: usize,
}

fn highlight_color(kind: HighlightKind) -> Hsla {
    match kind {
        HighlightKind::Attribute | HighlightKind::Property => rgb(0x56b6c2).into(),
        HighlightKind::Boolean | HighlightKind::Constant | HighlightKind::Number => {
            rgb(0xd19a66).into()
        }
        HighlightKind::Comment => rgb(0x7f848e).into(),
        HighlightKind::Constructor | HighlightKind::Type => rgb(0xe5c07b).into(),
        HighlightKind::Function => rgb(0x61afef).into(),
        HighlightKind::Keyword => rgb(0xc678dd).into(),
        HighlightKind::Operator | HighlightKind::Punctuation | HighlightKind::Variable => {
            rgb(0xd7dae0).into()
        }
        HighlightKind::String => rgb(0x98c379).into(),
    }
}

fn parse_error_underline() -> UnderlineStyle {
    UnderlineStyle {
        thickness: px(1.5),
        color: Some(rgb(0xe06c75).into()),
        wavy: true,
    }
}

fn visible_error_range(
    error: ByteRange,
    line_start: usize,
    line_end: usize,
    line_text: &str,
) -> Option<(usize, usize)> {
    let mut start = error.start.get();
    let mut end = error.end.get();
    if start == end {
        if start >= line_start && start < line_end {
            let relative = start - line_start;
            let len = line_text[relative..].chars().next()?.len_utf8();
            end = start + len;
        } else if start == line_end && line_end > line_start {
            let len = line_text.chars().next_back()?.len_utf8();
            start = line_end - len;
            end = line_end;
        } else {
            return None;
        }
    }
    let visible_start = start.max(line_start);
    let visible_end = end.min(line_end);
    (visible_start < visible_end).then_some((visible_start, visible_end))
}

#[cfg(test)]
fn visual_line_count(content: &str) -> usize {
    if content.is_empty() {
        1
    } else {
        content.bytes().filter(|byte| *byte == b'\n').count() + 1
    }
}

fn visible_line_range(
    line_count: usize,
    top: Pixels,
    line_height: Pixels,
    clip: Bounds<Pixels>,
    viewport_height: Pixels,
) -> Range<usize> {
    if line_count == 0 || line_height <= px(0.0) {
        return 0..0;
    }

    let clip_height = clip.size.height.min(viewport_height);
    if clip_height <= px(0.0) {
        return 0..line_count.min(48);
    }

    let first = ((clip.top() - top) / line_height).floor().max(0.0) as usize;
    let first = first.saturating_sub(4);
    let visible = ((clip_height / line_height).ceil() as usize).saturating_add(12);
    first..(first + visible).min(line_count)
}

fn visual_lines(content: &str) -> Vec<VisualLine<'_>> {
    if content.is_empty() {
        return vec![VisualLine {
            text: "",
            start: 0,
            display_end: 0,
            full_end: 0,
        }];
    }

    let mut lines = Vec::new();
    let mut start = 0;
    for raw_line in content.split_inclusive('\n') {
        let without_newline = raw_line.strip_suffix('\n').unwrap_or(raw_line);
        let text = without_newline
            .strip_suffix('\r')
            .unwrap_or(without_newline);
        let full_end = start + raw_line.len();
        lines.push(VisualLine {
            text,
            start,
            display_end: start + text.len(),
            full_end,
        });
        start = full_end;
    }

    if content.ends_with('\n') {
        lines.push(VisualLine {
            text: "",
            start,
            display_end: start,
            full_end: start,
        });
    }
    lines
}

impl Element for EditorElement {
    type RequestLayoutState = ();
    type PrepaintState = EditorPrepaint;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let line_count = self
            .editor
            .read(cx)
            .editor
            .document()
            .len_lines()
            .max(1);
        let line_height = window.line_height();
        let mut style = Style::default();
        style.size.width = relative(1.0).into();
        style.size.height = (line_height * line_count as f32 + px(24.0)).into();
        style.min_size.height = relative(1.0).into();
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let editor = self.editor.read(cx);
        let content = editor.content();
        let selection = editor.editor.selection().range();
        let cursor_offset = editor.editor.cursor().get();
        let highlight_spans = &editor.highlight_spans;
        let parse_errors: Vec<ByteRange> = editor
            .analysis
            .as_ref()
            .and_then(|analysis| analysis.syntax())
            .map(|tree| tree.errors().iter().map(|error| error.range()).collect())
            .unwrap_or_default();
        let text_style = window.text_style();
        let font_size = text_style.font_size.to_pixels(window.rem_size());
        let line_height = window.line_height();
        let gutter = px(48.0);
        let left = bounds.left() + gutter + px(8.0);
        let top = bounds.top() + px(12.0);
        let mut lines = Vec::new();
        let mut selections = Vec::new();
        let mut decorations = Vec::new();
        let mut line_numbers = Vec::new();
        let mut cursor = None;
        let brackets = editor.editor.matching_brackets();
        let lines_in_document = visual_lines(&content);
        let visible = visible_line_range(
            lines_in_document.len(),
            top,
            line_height,
            window.content_mask().bounds,
            window.viewport_size().height,
        );

        for (index, visual_line) in lines_in_document
            .into_iter()
            .enumerate()
            .skip(visible.start)
            .take(visible.end.saturating_sub(visible.start))
        {
            let text = visual_line.text.to_owned().into();
            let make_run = |len, color, underline| TextRun {
                len,
                font: text_style.font(),
                color,
                background_color: None,
                underline,
                strikethrough: None,
            };
            let mut cuts = vec![visual_line.start, visual_line.display_end];
            for span in highlight_spans {
                if span.start > visual_line.start && span.start < visual_line.display_end {
                    cuts.push(span.start);
                }
                if span.end > visual_line.start && span.end < visual_line.display_end {
                    cuts.push(span.end);
                }
            }
            for error in &parse_errors {
                if let Some((start, end)) = visible_error_range(
                    *error,
                    visual_line.start,
                    visual_line.display_end,
                    visual_line.text,
                ) {
                    if start > visual_line.start {
                        cuts.push(start);
                    }
                    if end < visual_line.display_end {
                        cuts.push(end);
                    }
                }
            }
            cuts.sort_unstable();
            cuts.dedup();
            let mut runs = Vec::new();
            for pair in cuts.windows(2) {
                let start = pair[0];
                let end = pair[1];
                if start >= end {
                    continue;
                }
                let color = highlight_spans
                    .iter()
                    .rev()
                    .find(|span| span.start <= start && start < span.end)
                    .map(|span| highlight_color(span.kind))
                    .unwrap_or(text_style.color);
                let underline = parse_errors
                    .iter()
                    .any(|error| {
                        visible_error_range(
                            *error,
                            visual_line.start,
                            visual_line.display_end,
                            visual_line.text,
                        )
                        .is_some_and(|(error_start, error_end)| error_start < end && error_end > start)
                    })
                    .then(parse_error_underline);
                runs.push(make_run(end - start, color, underline));
            }
            if runs.is_empty() {
                runs.push(make_run(visual_line.text.len(), text_style.color, None));
            }
            let layout = window
                .text_system()
                .shape_line(text, font_size, &runs, None);
            let line_bounds = Bounds::new(
                point(left, top + line_height * index as f32),
                size(bounds.right() - left, line_height),
            );
            let is_current_line = cursor_offset >= visual_line.start
                && (cursor_offset < visual_line.full_end
                    || (visual_line.full_end == content.len() && cursor_offset == content.len()));
            if is_current_line {
                decorations.push(fill(
                    Bounds::new(
                        point(bounds.left(), line_bounds.top()),
                        size(bounds.size.width, line_height),
                    ),
                    rgba(0x252a3355),
                ));
            }
            if let Some((open, close)) = brackets {
                for range in [open, close] {
                    let start = range.start.get().max(visual_line.start);
                    let end = range.end.get().min(visual_line.display_end);
                    if start < end {
                        decorations.push(fill(
                            Bounds::from_corners(
                                point(
                                    left + layout.x_for_index(start - visual_line.start),
                                    line_bounds.top(),
                                ),
                                point(
                                    left + layout.x_for_index(end - visual_line.start),
                                    line_bounds.bottom(),
                                ),
                            ),
                            rgba(0xe5c07b55),
                        ));
                    }
                }
            }

            let number = format!("{}", index + 1);
            let number_color = if is_current_line {
                rgb(0xd7dae0).into()
            } else {
                rgb(0x5c6370).into()
            };
            let number_layout = window.text_system().shape_line(
                number.clone().into(),
                font_size,
                &[TextRun {
                    len: number.len(),
                    font: text_style.font(),
                    color: number_color,
                    background_color: None,
                    underline: None,
                    strikethrough: None,
                }],
                None,
            );
            let number_x =
                (bounds.left() + gutter - px(8.0) - number_layout.x_for_index(number.len()))
                    .max(bounds.left() + px(4.0));
            line_numbers.push((point(number_x, line_bounds.top()), number_layout));

            let selected_start = selection.start.get().max(visual_line.start);
            let selected_end = selection.end.get().min(visual_line.display_end);
            if selected_start < selected_end {
                selections.push(fill(
                    Bounds::from_corners(
                        point(
                            left + layout.x_for_index(selected_start - visual_line.start),
                            line_bounds.top(),
                        ),
                        point(
                            left + layout.x_for_index(selected_end - visual_line.start),
                            line_bounds.bottom(),
                        ),
                    ),
                    rgba(0x4c78ff55),
                ));
            } else if selection.start.get() < visual_line.full_end
                && selection.end.get() > visual_line.display_end
            {
                let x = left + layout.x_for_index(visual_line.text.len());
                selections.push(fill(
                    Bounds::new(point(x, line_bounds.top()), size(px(8.0), line_height)),
                    rgba(0x4c78ff55),
                ));
            }

            let is_cursor_line = cursor_offset < visual_line.full_end
                || (visual_line.full_end == content.len() && cursor_offset == visual_line.full_end);
            if cursor.is_none() && is_cursor_line {
                let line_offset = cursor_offset
                    .saturating_sub(visual_line.start)
                    .min(visual_line.text.len());
                let x = left + layout.x_for_index(line_offset);
                cursor = Some(fill(
                    Bounds::new(point(x, line_bounds.top()), size(px(2.0), line_height)),
                    rgb(0xd7dae0),
                ));
            }

            lines.push(PaintedLine {
                start: visual_line.start,
                display_end: visual_line.display_end,
                full_end: visual_line.full_end,
                bounds: line_bounds,
                layout,
            });
        }

        if cursor.is_none()
            && let Some(last) = lines.last()
            && last.full_end == content.len()
            && cursor_offset >= last.start
        {
            let x = left + last.layout.x_for_index(last.display_end - last.start);
            cursor = Some(fill(
                Bounds::new(point(x, last.bounds.top()), size(px(2.0), line_height)),
                rgb(0xd7dae0),
            ));
        }

        EditorPrepaint {
            lines,
            selections,
            decorations,
            line_numbers,
            cursor,
        }
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let focus_handle = self.editor.read(cx).focus_handle.clone();
        window.handle_input(
            &focus_handle,
            ElementInputHandler::new(bounds, self.editor.clone()),
            cx,
        );

        window.paint_quad(fill(
            Bounds::new(bounds.origin, size(px(48.0), bounds.size.height)),
            rgb(0x13161b),
        ));
        for decoration in prepaint.decorations.drain(..) {
            window.paint_quad(decoration);
        }
        for selection in prepaint.selections.drain(..) {
            window.paint_quad(selection);
        }
        for (origin, layout) in prepaint.line_numbers.drain(..) {
            layout
                .paint(
                    origin,
                    window.line_height(),
                    gpui::TextAlign::Left,
                    None,
                    window,
                    cx,
                )
                .expect("shaped line number must paint");
        }

        let mut cached_lines = Vec::with_capacity(prepaint.lines.len());
        for line in prepaint.lines.drain(..) {
            line.layout
                .paint(
                    line.bounds.origin,
                    line.bounds.size.height,
                    gpui::TextAlign::Left,
                    None,
                    window,
                    cx,
                )
                .expect("shaped editor line must paint");
            cached_lines.push(CachedLine {
                start: line.start,
                display_end: line.display_end,
                full_end: line.full_end,
                bounds: line.bounds,
                layout: line.layout,
            });
        }

        if focus_handle.is_focused(window)
            && let Some(cursor) = prepaint.cursor.take()
        {
            window.paint_quad(cursor);
        }

        self.editor.update(cx, |editor, _| {
            editor.cached_lines = cached_lines;
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn visual_line_count_matches_split_lines() {
        assert_eq!(visual_line_count(""), 1);
        assert_eq!(visual_line_count("one"), 1);
        assert_eq!(visual_line_count("one\n"), 2);
        assert_eq!(visual_line_count("one\r\ntwo\n"), visual_lines("one\r\ntwo\n").len());
    }

    #[test]
    fn visible_line_range_stays_near_the_viewport() {
        let range = visible_line_range(
            2000,
            px(0.0),
            px(22.0),
            Bounds::new(point(px(0.0), px(440.0)), size(px(800.0), px(220.0))),
            px(720.0),
        );
        assert!(range.start <= 20);
        assert!(range.end - range.start <= 40);
        assert!(range.end < 2000);
    }

    #[test]
    fn splits_lf_crlf_and_trailing_empty_lines() {
        let lines = visual_lines("one\r\ntwo\n");

        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0].text, "one");
        assert_eq!(
            (lines[0].start, lines[0].display_end, lines[0].full_end),
            (0, 3, 5)
        );
        assert_eq!(lines[1].text, "two");
        assert_eq!(
            (lines[1].start, lines[1].display_end, lines[1].full_end),
            (5, 8, 9)
        );
        assert_eq!(lines[2].text, "");
        assert_eq!(lines[2].start, 9);
    }

    #[test]
    fn converts_between_utf8_and_utf16_offsets() {
        let content = "a𐐀b";

        assert_eq!(EditorView::byte_offset_to_utf16(content, 5), 3);
        assert_eq!(EditorView::byte_offset_from_utf16(content, 3), 5);
    }

    #[test]
    fn empty_parse_error_expands_to_a_character_on_the_line() {
        assert_eq!(
            visible_error_range(ByteRange::from(3..3), 0, 5, "hello"),
            Some((3, 4))
        );
        assert_eq!(
            visible_error_range(ByteRange::from(5..5), 0, 5, "hello"),
            Some((4, 5))
        );
    }

    #[test]
    fn parse_error_outside_the_line_is_ignored() {
        assert_eq!(
            visible_error_range(ByteRange::from(10..12), 0, 5, "hello"),
            None
        );
    }
}
