//! UI-independent foundations for the Caduceus editor.

mod document;
mod editor;
mod paths;
mod project;
mod registry;

#[cfg(test)]
mod test_support;

pub use document::{
    ByteOffset, ByteRange, Document, DocumentId, DocumentSnapshot, EditError, Revision,
};
pub use editor::{Editor, Selection};
pub use paths::AppPaths;
pub use project::{PROJECT_METADATA_DIR, Project, ProjectEntry, ProjectEntryKind};
pub use registry::{ProjectRegistry, RecentProject};
