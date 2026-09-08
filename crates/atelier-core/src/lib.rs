//! UI-independent foundations for Atelier.
//!
//! The crate is three models, not one bag of types:
//! - **text** — `Document`, offsets, and edits
//! - **editor** — selections, history, and editing commands
//! - **workspace / intelligence** — projects and language-agnostic analysis
//!
//! The GPUI application is the UI layer. Language highlighting and LSP are
//! add-ons that sit on top of the editor and only share [`LanguageEditRules`]
//! with the text core.

mod addons;
mod document;
mod editor;
mod paths;
mod project;
mod registry;
mod symbols;
mod syntax;

#[cfg(test)]
mod test_support;

pub use addons::{
    EditorSettings, LanguageAddon, LanguageEditRules, SearchQuery,
};
pub use document::{
    ByteOffset, ByteRange, Document, DocumentId, DocumentSnapshot, EditError, Revision,
    is_unsupported_text,
};
pub use editor::{Editor, Selection};
pub use paths::AppPaths;
pub use project::{PROJECT_METADATA_DIR, Project, ProjectEntry, ProjectEntryKind};
pub use registry::{ProjectRegistry, RecentProject};
pub use symbols::{
    Definition, DefinitionId, DefinitionLookup, DuplicateDefinitions, Reference, ReferenceLookup,
    SymbolGraph, SymbolKind, SymbolRole, SymbolTag,
};
pub use syntax::{ParseError, ParseErrorKind, SyntaxNode, SyntaxTree};
