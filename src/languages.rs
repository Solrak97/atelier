use std::{path::Path, sync::OnceLock};

use tree_sitter::Query;
use tree_sitter_highlight::{HighlightConfiguration, HighlightEvent, Highlighter};

use crate::analysis::SyntaxSession;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum HighlightKind {
    Attribute,
    Boolean,
    Comment,
    Constant,
    Constructor,
    Function,
    Keyword,
    Number,
    Operator,
    Property,
    Punctuation,
    String,
    Type,
    Variable,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct HighlightSpan {
    pub(crate) start: usize,
    pub(crate) end: usize,
    pub(crate) kind: HighlightKind,
}

pub(crate) trait LanguageExtension: Send + Sync {
    fn name(&self) -> &'static str;
    fn matches_path(&self, path: &Path) -> bool;
    fn highlight(&self, source: &str) -> Result<Vec<HighlightSpan>, String>;
    fn analysis_session(&self) -> Result<SyntaxSession, String>;
}

struct TreeSitterLanguageExtension {
    name: &'static str,
    file_extensions: &'static [&'static str],
    language: tree_sitter::Language,
    configuration: HighlightConfiguration,
    tags_query: &'static str,
}

impl TreeSitterLanguageExtension {
    fn new(
        name: &'static str,
        file_extensions: &'static [&'static str],
        language: tree_sitter::Language,
        highlights_query: &str,
        injections_query: &str,
        locals_query: &str,
        tags_query: &'static str,
    ) -> Self {
        if !tags_query.is_empty() {
            Query::new(&language, tags_query).unwrap_or_else(|error| {
                panic!("invalid {name} tags query: {error}");
            });
        }
        let mut configuration = HighlightConfiguration::new(
            language.clone(),
            name,
            highlights_query,
            injections_query,
            locals_query,
        )
        .unwrap_or_else(|error| panic!("invalid {name} highlight queries: {error}"));
        configuration.configure(HIGHLIGHT_NAMES);
        Self {
            name,
            file_extensions,
            language,
            configuration,
            tags_query,
        }
    }
}

impl LanguageExtension for TreeSitterLanguageExtension {
    fn name(&self) -> &'static str {
        self.name
    }

    fn matches_path(&self, path: &Path) -> bool {
        path.extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| {
                self.file_extensions
                    .iter()
                    .any(|candidate| extension.eq_ignore_ascii_case(candidate))
            })
    }

    fn highlight(&self, source: &str) -> Result<Vec<HighlightSpan>, String> {
        let mut highlighter = Highlighter::new();
        let events = highlighter
            .highlight(&self.configuration, source.as_bytes(), None, None, |_| None)
            .map_err(|error| format!("{} highlighting failed: {error}", self.name))?;
        let mut active_highlights = Vec::new();
        let mut spans: Vec<HighlightSpan> = Vec::new();

        for event in events {
            match event.map_err(|error| format!("{} highlighting failed: {error}", self.name))? {
                HighlightEvent::HighlightStart(highlight) => {
                    active_highlights.push(HIGHLIGHT_KINDS[highlight.0]);
                }
                HighlightEvent::HighlightEnd => {
                    active_highlights.pop();
                }
                HighlightEvent::Source { start, end } => {
                    let Some(&kind) = active_highlights.last() else {
                        continue;
                    };
                    if let Some(previous) = spans.last_mut()
                        && previous.end == start
                        && previous.kind == kind
                    {
                        previous.end = end;
                    } else {
                        spans.push(HighlightSpan { start, end, kind });
                    }
                }
            }
        }

        Ok(spans)
    }

    fn analysis_session(&self) -> Result<SyntaxSession, String> {
        SyntaxSession::new(self.language.clone(), self.tags_query)
    }
}

pub(crate) struct LanguageRegistry {
    extensions: Vec<Box<dyn LanguageExtension>>,
}

impl LanguageRegistry {
    fn built_in() -> Self {
        Self {
            extensions: vec![
                Box::new(TreeSitterLanguageExtension::new(
                    "Rust",
                    &["rs"],
                    tree_sitter_rust::LANGUAGE.into(),
                    tree_sitter_rust::HIGHLIGHTS_QUERY,
                    tree_sitter_rust::INJECTIONS_QUERY,
                    "",
                    tree_sitter_rust::TAGS_QUERY,
                )),
                Box::new(TreeSitterLanguageExtension::new(
                    "TOML",
                    &["toml"],
                    tree_sitter_toml_ng::LANGUAGE.into(),
                    tree_sitter_toml_ng::HIGHLIGHTS_QUERY,
                    "",
                    "",
                    TOML_TAGS_QUERY,
                )),
            ],
        }
    }

    pub(crate) fn extension_for_path(
        &'static self,
        path: &Path,
    ) -> Option<&'static dyn LanguageExtension> {
        self.extensions
            .iter()
            .find(|extension| extension.matches_path(path))
            .map(Box::as_ref)
    }
}

pub(crate) fn registry() -> &'static LanguageRegistry {
    static REGISTRY: OnceLock<LanguageRegistry> = OnceLock::new();
    REGISTRY.get_or_init(LanguageRegistry::built_in)
}

const TOML_TAGS_QUERY: &str = r#"
(pair (bare_key) @name) @definition.constant
(table (bare_key) @name) @definition.module
(table_array_element (bare_key) @name) @definition.module
"#;

const HIGHLIGHT_NAMES: &[&str] = &[
    "attribute",
    "boolean",
    "comment",
    "constant",
    "constructor",
    "function",
    "keyword",
    "number",
    "operator",
    "property",
    "punctuation",
    "string",
    "type",
    "variable",
];

const HIGHLIGHT_KINDS: &[HighlightKind] = &[
    HighlightKind::Attribute,
    HighlightKind::Boolean,
    HighlightKind::Comment,
    HighlightKind::Constant,
    HighlightKind::Constructor,
    HighlightKind::Function,
    HighlightKind::Keyword,
    HighlightKind::Number,
    HighlightKind::Operator,
    HighlightKind::Property,
    HighlightKind::Punctuation,
    HighlightKind::String,
    HighlightKind::Type,
    HighlightKind::Variable,
];

#[cfg(test)]
mod tests {
    use super::*;

    fn contains_highlight(
        source: &str,
        spans: &[HighlightSpan],
        text: &str,
        kind: HighlightKind,
    ) -> bool {
        spans
            .iter()
            .any(|span| span.kind == kind && &source[span.start..span.end] == text)
    }

    #[test]
    fn selects_built_in_extensions_by_file_extension() {
        assert_eq!(
            registry()
                .extension_for_path(Path::new("src/main.rs"))
                .map(LanguageExtension::name),
            Some("Rust")
        );
        assert_eq!(
            registry()
                .extension_for_path(Path::new("Cargo.toml"))
                .map(LanguageExtension::name),
            Some("TOML")
        );
        assert!(
            registry()
                .extension_for_path(Path::new("README.md"))
                .is_none()
        );
    }

    #[test]
    fn highlights_rust_source() {
        let source = "fn main() { let message = \"hello\"; // greeting\n}";
        let extension = registry().extension_for_path(Path::new("main.rs")).unwrap();
        let spans = extension.highlight(source).unwrap();

        assert!(contains_highlight(
            source,
            &spans,
            "fn",
            HighlightKind::Keyword
        ));
        assert!(contains_highlight(
            source,
            &spans,
            "\"hello\"",
            HighlightKind::String
        ));
        assert!(contains_highlight(
            source,
            &spans,
            "// greeting",
            HighlightKind::Comment
        ));
    }

    #[test]
    fn highlights_toml_source() {
        let source = "[package]\nname = \"atelier\"\nversion = 1\n";
        let extension = registry()
            .extension_for_path(Path::new("Cargo.toml"))
            .unwrap();
        let spans = extension.highlight(source).unwrap();

        assert!(contains_highlight(
            source,
            &spans,
            "\"atelier\"",
            HighlightKind::String
        ));
        assert!(contains_highlight(
            source,
            &spans,
            "1",
            HighlightKind::Number
        ));
    }
}
