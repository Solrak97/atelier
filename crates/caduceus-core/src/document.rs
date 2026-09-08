use std::{
    error::Error,
    fmt, fs,
    io::{self, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use crop::Rope;
use tempfile::NamedTempFile;

static NEXT_DOCUMENT_ID: AtomicU64 = AtomicU64::new(1);

/// A stable identity for an open document, including an unsaved document.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DocumentId(u64);

impl DocumentId {
    pub const fn get(self) -> u64 {
        self.0
    }

    fn next() -> Self {
        let id = NEXT_DOCUMENT_ID
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |id| id.checked_add(1))
            .expect("document ID counter overflowed");
        Self(id)
    }
}

/// A UTF-8 byte offset into a document.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ByteOffset(usize);

impl ByteOffset {
    pub const fn new(value: usize) -> Self {
        Self(value)
    }

    pub const fn get(self) -> usize {
        self.0
    }
}

impl From<usize> for ByteOffset {
    fn from(value: usize) -> Self {
        Self::new(value)
    }
}

/// A half-open UTF-8 byte range.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct ByteRange {
    pub start: ByteOffset,
    pub end: ByteOffset,
}

impl ByteRange {
    pub const fn new(start: ByteOffset, end: ByteOffset) -> Self {
        Self { start, end }
    }

    pub const fn is_empty(self) -> bool {
        self.start.0 == self.end.0
    }

    pub const fn contains_range(self, other: Self) -> bool {
        self.start.0 <= other.start.0 && other.end.0 <= self.end.0
    }

    pub const fn len(self) -> usize {
        self.end.0.saturating_sub(self.start.0)
    }
}

impl From<std::ops::Range<usize>> for ByteRange {
    fn from(value: std::ops::Range<usize>) -> Self {
        Self::new(value.start.into(), value.end.into())
    }
}

/// A monotonically increasing document revision.
#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct Revision(u64);

impl Revision {
    pub const fn get(self) -> u64 {
        self.0
    }

    fn advance(&mut self) {
        self.0 = self
            .0
            .checked_add(1)
            .expect("document revision counter overflowed");
    }
}

/// A rejected document edit.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EditError {
    ReversedRange {
        start: ByteOffset,
        end: ByteOffset,
    },
    OutOfBounds {
        offset: ByteOffset,
        document_len: usize,
    },
    NotCharBoundary {
        offset: ByteOffset,
    },
}

impl fmt::Display for EditError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ReversedRange { start, end } => write!(
                formatter,
                "range start {} is after end {}",
                start.get(),
                end.get()
            ),
            Self::OutOfBounds {
                offset,
                document_len,
            } => write!(
                formatter,
                "byte offset {} is outside document length {}",
                offset.get(),
                document_len
            ),
            Self::NotCharBoundary { offset } => write!(
                formatter,
                "byte offset {} is not a UTF-8 character boundary",
                offset.get()
            ),
        }
    }
}

impl Error for EditError {}

/// Editable text and file identity independent from any UI framework.
pub struct Document {
    id: DocumentId,
    path: Option<PathBuf>,
    text: Rope,
    revision: Revision,
    modified: bool,
}

impl Document {
    pub fn new(text: impl AsRef<str>) -> Self {
        Self {
            id: DocumentId::next(),
            path: None,
            text: Rope::from(text.as_ref()),
            revision: Revision::default(),
            modified: false,
        }
    }

    pub fn with_path(path: impl Into<PathBuf>, text: impl AsRef<str>) -> Self {
        Self {
            path: Some(path.into()),
            ..Self::new(text)
        }
    }

    pub fn open(path: impl AsRef<Path>) -> io::Result<Self> {
        let path = fs::canonicalize(path)?;
        let text = fs::read_to_string(&path)?;
        Ok(Self::with_path(path, text))
    }

    pub const fn id(&self) -> DocumentId {
        self.id
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    pub const fn revision(&self) -> Revision {
        self.revision
    }

    pub const fn is_modified(&self) -> bool {
        self.modified
    }

    pub fn mark_saved(&mut self) {
        self.modified = false;
    }

    pub fn save(&mut self) -> io::Result<()> {
        let path = self.path.as_deref().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "cannot save a document without a file path",
            )
        })?;
        let parent = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        let mut temporary = NamedTempFile::new_in(parent)?;

        match fs::metadata(path) {
            Ok(metadata) => temporary
                .as_file()
                .set_permissions(metadata.permissions())?,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }

        for chunk in self.text.chunks() {
            temporary.write_all(chunk.as_bytes())?;
        }
        temporary.as_file().sync_all()?;
        temporary.persist(path).map_err(|error| error.error)?;

        #[cfg(unix)]
        fs::File::open(parent)?.sync_all()?;

        self.mark_saved();
        Ok(())
    }

    pub fn len_bytes(&self) -> usize {
        self.text.byte_len()
    }

    pub fn len_lines(&self) -> usize {
        self.text.line_len()
    }

    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    pub fn text(&self) -> String {
        self.text.chunks().collect()
    }

    pub fn line(&self, index: usize) -> Option<String> {
        (index < self.len_lines()).then(|| self.text.line(index).to_string())
    }

    pub(crate) fn previous_char_boundary(&self, offset: ByteOffset) -> ByteOffset {
        debug_assert!(self.validate_offset(offset).is_ok());

        if offset.get() >= 2
            && self.text.byte(offset.get() - 2) == b'\r'
            && self.text.byte(offset.get() - 1) == b'\n'
        {
            return (offset.get() - 2).into();
        }

        self.text
            .byte_slice(..offset.get())
            .chars()
            .next_back()
            .map(|character| (offset.get() - character.len_utf8()).into())
            .unwrap_or_default()
    }

    pub(crate) fn next_char_boundary(&self, offset: ByteOffset) -> ByteOffset {
        debug_assert!(self.validate_offset(offset).is_ok());

        if offset.get() + 1 < self.len_bytes()
            && self.text.byte(offset.get()) == b'\r'
            && self.text.byte(offset.get() + 1) == b'\n'
        {
            return (offset.get() + 2).into();
        }

        self.text
            .byte_slice(offset.get()..)
            .chars()
            .next()
            .map(|character| (offset.get() + character.len_utf8()).into())
            .unwrap_or_else(|| self.len_bytes().into())
    }

    pub(crate) fn line_index_at(&self, offset: ByteOffset) -> usize {
        debug_assert!(self.validate_offset(offset).is_ok());
        self.text.line_of_byte(offset.get())
    }

    pub(crate) fn last_line_index(&self) -> usize {
        self.text.line_of_byte(self.len_bytes())
    }

    pub(crate) fn char_column_at(&self, offset: ByteOffset) -> usize {
        debug_assert!(self.validate_offset(offset).is_ok());
        let line_start = self.text.byte_of_line(self.line_index_at(offset));
        self.text
            .byte_slice(line_start..offset.get())
            .chars()
            .count()
    }

    pub(crate) fn offset_for_line_column(
        &self,
        line_index: usize,
        char_column: usize,
    ) -> Option<ByteOffset> {
        if line_index > self.last_line_index() {
            return None;
        }

        let line_start = self.text.byte_of_line(line_index);
        if line_start == self.len_bytes() {
            return Some(line_start.into());
        }

        let byte_column = self
            .text
            .line(line_index)
            .chars()
            .take(char_column)
            .map(char::len_utf8)
            .sum::<usize>();
        Some((line_start + byte_column).into())
    }

    pub fn snapshot(&self) -> DocumentSnapshot {
        DocumentSnapshot {
            id: self.id,
            path: self.path.clone(),
            text: self.text.clone(),
            revision: self.revision,
            modified: self.modified,
        }
    }

    pub fn insert(
        &mut self,
        offset: ByteOffset,
        text: impl AsRef<str>,
    ) -> Result<Revision, EditError> {
        self.validate_offset(offset)?;

        let text = text.as_ref();
        if text.is_empty() {
            return Ok(self.revision);
        }

        self.text.insert(offset.get(), text);
        self.revision.advance();
        self.modified = true;
        Ok(self.revision)
    }

    pub fn delete(&mut self, range: ByteRange) -> Result<Revision, EditError> {
        self.validate_range(range)?;

        if range.is_empty() {
            return Ok(self.revision);
        }

        self.text.delete(range.start.get()..range.end.get());
        self.revision.advance();
        self.modified = true;
        Ok(self.revision)
    }

    pub fn replace(
        &mut self,
        range: ByteRange,
        text: impl AsRef<str>,
    ) -> Result<Revision, EditError> {
        self.validate_range(range)?;

        let text = text.as_ref();
        if range.is_empty() && text.is_empty() {
            return Ok(self.revision);
        }

        self.text.replace(range.start.get()..range.end.get(), text);
        self.revision.advance();
        self.modified = true;
        Ok(self.revision)
    }

    fn validate_range(&self, range: ByteRange) -> Result<(), EditError> {
        if range.start > range.end {
            return Err(EditError::ReversedRange {
                start: range.start,
                end: range.end,
            });
        }

        self.validate_offset(range.start)?;
        self.validate_offset(range.end)
    }

    pub(crate) fn validate_offset(&self, offset: ByteOffset) -> Result<(), EditError> {
        if offset.get() > self.len_bytes() {
            return Err(EditError::OutOfBounds {
                offset,
                document_len: self.len_bytes(),
            });
        }

        if !self.text.is_char_boundary(offset.get()) {
            return Err(EditError::NotCharBoundary { offset });
        }

        Ok(())
    }
}

impl Default for Document {
    fn default() -> Self {
        Self::new("")
    }
}

/// An immutable, cheaply cloned view of a document revision.
#[derive(Clone)]
pub struct DocumentSnapshot {
    id: DocumentId,
    path: Option<PathBuf>,
    text: Rope,
    revision: Revision,
    modified: bool,
}

impl DocumentSnapshot {
    pub const fn id(&self) -> DocumentId {
        self.id
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    pub const fn revision(&self) -> Revision {
        self.revision
    }

    pub const fn is_modified(&self) -> bool {
        self.modified
    }

    pub fn len_bytes(&self) -> usize {
        self.text.byte_len()
    }

    pub fn len_lines(&self) -> usize {
        self.text.line_len()
    }

    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    pub fn text(&self) -> String {
        self.text.chunks().collect()
    }

    pub fn line(&self, index: usize) -> Option<String> {
        (index < self.len_lines()).then(|| self.text.line(index).to_string())
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    use tempfile::tempdir;

    use super::*;

    #[test]
    fn edits_text_and_advances_revisions() {
        let mut document = Document::new("Hello world");

        assert!(!document.is_modified());
        assert_eq!(
            document.insert(ByteOffset::new(5), ", brave"),
            Ok(Revision(1))
        );
        assert!(document.is_modified());
        assert_eq!(document.text(), "Hello, brave world");

        assert_eq!(document.delete((5..12).into()), Ok(Revision(2)));
        assert_eq!(document.text(), "Hello world");

        assert_eq!(
            document.replace((6..11).into(), "Caduceus"),
            Ok(Revision(3))
        );
        assert_eq!(document.text(), "Hello Caduceus");
    }

    #[test]
    fn empty_edits_do_not_advance_the_revision() {
        let mut document = Document::new("text");

        assert_eq!(document.insert(0.into(), ""), Ok(Revision(0)));
        assert_eq!(document.delete((2..2).into()), Ok(Revision(0)));
        assert_eq!(document.replace((4..4).into(), ""), Ok(Revision(0)));
        assert!(!document.is_modified());
    }

    #[test]
    fn can_mark_a_modified_document_as_saved() {
        let mut document = Document::new("before");
        document
            .insert(document.len_bytes().into(), " after")
            .unwrap();

        document.mark_saved();

        assert!(!document.is_modified());
        assert_eq!(document.revision(), Revision(1));
    }

    #[test]
    fn saves_file_contents_and_clears_modified_state() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("document.txt");
        fs::write(&path, "before").unwrap();
        let mut document = Document::open(&path).unwrap();
        document.replace((0..6).into(), "after").unwrap();

        document.save().unwrap();

        assert_eq!(fs::read_to_string(&path).unwrap(), "after");
        assert!(!document.is_modified());
        assert_eq!(document.revision(), Revision(1));
    }

    #[test]
    fn failed_save_keeps_the_document_modified() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("occupied");
        fs::create_dir(&path).unwrap();
        let mut document = Document::with_path(&path, "content");
        document.insert(document.len_bytes().into(), "!").unwrap();

        assert!(document.save().is_err());

        assert!(path.is_dir());
        assert!(document.is_modified());
        assert_eq!(document.text(), "content!");
    }

    #[test]
    fn rejects_saving_a_document_without_a_path() {
        let mut document = Document::new("content");
        document.insert(document.len_bytes().into(), "!").unwrap();

        let error = document.save().unwrap_err();

        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        assert!(document.is_modified());
    }

    #[cfg(unix)]
    #[test]
    fn preserves_existing_file_permissions_when_saving() {
        use std::os::unix::fs::PermissionsExt;

        let directory = tempdir().unwrap();
        let path = directory.path().join("executable.sh");
        fs::write(&path, "before").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o750)).unwrap();
        let mut document = Document::open(&path).unwrap();
        document.replace((0..6).into(), "after").unwrap();

        document.save().unwrap();

        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o750
        );
    }

    #[test]
    fn rejects_invalid_utf8_boundaries_without_changing_text() {
        let mut document = Document::new("aé日");
        let original = document.text();

        assert_eq!(
            document.insert(2.into(), "!"),
            Err(EditError::NotCharBoundary { offset: 2.into() })
        );
        assert_eq!(
            document.delete((1..2).into()),
            Err(EditError::NotCharBoundary { offset: 2.into() })
        );
        assert_eq!(document.text(), original);
        assert_eq!(document.revision(), Revision(0));
    }

    #[test]
    fn rejects_reversed_and_out_of_bounds_ranges() {
        let mut document = Document::new("text");

        assert_eq!(
            document.delete(ByteRange::new(3.into(), 1.into())),
            Err(EditError::ReversedRange {
                start: 3.into(),
                end: 1.into(),
            })
        );
        assert_eq!(
            document.replace((0..5).into(), "other"),
            Err(EditError::OutOfBounds {
                offset: 5.into(),
                document_len: 4,
            })
        );
    }

    #[test]
    fn tracks_lf_and_crlf_lines() {
        let document = Document::new("one\r\ntwo\nthree");

        assert_eq!(document.len_lines(), 3);
        assert_eq!(document.line(0).as_deref(), Some("one"));
        assert_eq!(document.line(1).as_deref(), Some("two"));
        assert_eq!(document.line(2).as_deref(), Some("three"));
        assert_eq!(document.line(3), None);
    }

    #[test]
    fn snapshots_keep_their_original_revision_and_text() {
        let mut document = Document::new("before");
        let snapshot = document.snapshot();

        document.replace((0..6).into(), "after").unwrap();

        assert_eq!(snapshot.id(), document.id());
        assert_eq!(snapshot.revision(), Revision(0));
        assert!(!snapshot.is_modified());
        assert_eq!(snapshot.text(), "before");
        assert_eq!(document.revision(), Revision(1));
        assert!(document.is_modified());
        assert_eq!(document.text(), "after");
    }

    #[test]
    fn assigns_distinct_identities_to_documents() {
        let first = Document::default();
        let second = Document::default();

        assert_ne!(first.id(), second.id());
    }

    #[test]
    fn opens_utf8_files_and_preserves_their_path() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "caduceus-document-{}-{unique}.txt",
            std::process::id()
        ));

        fs::write(&path, "hello\nfrom disk").unwrap();
        let document = Document::open(&path).unwrap();
        fs::remove_file(&path).unwrap();

        assert_eq!(document.path(), Some(path.as_path()));
        assert_eq!(document.text(), "hello\nfrom disk");
        assert_eq!(document.len_lines(), 2);
    }
}
