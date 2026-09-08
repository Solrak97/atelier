use crate::{ByteOffset, ByteRange, Document, EditError, Revision};

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
}

impl Editor {
    pub fn new(document: Document) -> Self {
        Self {
            document,
            selection: Selection::default(),
        }
    }

    pub const fn document(&self) -> &Document {
        &self.document
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

    pub fn select_all(&mut self) {
        self.selection = Selection::new(0.into(), self.document.len_bytes().into());
    }

    pub fn insert(&mut self, text: impl AsRef<str>) -> Result<Revision, EditError> {
        self.replace_selection(text)
    }

    pub fn backspace(&mut self) -> Result<Revision, EditError> {
        if self.selection.is_empty() {
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

    fn replace_selection(&mut self, text: impl AsRef<str>) -> Result<Revision, EditError> {
        let text = text.as_ref();
        let range = self.selection.range();
        let revision = self.document.replace(range, text)?;
        let cursor = ByteOffset::new(range.start.get() + text.len());
        self.selection = Selection::new(cursor, cursor);
        Ok(revision)
    }

    fn move_to(&mut self, target: ByteOffset, extend: bool) {
        self.selection = if extend {
            Selection::new(self.selection.anchor(), target)
        } else {
            Selection::new(target, target)
        };
    }

    fn previous_boundary(&self, offset: ByteOffset) -> ByteOffset {
        self.document.previous_char_boundary(offset)
    }

    fn next_boundary(&self, offset: ByteOffset) -> ByteOffset {
        self.document.next_char_boundary(offset)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inserts_and_replaces_the_selection() {
        let mut editor = Editor::new(Document::new("hello world"));

        editor.set_selection(6.into(), 11.into()).unwrap();
        editor.insert("Caduceus").unwrap();

        assert_eq!(editor.document().text(), "hello Caduceus");
        assert_eq!(editor.cursor(), ByteOffset::new(14));
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
}
