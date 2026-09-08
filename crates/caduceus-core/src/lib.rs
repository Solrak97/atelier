//! UI-independent foundations for the Caduceus editor.

mod document;
mod editor;
mod project;

pub use document::{
    ByteOffset, ByteRange, Document, DocumentId, DocumentSnapshot, EditError, Revision,
};
pub use editor::{Editor, Selection};
pub use project::{Project, ProjectEntry, ProjectEntryKind};
