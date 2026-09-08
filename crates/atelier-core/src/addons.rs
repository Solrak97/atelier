use std::path::Path;

/// Language-facing editor behavior that add-ons can supply.
///
/// Highlighting, diagnostics, and LSP stay outside this crate. An add-on only
/// tells the text editor how to indent, comment, and pair characters.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LanguageEditRules {
    line_comment: Option<&'static str>,
    pairs: &'static [(char, char)],
}

impl LanguageEditRules {
    pub const DEFAULT_PAIRS: &'static [(char, char)] =
        &[('(', ')'), ('[', ']'), ('{', '}'), ('"', '"')];

    pub const fn new(line_comment: Option<&'static str>, pairs: &'static [(char, char)]) -> Self {
        Self {
            line_comment,
            pairs,
        }
    }

    pub const fn line_comment(self) -> Option<&'static str> {
        self.line_comment
    }

    pub const fn pairs(self) -> &'static [(char, char)] {
        self.pairs
    }

    pub fn closing_for(self, open: char) -> Option<char> {
        self.pairs
            .iter()
            .find(|(left, _)| *left == open)
            .map(|(_, close)| *close)
    }

    pub fn opening_for(self, close: char) -> Option<char> {
        self.pairs
            .iter()
            .find(|(_, right)| *right == close)
            .map(|(open, _)| *open)
    }
}

impl Default for LanguageEditRules {
    fn default() -> Self {
        Self::new(None, Self::DEFAULT_PAIRS)
    }
}

/// Tab and indent preferences for the text editor core.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EditorSettings {
    tab_width: usize,
    indent_with_spaces: bool,
}

impl EditorSettings {
    pub const fn new(tab_width: usize, indent_with_spaces: bool) -> Self {
        Self {
            tab_width: if tab_width == 0 { 4 } else { tab_width },
            indent_with_spaces,
        }
    }

    pub const fn tab_width(self) -> usize {
        self.tab_width
    }

    pub const fn indent_with_spaces(self) -> bool {
        self.indent_with_spaces
    }

    pub fn indent_unit(self) -> String {
        if self.indent_with_spaces {
            " ".repeat(self.tab_width)
        } else {
            "\t".to_owned()
        }
    }
}

impl Default for EditorSettings {
    fn default() -> Self {
        Self::new(4, true)
    }
}

/// An add-on that customizes core editing for a language.
///
/// Intelligence (highlighting, trees, LSP) should implement a separate trait
/// in the IDE layer and only reuse these edit rules.
pub trait LanguageAddon: Send + Sync {
    fn id(&self) -> &'static str;
    fn matches_path(&self, path: &Path) -> bool;
    fn edit_rules(&self) -> LanguageEditRules {
        LanguageEditRules::default()
    }
}

/// Search options used by the editor feature layer.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SearchQuery {
    pub text: String,
    pub case_sensitive: bool,
    pub whole_word: bool,
}

impl SearchQuery {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            case_sensitive: false,
            whole_word: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_rules_pair_braces_but_not_apostrophes() {
        let rules = LanguageEditRules::default();
        assert_eq!(rules.closing_for('{'), Some('}'));
        assert_eq!(rules.closing_for('\''), None);
    }
}
