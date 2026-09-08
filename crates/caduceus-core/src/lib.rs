//! UI-independent foundations for the Caduceus editor.

mod document;

pub use document::{
    ByteOffset, ByteRange, Document, DocumentId, DocumentSnapshot, EditError, Revision,
};
