use std::{io, ops::Range};

use caduceus_core::{ByteOffset, Document, Editor, Revision};
use gpui::{
    App, Bounds, ClipboardItem, Context, CursorStyle, Element, ElementId, ElementInputHandler,
    Entity, EntityInputHandler, FocusHandle, Focusable, GlobalElementId, Hsla, KeyBinding,
    LayoutId, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, PaintQuad, Pixels, Point,
    ShapedLine, Style, TextRun, UTF16Selection, Window, actions, div, fill, point, prelude::*, px,
    relative, rgb, rgba, size,
};

use crate::analysis::SyntaxSession;
use crate::languages::{HighlightKind, HighlightSpan, LanguageExtension, registry};
use crate::scrollbar::VerticalScroll;

actions!(
    caduceus_editor,
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
        Save,
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
        KeyBinding::new("ctrl-s", Save, Some("Editor")),
        KeyBinding::new("ctrl-s", Save, Some("Workspace")),
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
    scroll: VerticalScroll,
}

impl EditorView {
    pub fn new(document: Document, cx: &mut Context<Self>) -> Self {
        let language = document
            .path()
            .and_then(|path| registry().extension_for_path(path));
        let analysis = language.and_then(|language| language.analysis_session().ok());
        Self {
            editor: Editor::new(document),
            focus_handle: cx.focus_handle(),
            marked_range: None,
            cached_lines: Vec::new(),
            is_selecting: false,
            language,
            analysis,
            highlighted_revision: None,
            highlight_spans: Vec::new(),
            save_error: None,
            scroll: VerticalScroll::new(),
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

    fn note_edit(&mut self, cx: &mut Context<Self>) {
        self.marked_range = None;
        self.save_error = None;
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

    fn on_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        window.focus(&self.focus_handle, cx);
        let offset = self.offset_for_position(event.position);
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
        let path = self
            .editor
            .document()
            .path()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "Untitled".to_owned());
        let revision = self.editor.document().revision().get();
        let language_label = self
            .language
            .map(LanguageExtension::name)
            .unwrap_or("Plain text");
        let (save_label, save_color) = if let Some(error) = &self.save_error {
            (format!("Save failed: {error}"), rgb(0xe06c75))
        } else if self.is_modified() {
            ("Unsaved changes · Ctrl+S to save".to_owned(), rgb(0xd19a66))
        } else {
            ("Saved · Ctrl+S".to_owned(), rgb(0x8f96a3))
        };

        div()
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
            .on_action(cx.listener(Self::save_action))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
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
                    .text_color(save_color)
                    .child(format!("{language_label} · {save_label}")),
            )
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
        let content = self.editor.read(cx).content();
        let line_count = visual_lines(&content).len();
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
        let text_style = window.text_style();
        let font_size = text_style.font_size.to_pixels(window.rem_size());
        let line_height = window.line_height();
        let left = bounds.left() + px(16.0);
        let top = bounds.top() + px(12.0);
        let mut lines = Vec::new();
        let mut selections = Vec::new();
        let mut cursor = None;

        for (index, visual_line) in visual_lines(&content).into_iter().enumerate() {
            let text = visual_line.text.to_owned().into();
            let make_run = |len, color| TextRun {
                len,
                font: text_style.font(),
                color,
                background_color: None,
                underline: None,
                strikethrough: None,
            };
            let mut runs = Vec::new();
            let mut run_start = visual_line.start;
            for span in highlight_spans
                .iter()
                .filter(|span| span.end > visual_line.start && span.start < visual_line.display_end)
            {
                let start = span.start.max(run_start).max(visual_line.start);
                let end = span.end.min(visual_line.display_end);
                if run_start < start {
                    runs.push(make_run(start - run_start, text_style.color));
                }
                if start < end {
                    runs.push(make_run(end - start, highlight_color(span.kind)));
                    run_start = end;
                }
            }
            if run_start < visual_line.display_end {
                runs.push(make_run(
                    visual_line.display_end - run_start,
                    text_style.color,
                ));
            }
            if runs.is_empty() {
                runs.push(make_run(visual_line.text.len(), text_style.color));
            }
            let layout = window
                .text_system()
                .shape_line(text, font_size, &runs, None);
            let line_bounds = Bounds::new(
                point(left, top + line_height * index as f32),
                size(bounds.right() - left, line_height),
            );

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

        for selection in prepaint.selections.drain(..) {
            window.paint_quad(selection);
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
}
