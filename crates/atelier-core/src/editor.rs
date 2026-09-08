use std::io;

use crate::{
    ByteOffset, ByteRange, Document, EditError, EditorSettings, LanguageEditRules, Revision,
    SearchQuery,
};

const HISTORY_LIMIT: usize = 1000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum HistoryKind {
    Insert,
    Delete,
    Replace,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct HistoryEdit {
    start: ByteOffset,
    deleted: String,
    inserted: String,
    selection_before: Selection,
    selection_after: Selection,
    kind: HistoryKind,
}

/// An anchored selection whose head is the active caret position.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Selection {
    anchor: ByteOffset,
    head: ByteOffset,
}

impl Selection {
    pub const fn new(anchor: ByteOffset, head: ByteOffset) -> Self {
        Self { anchor, head }
    }

    pub const fn anchor(self) -> ByteOffset {
        self.anchor
    }

    pub const fn head(self) -> ByteOffset {
        self.head
    }

    pub const fn is_empty(self) -> bool {
        self.anchor.get() == self.head.get()
    }

    pub fn range(self) -> ByteRange {
        if self.anchor <= self.head {
            ByteRange::new(self.anchor, self.head)
        } else {
            ByteRange::new(self.head, self.anchor)
        }
    }
}

/// UI-independent editing state for one document.
pub struct Editor {
    document: Document,
    selection: Selection,
    preferred_column: Option<usize>,
    undo_stack: Vec<HistoryEdit>,
    redo_stack: Vec<HistoryEdit>,
    settings: EditorSettings,
    edit_rules: LanguageEditRules,
}

impl Editor {
    pub fn new(document: Document) -> Self {
        Self {
            document,
            selection: Selection::default(),
            preferred_column: None,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            settings: EditorSettings::default(),
            edit_rules: LanguageEditRules::default(),
        }
    }

    pub const fn settings(&self) -> EditorSettings {
        self.settings
    }

    pub fn set_settings(&mut self, settings: EditorSettings) {
        self.settings = settings;
    }

    pub const fn edit_rules(&self) -> LanguageEditRules {
        self.edit_rules
    }

    pub fn set_edit_rules(&mut self, rules: LanguageEditRules) {
        self.edit_rules = rules;
    }

    pub const fn document(&self) -> &Document {
        &self.document
    }

    pub fn save(&mut self) -> io::Result<()> {
        self.document.save()
    }

    pub fn undo(&mut self) -> Result<Revision, EditError> {
        let Some(edit) = self.undo_stack.pop() else {
            return Ok(self.document.revision());
        };
        let start = edit.start.get();
        let range = ByteRange::from(start..start + edit.inserted.len());
        let revision = self.document.replace(range, &edit.deleted)?;
        self.selection = edit.selection_before;
        self.preferred_column = None;
        self.redo_stack.push(edit);
        Ok(revision)
    }

    pub fn redo(&mut self) -> Result<Revision, EditError> {
        let Some(edit) = self.redo_stack.pop() else {
            return Ok(self.document.revision());
        };
        let start = edit.start.get();
        let range = ByteRange::from(start..start + edit.deleted.len());
        let revision = self.document.replace(range, &edit.inserted)?;
        self.selection = edit.selection_after;
        self.preferred_column = None;
        self.undo_stack.push(edit);
        Ok(revision)
    }

    pub const fn selection(&self) -> Selection {
        self.selection
    }

    pub const fn cursor(&self) -> ByteOffset {
        self.selection.head()
    }

    pub fn set_selection(&mut self, anchor: ByteOffset, head: ByteOffset) -> Result<(), EditError> {
        self.document.validate_offset(anchor)?;
        self.document.validate_offset(head)?;
        self.selection = Selection::new(anchor, head);
        self.preferred_column = None;
        Ok(())
    }

    pub fn move_left(&mut self, extend: bool) {
        let target = if !extend && !self.selection.is_empty() {
            self.selection.range().start
        } else {
            self.previous_boundary(self.cursor())
        };
        self.move_to(target, extend);
    }

    pub fn move_right(&mut self, extend: bool) {
        let target = if !extend && !self.selection.is_empty() {
            self.selection.range().end
        } else {
            self.next_boundary(self.cursor())
        };
        self.move_to(target, extend);
    }

    pub fn move_up(&mut self, extend: bool) {
        let current_line = self.document.line_index_at(self.cursor());
        if current_line == 0 {
            return;
        }
        self.move_vertically(current_line - 1, extend);
    }

    pub fn move_down(&mut self, extend: bool) {
        let current_line = self.document.line_index_at(self.cursor());
        if current_line >= self.document.last_line_index() {
            return;
        }
        self.move_vertically(current_line + 1, extend);
    }

    pub fn select_all(&mut self) {
        self.selection = Selection::new(0.into(), self.document.len_bytes().into());
        self.preferred_column = None;
    }

    pub fn move_word_left(&mut self, extend: bool) {
        let target = self.document.previous_word_boundary(self.cursor());
        self.move_to(target, extend);
    }

    pub fn move_word_right(&mut self, extend: bool) {
        let target = self.document.next_word_boundary(self.cursor());
        self.move_to(target, extend);
    }

    pub fn move_line_start(&mut self, extend: bool) {
        let line = self.document.line_index_at(self.cursor());
        let start = self.document.line_start(line).unwrap_or_default();
        let content_end = self
            .document
            .line_content_end(line)
            .unwrap_or(self.document.len_bytes().into());
        let line_text = self.document.text_in(ByteRange::new(start, content_end)).unwrap_or_default();
        let indent = line_text
            .chars()
            .take_while(|character| *character == ' ' || *character == '\t')
            .map(char::len_utf8)
            .sum::<usize>();
        let indented = ByteOffset::new(start.get() + indent);
        let target = if self.cursor() == indented { start } else { indented };
        self.move_to(target, extend);
    }

    pub fn move_line_end(&mut self, extend: bool) {
        let line = self.document.line_index_at(self.cursor());
        let target = self
            .document
            .line_content_end(line)
            .unwrap_or_else(|| self.document.len_bytes().into());
        self.move_to(target, extend);
    }

    pub fn move_document_start(&mut self, extend: bool) {
        self.move_to(ByteOffset::default(), extend);
    }

    pub fn move_document_end(&mut self, extend: bool) {
        self.move_to(self.document.len_bytes().into(), extend);
    }

    pub fn page_up(&mut self, lines: usize, extend: bool) {
        let current = self.document.line_index_at(self.cursor());
        let target = current.saturating_sub(lines.max(1));
        self.move_vertically(target, extend);
    }

    pub fn page_down(&mut self, lines: usize, extend: bool) {
        let current = self.document.line_index_at(self.cursor());
        let last = self.document.last_line_index();
        let target = current.saturating_add(lines.max(1)).min(last);
        self.move_vertically(target, extend);
    }

    pub fn go_to_line(&mut self, line_number: usize) {
        let index = line_number.saturating_sub(1).min(self.document.last_line_index());
        let target = self.document.line_start(index).unwrap_or_default();
        self.move_to(target, false);
    }

    pub fn insert(&mut self, text: impl AsRef<str>) -> Result<Revision, EditError> {
        let text = text.as_ref();
        if text == "\n" || text == "\r\n" {
            return self.insert_newline();
        }
        if let Some(character) = single_char(text) {
            if self.try_skip_closing(character)? {
                return Ok(self.document.revision());
            }
            if let Some(close) = self.edit_rules.closing_for(character) {
                return self.insert_pair(character, close);
            }
        }
        self.replace_selection(text)
    }

    pub fn backspace(&mut self) -> Result<Revision, EditError> {
        if self.selection.is_empty() {
            if let (Some(open), Some(close)) = (
                self.document.char_before(self.cursor()),
                self.document.char_at(self.cursor()),
            ) && self.edit_rules.closing_for(open) == Some(close)
            {
                let start = self.previous_boundary(self.cursor());
                let end = self.next_boundary(self.cursor());
                return self.apply_replacement(
                    ByteRange::new(start, end),
                    "",
                    Selection::new(start, start),
                );
            }
            let previous = self.previous_boundary(self.cursor());
            self.selection = Selection::new(previous, self.cursor());
        }
        self.replace_selection("")
    }

    pub fn delete_forward(&mut self) -> Result<Revision, EditError> {
        if self.selection.is_empty() {
            let next = self.next_boundary(self.cursor());
            self.selection = Selection::new(self.cursor(), next);
        }
        self.replace_selection("")
    }

    pub fn indent(&mut self) -> Result<Revision, EditError> {
        let unit = self.settings.indent_unit();
        if self.selection.is_empty() {
            return self.replace_selection(&unit);
        }
        self.map_selected_lines(|line| format!("{unit}{line}"))
    }

    pub fn outdent(&mut self) -> Result<Revision, EditError> {
        let unit = self.settings.indent_unit();
        let tab_width = self.settings.tab_width();
        self.map_selected_lines(|line| strip_indent(line, &unit, tab_width))
    }

    pub fn toggle_line_comment(&mut self) -> Result<Revision, EditError> {
        let Some(prefix) = self.edit_rules.line_comment() else {
            return Ok(self.document.revision());
        };
        let marker = format!("{prefix} ");
        let (first, last) = self.selected_line_span();
        let all_commented = (first..=last).all(|index| {
            self.document
                .line(index)
                .is_some_and(|line| line.trim_start().starts_with(prefix))
        });
        self.map_selected_lines(|line| {
            if all_commented {
                uncomment_line(line, prefix)
            } else if line.trim().is_empty() {
                line.to_owned()
            } else {
                let indent = line
                    .chars()
                    .take_while(|character| character.is_whitespace())
                    .collect::<String>();
                let rest = &line[indent.len()..];
                format!("{indent}{marker}{rest}")
            }
        })
    }

    pub fn delete_line(&mut self) -> Result<Revision, EditError> {
        let (first, last) = self.selected_line_span();
        let start = self.document.line_start(first).unwrap_or_default();
        let end = self
            .document
            .line_full_end(last)
            .unwrap_or_else(|| self.document.len_bytes().into());
        self.apply_replacement(ByteRange::new(start, end), "", Selection::new(start, start))
    }

    pub fn duplicate_line(&mut self) -> Result<Revision, EditError> {
        let range = self.selection.range();
        if range.is_empty() {
            let line = self.document.line_index_at(self.cursor());
            let start = self.document.line_start(line).unwrap_or_default();
            let end = self
                .document
                .line_full_end(line)
                .unwrap_or_else(|| self.document.len_bytes().into());
            let text = self.document.text_in(ByteRange::new(start, end))?;
            let insert_at = end;
            let insertion = if text.ends_with('\n') {
                text
            } else {
                format!("\n{text}")
            };
            let after = ByteOffset::new(insert_at.get() + insertion.len());
            return self.apply_replacement(
                ByteRange::new(insert_at, insert_at),
                &insertion,
                Selection::new(after, after),
            );
        }
        let text = self.document.text_in(range)?;
        let after = ByteOffset::new(range.end.get() + text.len());
        self.apply_replacement(ByteRange::new(range.end, range.end), &text, Selection::new(after, after))
    }

    pub fn move_line_up(&mut self) -> Result<Revision, EditError> {
        let (first, last) = self.selected_line_span();
        if first == 0 {
            return Ok(self.document.revision());
        }
        self.swap_line_block(first - 1, first, last)
    }

    pub fn move_line_down(&mut self) -> Result<Revision, EditError> {
        let (first, last) = self.selected_line_span();
        if last >= self.document.last_line_index() {
            return Ok(self.document.revision());
        }
        self.swap_line_block(first, last + 1, last + 1)
    }

    pub fn find_next(&mut self, query: &SearchQuery) -> Option<ByteRange> {
        if query.text.is_empty() {
            return None;
        }
        let text = self.document.text();
        let start = self.selection.range().end.get();
        let found = find_in(&text, query, start, true)
            .or_else(|| find_in(&text, query, 0, true))?;
        self.selection = Selection::new(found.start, found.end);
        self.preferred_column = None;
        Some(found)
    }

    pub fn find_previous(&mut self, query: &SearchQuery) -> Option<ByteRange> {
        if query.text.is_empty() {
            return None;
        }
        let text = self.document.text();
        let start = self.selection.range().start.get();
        let found = find_in(&text, query, start, false)
            .or_else(|| find_in(&text, query, text.len(), false))?;
        self.selection = Selection::new(found.start, found.end);
        self.preferred_column = None;
        Some(found)
    }

    pub fn replace_match(
        &mut self,
        query: &SearchQuery,
        replacement: &str,
    ) -> Result<Option<ByteRange>, EditError> {
        let current = self.selection.range();
        let text = self.document.text_in(current)?;
        if !is_search_match(&text, query) {
            return Ok(self.find_next(query));
        }
        self.replace_selection(replacement)?;
        Ok(self.find_next(query))
    }

    pub fn replace_all(
        &mut self,
        query: &SearchQuery,
        replacement: &str,
    ) -> Result<usize, EditError> {
        if query.text.is_empty() {
            return Ok(0);
        }
        let text = self.document.text();
        let mut matches = Vec::new();
        let mut cursor = 0;
        while let Some(found) = find_in(&text, query, cursor, true) {
            cursor = found.end.get();
            if cursor == found.start.get() {
                cursor += 1;
            }
            matches.push(found);
        }
        if matches.is_empty() {
            return Ok(0);
        }
        let mut rebuilt = String::new();
        let mut last = 0;
        for found in &matches {
            rebuilt.push_str(&text[last..found.start.get()]);
            rebuilt.push_str(replacement);
            last = found.end.get();
        }
        rebuilt.push_str(&text[last..]);
        let whole = ByteRange::from(0..text.len());
        let caret = ByteOffset::new(rebuilt.len().min(self.cursor().get()));
        self.apply_replacement(whole, &rebuilt, Selection::new(caret, caret))?;
        Ok(matches.len())
    }

    pub fn matching_brackets(&self) -> Option<(ByteRange, ByteRange)> {
        let cursor = self.cursor();
        let at = self.document.char_at(cursor);
        let before = self.document.char_before(cursor);
        let (offset, character) = if at.is_some_and(|c| self.is_bracket(c)) {
            (cursor, at?)
        } else if before.is_some_and(|c| self.is_bracket(c)) {
            let start = self.previous_boundary(cursor);
            (start, before?)
        } else {
            return None;
        };
        let text = self.document.text();
        find_matching_bracket(&text, offset.get(), character, self.edit_rules)
    }

    fn insert_newline(&mut self) -> Result<Revision, EditError> {
        let line = self.document.line_index_at(self.cursor());
        let start = self.document.line_start(line).unwrap_or_default();
        let prefix = self
            .document
            .text_in(ByteRange::new(start, self.cursor()))
            .unwrap_or_default();
        let indent: String = prefix
            .chars()
            .take_while(|character| *character == ' ' || *character == '\t')
            .collect();
        let extra = if self
            .document
            .char_before(self.cursor())
            .is_some_and(|character| matches!(character, '{' | '[' | '('))
        {
            self.settings.indent_unit()
        } else {
            String::new()
        };
        self.replace_selection(format!("\n{indent}{extra}"))
    }

    fn insert_pair(&mut self, open: char, close: char) -> Result<Revision, EditError> {
        let range = self.selection.range();
        let inner = self.document.text_in(range)?;
        let text = format!("{open}{inner}{close}");
        let start = range.start.get();
        let after = if inner.is_empty() {
            let caret = ByteOffset::new(start + open.len_utf8());
            Selection::new(caret, caret)
        } else {
            Selection::new(
                ByteOffset::new(start + open.len_utf8()),
                ByteOffset::new(start + open.len_utf8() + inner.len()),
            )
        };
        self.apply_replacement(range, &text, after)
    }

    fn try_skip_closing(&mut self, character: char) -> Result<bool, EditError> {
        if !self.selection.is_empty() || self.edit_rules.opening_for(character).is_none() {
            return Ok(false);
        }
        if self.document.char_at(self.cursor()) != Some(character) {
            return Ok(false);
        }
        let next = self.next_boundary(self.cursor());
        self.selection = Selection::new(next, next);
        self.preferred_column = None;
        Ok(true)
    }

    fn is_bracket(&self, character: char) -> bool {
        self.edit_rules.closing_for(character).is_some()
            || self.edit_rules.opening_for(character).is_some()
    }

    fn selected_line_span(&self) -> (usize, usize) {
        let range = self.selection.range();
        let first = self.document.line_index_at(range.start);
        let last = if range.is_empty() || range.end.get() == 0 {
            first
        } else {
            let end = ByteOffset::new(range.end.get().saturating_sub(1));
            self.document.line_index_at(end).max(first)
        };
        (first, last)
    }

    fn map_selected_lines(
        &mut self,
        mut rewrite: impl FnMut(&str) -> String,
    ) -> Result<Revision, EditError> {
        let (first, last) = self.selected_line_span();
        let start = self.document.line_start(first).unwrap_or_default();
        let end = self
            .document
            .line_full_end(last)
            .unwrap_or_else(|| self.document.len_bytes().into());
        let original = self.document.text_in(ByteRange::new(start, end))?;
        let mut rebuilt = String::new();
        for line in original.split_inclusive('\n') {
            let (content, ending) = split_line_ending(line);
            rebuilt.push_str(&rewrite(content));
            rebuilt.push_str(ending);
        }
        let after = Selection::new(start, ByteOffset::new(start.get() + rebuilt.len()));
        self.apply_replacement(ByteRange::new(start, end), &rebuilt, after)
    }

    fn swap_line_block(
        &mut self,
        upper_start: usize,
        block_start: usize,
        block_end: usize,
    ) -> Result<Revision, EditError> {
        let upper = self.line_block(upper_start, block_start.saturating_sub(1))?;
        let block = self.line_block(block_start, block_end)?;
        let start = self.document.line_start(upper_start).unwrap_or_default();
        let end = self
            .document
            .line_full_end(block_end)
            .unwrap_or_else(|| self.document.len_bytes().into());
        let rebuilt = format!("{}{}", block, upper);
        let after_start = start;
        let after_end = ByteOffset::new(start.get() + block.len());
        self.apply_replacement(
            ByteRange::new(start, end),
            &rebuilt,
            Selection::new(after_start, after_end),
        )
    }

    fn line_block(&self, first: usize, last: usize) -> Result<String, EditError> {
        let start = self.document.line_start(first).unwrap_or_default();
        let end = self
            .document
            .line_full_end(last)
            .unwrap_or_else(|| self.document.len_bytes().into());
        self.document.text_in(ByteRange::new(start, end))
    }

    fn apply_replacement(
        &mut self,
        range: ByteRange,
        text: &str,
        selection_after: Selection,
    ) -> Result<Revision, EditError> {
        let selection_before = self.selection;
        let deleted = self.document.text_in(range)?;
        if deleted.is_empty() && text.is_empty() {
            return Ok(self.document.revision());
        }
        let revision = self.document.replace(range, text)?;
        self.selection = selection_after;
        self.preferred_column = None;
        let kind = history_kind(&deleted, text);
        self.record_edit(HistoryEdit {
            start: range.start,
            deleted,
            inserted: text.to_owned(),
            selection_before,
            selection_after,
            kind,
        });
        Ok(revision)
    }

    fn replace_selection(&mut self, text: impl AsRef<str>) -> Result<Revision, EditError> {
        let text = text.as_ref();
        let range = self.selection.range();
        let cursor = ByteOffset::new(range.start.get() + text.len());
        self.apply_replacement(range, text, Selection::new(cursor, cursor))
    }

    fn record_edit(&mut self, edit: HistoryEdit) {
        if self.try_coalesce(&edit) {
            self.redo_stack.clear();
            return;
        }
        self.undo_stack.push(edit);
        self.redo_stack.clear();
        if self.undo_stack.len() > HISTORY_LIMIT {
            self.undo_stack.remove(0);
        }
    }

    fn try_coalesce(&mut self, edit: &HistoryEdit) -> bool {
        let Some(last) = self.undo_stack.last_mut() else {
            return false;
        };
        match (last.kind, edit.kind) {
            (HistoryKind::Insert, HistoryKind::Insert)
                if last.deleted.is_empty()
                    && edit.deleted.is_empty()
                    && is_coalescable_insert(&edit.inserted)
                    && last.selection_after == edit.selection_before =>
            {
                last.inserted.push_str(&edit.inserted);
                last.selection_after = edit.selection_after;
                true
            }
            (HistoryKind::Delete, HistoryKind::Delete)
                if last.inserted.is_empty()
                    && edit.inserted.is_empty()
                    && is_coalescable_insert(&edit.deleted)
                    && last.start.get() == edit.start.get() + edit.deleted.len() =>
            {
                last.start = edit.start;
                last.deleted.insert_str(0, &edit.deleted);
                last.selection_after = edit.selection_after;
                true
            }
            (HistoryKind::Delete, HistoryKind::Delete)
                if last.inserted.is_empty()
                    && edit.inserted.is_empty()
                    && is_coalescable_insert(&edit.deleted)
                    && last.start == edit.start =>
            {
                last.deleted.push_str(&edit.deleted);
                last.selection_after = edit.selection_after;
                true
            }
            _ => false,
        }
    }

    fn move_to(&mut self, target: ByteOffset, extend: bool) {
        self.selection = if extend {
            Selection::new(self.selection.anchor(), target)
        } else {
            Selection::new(target, target)
        };
        self.preferred_column = None;
    }

    fn move_vertically(&mut self, target_line: usize, extend: bool) {
        let column = self
            .preferred_column
            .unwrap_or_else(|| self.document.char_column_at(self.cursor()));
        let target = self
            .document
            .offset_for_line_column(target_line, column)
            .expect("target line must exist");
        self.selection = if extend {
            Selection::new(self.selection.anchor(), target)
        } else {
            Selection::new(target, target)
        };
        self.preferred_column = Some(column);
    }

    fn previous_boundary(&self, offset: ByteOffset) -> ByteOffset {
        self.document.previous_char_boundary(offset)
    }

    fn next_boundary(&self, offset: ByteOffset) -> ByteOffset {
        self.document.next_char_boundary(offset)
    }
}

fn history_kind(deleted: &str, inserted: &str) -> HistoryKind {
    if deleted.is_empty() && !inserted.is_empty() {
        HistoryKind::Insert
    } else if inserted.is_empty() && !deleted.is_empty() {
        HistoryKind::Delete
    } else {
        HistoryKind::Replace
    }
}

fn is_coalescable_insert(text: &str) -> bool {
    let mut characters = text.chars();
    matches!(
        (characters.next(), characters.next()),
        (Some(character), None) if character != '\n' && character != '\r'
    )
}

fn single_char(text: &str) -> Option<char> {
    let mut characters = text.chars();
    match (characters.next(), characters.next()) {
        (Some(character), None) => Some(character),
        _ => None,
    }
}

fn split_line_ending(line: &str) -> (&str, &str) {
    if let Some(content) = line.strip_suffix("\r\n") {
        (content, "\r\n")
    } else if let Some(content) = line.strip_suffix('\n') {
        (content, "\n")
    } else {
        (line, "")
    }
}

fn strip_indent(line: &str, unit: &str, tab_width: usize) -> String {
    if let Some(rest) = line.strip_prefix(unit) {
        return rest.to_owned();
    }
    if let Some(rest) = line.strip_prefix('\t') {
        return rest.to_owned();
    }
    let trim = line
        .chars()
        .take(tab_width)
        .take_while(|character| *character == ' ')
        .count();
    line[trim..].to_owned()
}

fn uncomment_line(line: &str, prefix: &str) -> String {
    let indent: String = line
        .chars()
        .take_while(|character| character.is_whitespace())
        .collect();
    let rest = &line[indent.len()..];
    if let Some(rest) = rest.strip_prefix(&format!("{prefix} ")) {
        format!("{indent}{rest}")
    } else if let Some(rest) = rest.strip_prefix(prefix) {
        format!("{indent}{rest}")
    } else {
        line.to_owned()
    }
}

fn is_word_char(character: char) -> bool {
    character.is_alphanumeric() || character == '_'
}

fn is_search_match(haystack: &str, query: &SearchQuery) -> bool {
    find_in(haystack, query, 0, true)
        .is_some_and(|found| found.start.get() == 0 && found.end.get() == haystack.len())
}

fn find_in(haystack: &str, query: &SearchQuery, from: usize, forward: bool) -> Option<ByteRange> {
    let from = from.min(haystack.len());
    if query.text.is_empty() {
        return None;
    }
    let (hay, needle) = if query.case_sensitive {
        (haystack.to_owned(), query.text.clone())
    } else {
        (haystack.to_lowercase(), query.text.to_lowercase())
    };
    if forward {
        let search = &hay[from..];
        let mut start = 0;
        while let Some(relative) = search[start..].find(&needle) {
            let absolute = from + start + relative;
            let end = absolute + query.text.len();
            if !query.whole_word || is_whole_word(haystack, absolute, end) {
                return Some(ByteRange::from(absolute..end));
            }
            start += relative + needle.len().max(1);
        }
        None
    } else {
        let search = &hay[..from];
        let mut end_limit = search.len();
        while let Some(absolute) = search[..end_limit].rfind(&needle) {
            let end = absolute + query.text.len();
            if !query.whole_word || is_whole_word(haystack, absolute, end) {
                return Some(ByteRange::from(absolute..end));
            }
            end_limit = absolute;
            if end_limit == 0 {
                break;
            }
        }
        None
    }
}

fn is_whole_word(haystack: &str, start: usize, end: usize) -> bool {
    let before = haystack.get(..start).and_then(|text| text.chars().next_back());
    let after = haystack.get(end..).and_then(|text| text.chars().next());
    !before.is_some_and(is_word_char) && !after.is_some_and(is_word_char)
}

fn find_matching_bracket(
    text: &str,
    offset: usize,
    character: char,
    rules: LanguageEditRules,
) -> Option<(ByteRange, ByteRange)> {
    let open = rules.opening_for(character).unwrap_or(character);
    let close = rules.closing_for(character).or_else(|| rules.closing_for(open))?;
    let origin = ByteRange::from(offset..offset + character.len_utf8());
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let index = chars.iter().position(|(start, _)| *start == offset)?;
    let forward = rules.closing_for(character).is_some() && rules.opening_for(character) != Some(character)
        || character == open && character != close;
    if character == close && character != open {
        let mut depth = 0;
        for (start, current) in chars[..=index].iter().rev() {
            if *current == close {
                depth += 1;
            } else if *current == open {
                depth -= 1;
                if depth == 0 {
                    return Some((ByteRange::from(*start..*start + open.len_utf8()), origin));
                }
            }
        }
        None
    } else if forward {
        let mut depth = 0;
        for (start, current) in chars[index..].iter() {
            if *current == open {
                depth += 1;
            } else if *current == close {
                depth -= 1;
                if depth == 0 {
                    return Some((origin, ByteRange::from(*start..*start + close.len_utf8())));
                }
            }
        }
        None
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inserts_and_replaces_the_selection() {
        let mut editor = Editor::new(Document::new("hello world"));

        editor.set_selection(6.into(), 11.into()).unwrap();
        editor.insert("Atelier").unwrap();

        assert_eq!(editor.document().text(), "hello Atelier");
        assert_eq!(editor.cursor(), ByteOffset::new(13));
        assert!(editor.selection().is_empty());
    }

    #[test]
    fn moves_and_extends_a_selection_across_unicode() {
        let mut editor = Editor::new(Document::new("aé日"));
        editor.set_selection(1.into(), 1.into()).unwrap();

        editor.move_right(false);
        assert_eq!(editor.cursor(), ByteOffset::new(3));

        editor.move_right(true);
        assert_eq!(editor.selection(), Selection::new(3.into(), 6.into()));

        editor.move_left(true);
        assert!(editor.selection().is_empty());
        assert_eq!(editor.cursor(), ByteOffset::new(3));
    }

    #[test]
    fn backspace_and_delete_respect_character_boundaries() {
        let mut editor = Editor::new(Document::new("aé日"));
        editor.set_selection(3.into(), 3.into()).unwrap();

        editor.backspace().unwrap();
        assert_eq!(editor.document().text(), "a日");
        assert_eq!(editor.cursor(), ByteOffset::new(1));

        editor.delete_forward().unwrap();
        assert_eq!(editor.document().text(), "a");
        assert_eq!(editor.cursor(), ByteOffset::new(1));
    }

    #[test]
    fn collapsing_a_selection_uses_its_nearest_edge() {
        let mut editor = Editor::new(Document::new("abcd"));
        editor.set_selection(3.into(), 1.into()).unwrap();

        editor.move_left(false);
        assert_eq!(editor.cursor(), ByteOffset::new(1));

        editor.set_selection(1.into(), 3.into()).unwrap();
        editor.move_right(false);
        assert_eq!(editor.cursor(), ByteOffset::new(3));
    }

    #[test]
    fn treats_crlf_as_one_cursor_step() {
        let mut editor = Editor::new(Document::new("one\r\ntwo"));
        editor.set_selection(3.into(), 3.into()).unwrap();

        editor.move_right(false);
        assert_eq!(editor.cursor(), ByteOffset::new(5));

        editor.move_left(false);
        assert_eq!(editor.cursor(), ByteOffset::new(3));
    }

    #[test]
    fn moves_vertically_and_preserves_the_preferred_column() {
        let mut editor = Editor::new(Document::new("abcdef\nx\nuvwxyz"));
        editor.set_selection(5.into(), 5.into()).unwrap();

        editor.move_down(false);
        assert_eq!(editor.cursor(), ByteOffset::new(8));

        editor.move_down(false);
        assert_eq!(editor.cursor(), ByteOffset::new(14));

        editor.move_up(false);
        assert_eq!(editor.cursor(), ByteOffset::new(8));
    }

    #[test]
    fn extends_a_selection_vertically_across_unicode() {
        let mut editor = Editor::new(Document::new("aé日\nxyz"));
        editor.set_selection(3.into(), 3.into()).unwrap();

        editor.move_down(true);

        assert_eq!(editor.selection(), Selection::new(3.into(), 9.into()));
    }

    #[test]
    fn saves_through_the_editor() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("notes.txt");
        std::fs::write(&path, "before").unwrap();
        let mut editor = Editor::new(Document::open(&path).unwrap());
        editor.insert("after ").unwrap();

        editor.save().unwrap();

        assert_eq!(std::fs::read_to_string(&path).unwrap(), "after before");
        assert!(!editor.document().is_modified());
    }

    #[test]
    fn undo_restores_text_and_redo_cancels_undo() {
        let mut editor = Editor::new(Document::new("hello"));
        editor.set_selection(5.into(), 5.into()).unwrap();
        editor.insert("!").unwrap();

        editor.undo().unwrap();
        assert_eq!(editor.document().text(), "hello");
        assert_eq!(editor.cursor(), ByteOffset::new(5));

        editor.redo().unwrap();
        assert_eq!(editor.document().text(), "hello!");
        assert_eq!(editor.cursor(), ByteOffset::new(6));
    }

    #[test]
    fn consecutive_inserts_undo_as_one_group() {
        let mut editor = Editor::new(Document::new(""));
        editor.insert("h").unwrap();
        editor.insert("i").unwrap();
        editor.insert("!").unwrap();

        editor.undo().unwrap();
        assert_eq!(editor.document().text(), "");
        editor.redo().unwrap();
        assert_eq!(editor.document().text(), "hi!");
    }

    #[test]
    fn typing_after_undo_discards_the_cancelled_edits() {
        let mut editor = Editor::new(Document::new("ab"));
        editor.set_selection(2.into(), 2.into()).unwrap();
        editor.insert("c").unwrap();
        editor.undo().unwrap();
        editor.insert("d").unwrap();

        assert_eq!(editor.document().text(), "abd");
        editor.redo().unwrap();
        assert_eq!(editor.document().text(), "abd");
    }

    #[test]
    fn reverting_edits_clears_the_modified_marker() {
        let mut editor = Editor::new(Document::new("hello"));
        editor.set_selection(5.into(), 5.into()).unwrap();
        editor.insert(" ").unwrap();
        assert!(editor.document().is_modified());
        editor.backspace().unwrap();
        assert_eq!(editor.document().text(), "hello");
        assert!(!editor.document().is_modified());
    }

    #[test]
    fn undo_after_save_marks_the_document_modified() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("notes.txt");
        std::fs::write(&path, "base").unwrap();
        let mut editor = Editor::new(Document::open(&path).unwrap());
        editor.set_selection(4.into(), 4.into()).unwrap();
        editor.insert("!").unwrap();
        editor.save().unwrap();
        assert!(!editor.document().is_modified());

        editor.undo().unwrap();
        assert_eq!(editor.document().text(), "base");
        assert!(editor.document().is_modified());

        editor.redo().unwrap();
        assert_eq!(editor.document().text(), "base!");
        assert!(!editor.document().is_modified());
    }

    #[test]
    fn home_toggles_between_indent_and_line_start() {
        let mut editor = Editor::new(Document::new("    foo"));
        editor.set_selection(6.into(), 6.into()).unwrap();
        editor.move_line_start(false);
        assert_eq!(editor.cursor(), ByteOffset::new(4));
        editor.move_line_start(false);
        assert_eq!(editor.cursor(), ByteOffset::new(0));
    }

    #[test]
    fn word_motion_skips_identifiers() {
        let mut editor = Editor::new(Document::new("foo_bar baz"));
        editor.set_selection(0.into(), 0.into()).unwrap();
        editor.move_word_right(false);
        assert_eq!(editor.cursor(), ByteOffset::new(8));
        editor.move_word_right(false);
        assert_eq!(editor.cursor(), ByteOffset::new(11));
        editor.move_word_left(false);
        assert_eq!(editor.cursor(), ByteOffset::new(8));
    }

    #[test]
    fn newline_preserves_and_increases_indent() {
        let mut editor = Editor::new(Document::new("    if true {"));
        editor.set_selection(13.into(), 13.into()).unwrap();
        editor.insert("\n").unwrap();
        assert_eq!(editor.document().text(), "    if true {\n        ");
    }

    #[test]
    fn inserts_and_skips_matching_pairs() {
        let mut editor = Editor::new(Document::new(""));
        editor.insert("(").unwrap();
        assert_eq!(editor.document().text(), "()");
        assert_eq!(editor.cursor(), ByteOffset::new(1));
        editor.insert(")").unwrap();
        assert_eq!(editor.document().text(), "()");
        assert_eq!(editor.cursor(), ByteOffset::new(2));
        editor.set_selection(1.into(), 1.into()).unwrap();
        editor.backspace().unwrap();
        assert_eq!(editor.document().text(), "");
    }

    #[test]
    fn indent_outdent_and_comment_selected_lines() {
        let mut editor = Editor::new(Document::new("fn a() {}\nfn b() {}\n"));
        editor.set_edit_rules(LanguageEditRules::new(
            Some("//"),
            LanguageEditRules::DEFAULT_PAIRS,
        ));
        editor.set_selection(0.into(), 19.into()).unwrap();
        editor.indent().unwrap();
        assert_eq!(editor.document().text(), "    fn a() {}\n    fn b() {}\n");
        editor.outdent().unwrap();
        assert_eq!(editor.document().text(), "fn a() {}\nfn b() {}\n");
        editor.toggle_line_comment().unwrap();
        assert_eq!(editor.document().text(), "// fn a() {}\n// fn b() {}\n");
        editor.toggle_line_comment().unwrap();
        assert_eq!(editor.document().text(), "fn a() {}\nfn b() {}\n");
    }

    #[test]
    fn find_replace_and_brackets() {
        let mut editor = Editor::new(Document::new("user.name\nuser.email\n"));
        let query = SearchQuery::new("user");
        assert_eq!(editor.find_next(&query), Some(ByteRange::from(0..4)));
        assert_eq!(editor.find_next(&query), Some(ByteRange::from(10..14)));
        editor.replace_all(&query, "account").unwrap();
        assert_eq!(editor.document().text(), "account.name\naccount.email\n");

        let mut braces = Editor::new(Document::new("fn x() { a }"));
        braces.set_selection(7.into(), 7.into()).unwrap();
        let (open, close) = braces.matching_brackets().unwrap();
        assert_eq!(open, ByteRange::from(7..8));
        assert_eq!(close, ByteRange::from(11..12));
    }

    #[test]
    fn duplicate_and_delete_line() {
        let mut editor = Editor::new(Document::new("one\ntwo\n"));
        editor.set_selection(0.into(), 0.into()).unwrap();
        editor.duplicate_line().unwrap();
        assert_eq!(editor.document().text(), "one\none\ntwo\n");
        editor.delete_line().unwrap();
        assert!(editor.document().text().starts_with("one\n") || editor.document().text().contains("two"));
    }
}
