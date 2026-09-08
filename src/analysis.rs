use atelier_core::{
    ByteRange, DocumentSnapshot, ParseError, ParseErrorKind, Revision, SymbolGraph, SymbolKind,
    SymbolTag, SyntaxNode, SyntaxTree,
};
use tree_sitter::{InputEdit, Parser, Point, Query, QueryCursor, StreamingIterator, Tree};

pub struct SyntaxSession {
    parser: Parser,
    tags: Option<Query>,
    tree: Option<Tree>,
    source: String,
    revision: Option<Revision>,
    syntax: Option<SyntaxTree>,
    symbols: SymbolGraph,
}

impl SyntaxSession {
    pub fn new(language: tree_sitter::Language, tags_query: &str) -> Result<Self, String> {
        let mut parser = Parser::new();
        parser
            .set_language(&language)
            .map_err(|error| format!("failed to set tree-sitter language: {error}"))?;
        let tags = if tags_query.is_empty() {
            None
        } else {
            Some(
                Query::new(&language, tags_query)
                    .map_err(|error| format!("invalid tags query: {error}"))?,
            )
        };
        Ok(Self {
            parser,
            tags,
            tree: None,
            source: String::new(),
            revision: None,
            syntax: None,
            symbols: SymbolGraph::default(),
        })
    }

    pub fn sync(&mut self, snapshot: &DocumentSnapshot) -> Result<(), String> {
        if self.revision == Some(snapshot.revision()) {
            return Ok(());
        }

        let source = snapshot.text();
        let tree = if let Some(previous) = self.tree.as_mut() {
            previous.edit(&input_edit_between(&self.source, &source));
            self.parser.parse(&source, Some(previous))
        } else {
            self.parser.parse(&source, None)
        }
        .ok_or_else(|| "tree-sitter produced no tree".to_owned())?;

        let syntax = SyntaxTree::with_errors(
            snapshot.revision(),
            convert_node(tree.root_node()),
            collect_parse_errors(tree.root_node()),
        );
        self.symbols = self
            .tags
            .as_ref()
            .map(|query| extract_symbols(query, &tree, &source))
            .unwrap_or_default();
        self.syntax = Some(syntax);
        self.tree = Some(tree);
        self.source = source;
        self.revision = Some(snapshot.revision());
        Ok(())
    }

    pub fn syntax(&self) -> Option<&SyntaxTree> {
        self.syntax.as_ref()
    }

    pub fn symbols(&self) -> &SymbolGraph {
        &self.symbols
    }
}

fn convert_node(node: tree_sitter::Node<'_>) -> SyntaxNode {
    let mut cursor = node.walk();
    let children = node.named_children(&mut cursor).map(convert_node).collect();
    SyntaxNode::new(
        node.kind(),
        ByteRange::from(node.start_byte()..node.end_byte()),
        children,
    )
}

fn collect_parse_errors(node: tree_sitter::Node<'_>) -> Vec<ParseError> {
    let mut errors = Vec::new();
    collect_parse_errors_into(node, &mut errors);
    errors
}

fn collect_parse_errors_into(node: tree_sitter::Node<'_>, errors: &mut Vec<ParseError>) {
    if node.is_error() {
        errors.push(ParseError::new(
            ByteRange::from(node.start_byte()..node.end_byte()),
            ParseErrorKind::Error,
        ));
        return;
    }
    if node.is_missing() {
        errors.push(ParseError::new(
            ByteRange::from(node.start_byte()..node.end_byte()),
            ParseErrorKind::Missing,
        ));
        return;
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_parse_errors_into(child, errors);
    }
}

fn extract_symbols(query: &Query, tree: &Tree, source: &str) -> SymbolGraph {
    let mut cursor = QueryCursor::new();
    let mut matches = cursor.matches(query, tree.root_node(), source.as_bytes());
    let mut tags = Vec::new();

    while let Some(query_match) = matches.next() {
        let mut name = None;
        let mut name_range = None;
        let mut definition_kind = None;
        let mut reference_kind = None;
        let mut range = None;
        let mut scope = None;

        for capture in query_match.captures() {
            let capture_name = query.capture_names()[capture.index as usize];
            let node = capture.node;
            match capture_name {
                "name" => {
                    name = Some(source[node.start_byte()..node.end_byte()].to_owned());
                    name_range = Some(ByteRange::from(node.start_byte()..node.end_byte()));
                }
                other if other.starts_with("definition.") => {
                    definition_kind = symbol_kind_from_tag(other);
                    range = Some(ByteRange::from(node.start_byte()..node.end_byte()));
                    scope = Some(scope_of(node));
                }
                other if other.starts_with("reference.") => {
                    reference_kind = symbol_kind_from_reference(other);
                    range = Some(ByteRange::from(node.start_byte()..node.end_byte()));
                    scope = Some(scope_of(node));
                }
                _ => {}
            }
        }

        let (Some(name), Some(name_range), Some(range), Some(scope)) =
            (name, name_range, range, scope)
        else {
            continue;
        };

        if let Some(kind) = definition_kind {
            tags.push(SymbolTag::definition(name, kind, name_range, range, scope));
        } else if let Some(kind) = reference_kind {
            tags.push(SymbolTag::reference(name, kind, name_range, range, scope));
        }
    }

    SymbolGraph::from_tags(tags)
}

fn scope_of(node: tree_sitter::Node<'_>) -> ByteRange {
    node.parent()
        .map(|parent| ByteRange::from(parent.start_byte()..parent.end_byte()))
        .unwrap_or_else(|| ByteRange::from(node.start_byte()..node.end_byte()))
}

fn symbol_kind_from_tag(capture: &str) -> Option<SymbolKind> {
    Some(match capture {
        "definition.function" => SymbolKind::Function,
        "definition.method" => SymbolKind::Method,
        "definition.class" | "definition.type" | "definition.struct" | "definition.enum" => {
            SymbolKind::Type
        }
        "definition.interface" => SymbolKind::Interface,
        "definition.module" | "definition.section" => SymbolKind::Module,
        "definition.macro" => SymbolKind::Macro,
        "definition.constant" => SymbolKind::Constant,
        "definition.field" => SymbolKind::Field,
        "definition.variable" => SymbolKind::Variable,
        _ => return None,
    })
}

fn symbol_kind_from_reference(capture: &str) -> Option<SymbolKind> {
    Some(match capture {
        "reference.call" => SymbolKind::Function,
        "reference.class" | "reference.implementation" => SymbolKind::Type,
        _ => return None,
    })
}

fn input_edit_between(old: &str, new: &str) -> InputEdit {
    let mut prefix = old
        .bytes()
        .zip(new.bytes())
        .take_while(|(left, right)| left == right)
        .count();
    while prefix > 0 && (!old.is_char_boundary(prefix) || !new.is_char_boundary(prefix)) {
        prefix -= 1;
    }

    let mut suffix = old.as_bytes()[prefix..]
        .iter()
        .rev()
        .zip(new.as_bytes()[prefix..].iter().rev())
        .take_while(|(left, right)| left == right)
        .count();
    while suffix > 0
        && (!old.is_char_boundary(old.len() - suffix) || !new.is_char_boundary(new.len() - suffix))
    {
        suffix -= 1;
    }

    let start_byte = prefix;
    let old_end_byte = old.len() - suffix;
    let new_end_byte = new.len() - suffix;
    InputEdit {
        start_byte,
        old_end_byte,
        new_end_byte,
        start_position: point_at(old, start_byte),
        old_end_position: point_at(old, old_end_byte),
        new_end_position: point_at(new, new_end_byte),
    }
}

fn point_at(source: &str, byte: usize) -> Point {
    let prefix = &source.as_bytes()[..byte];
    let row = prefix.iter().filter(|&&unit| unit == b'\n').count();
    let line_start = prefix
        .iter()
        .rposition(|&unit| unit == b'\n')
        .map(|index| index + 1)
        .unwrap_or(0);
    Point {
        row,
        column: byte - line_start,
    }
}

#[cfg(test)]
mod tests {
    use atelier_core::{DefinitionLookup, Document, ReferenceLookup, SymbolKind};

    use super::*;
    use crate::languages::registry;

    fn session_for(path: &str, source: &str) -> (Document, SyntaxSession) {
        let extension = registry()
            .extension_for_path(std::path::Path::new(path))
            .expect("language extension");
        let document = Document::with_path(path, source);
        let mut session = extension.analysis_session().expect("analysis session");
        session.sync(&document.snapshot()).unwrap();
        (document, session)
    }

    #[test]
    fn rust_tree_exists_after_open_and_updates_after_insert() {
        let (mut document, mut session) = session_for("lib.rs", "fn greet() {}\n");
        let before = session.syntax().expect("tree after open").clone();
        let greet = before.root().find_kind("function_item").unwrap().range();

        document
            .insert(document.len_bytes().into(), "fn main() {}\n")
            .unwrap();
        session.sync(&document.snapshot()).unwrap();
        let after = session.syntax().expect("tree after insert");

        assert_eq!(after.revision(), document.revision());
        assert!(after.root().range().end.get() > before.root().range().end.get());
        assert_eq!(
            after.root().find_kind("function_item").unwrap().range(),
            greet
        );
        assert_eq!(
            after
                .root()
                .children()
                .iter()
                .filter(|node| node.kind() == "function_item")
                .count(),
            2
        );
    }

    #[test]
    fn rust_function_and_two_calls_bind_in_file() {
        let source = "fn greet() {}\nfn main() {\n    greet();\n    greet();\n}\n";
        let (_document, session) = session_for("main.rs", source);
        let symbols = session.symbols();
        let greet = symbols
            .definitions_named("greet")
            .next()
            .expect("greet definition");

        assert_eq!(symbols.definitions_named("greet").count(), 1);
        assert_eq!(greet.kind(), SymbolKind::Function);
        assert_eq!(symbols.references_to(greet.id()).count(), 2);
        assert!(symbols.duplicates().is_empty());
    }

    #[test]
    fn rust_duplicate_functions_in_the_same_module_are_reported() {
        let (_document, session) = session_for("dup.rs", "fn foo() {}\nfn foo() {}\n");
        let symbols = session.symbols();

        assert_eq!(symbols.definitions_named("foo").count(), 2);
        assert_eq!(symbols.duplicates().len(), 1);
        assert_eq!(symbols.duplicates()[0].name(), "foo");
        assert_eq!(symbols.duplicates()[0].kind(), SymbolKind::Function);
        assert_eq!(symbols.duplicates()[0].definition_ids().len(), 2);
    }

    #[test]
    fn toml_tree_and_key_definitions() {
        let source = "[package]\nname = \"atelier\"\n";
        let (_document, session) = session_for("Cargo.toml", source);
        let syntax = session.syntax().expect("toml tree");
        let symbols = session.symbols();

        assert!(syntax.root().find_kind("table").is_some());
        assert!(syntax.root().find_kind("pair").is_some());
        assert_eq!(symbols.definitions_named("package").count(), 1);
        assert_eq!(
            symbols
                .definitions_named("name")
                .next()
                .map(|def| def.kind()),
            Some(SymbolKind::Constant)
        );
    }

    #[test]
    fn rust_go_to_definition_from_a_call_site() {
        let source = "fn greet() {}\nfn main() {\n    greet();\n    greet();\n}\n";
        let (_document, session) = session_for("main.rs", source);
        let symbols = session.symbols();
        let greet = symbols.definitions_named("greet").next().unwrap();
        let call = symbols.references_to(greet.id()).next().unwrap();

        assert_eq!(
            symbols.go_to_definition(call.name_range().start),
            DefinitionLookup::Found(greet.name_range())
        );
    }

    #[test]
    fn rust_find_references_visits_definition_and_uses() {
        let source = "fn greet() {}\nfn main() {\n    greet();\n    greet();\n}\n";
        let (_document, session) = session_for("main.rs", source);
        let symbols = session.symbols();
        let greet = symbols.definitions_named("greet").next().unwrap();
        let lookup = symbols.find_references(greet.name_range().start);
        let ReferenceLookup::Found(ranges) = lookup else {
            panic!("expected bound references");
        };

        assert_eq!(ranges.len(), 3);
        assert_eq!(ranges[0], greet.name_range());
        assert_eq!(
            ReferenceLookup::Found(ranges.clone()).next_after(greet.name_range().start),
            Some(ranges[1])
        );
    }

    #[test]
    fn rust_unresolved_call_does_not_panic() {
        let (_document, session) = session_for("main.rs", "fn main() { missing(); }\n");
        let symbols = session.symbols();
        let lookup = symbols.go_to_definition(symbols.references()[0].name_range().start);

        assert!(matches!(lookup, DefinitionLookup::Unresolved(name) if name == "missing"));
        assert!(matches!(
            symbols.find_references(symbols.references()[0].name_range().start),
            ReferenceLookup::Unresolved(_)
        ));
    }

    #[test]
    fn rust_parse_errors_are_reported_and_cleared() {
        let (mut document, mut session) = session_for("bad.rs", "fn main() { let x = ; }\n");
        let errors = session.syntax().expect("tree").errors();
        assert!(
            !errors.is_empty(),
            "invalid rust should produce at least one parse error"
        );
        assert!(
            errors.iter().any(|error| error.range().len() > 0),
            "error ranges should cover the invalid syntax"
        );

        document
            .replace(
                atelier_core::ByteRange::from(0..document.len_bytes()),
                "fn main() { let x = 1; }\n",
            )
            .unwrap();
        session.sync(&document.snapshot()).unwrap();

        assert!(
            session.syntax().expect("tree after fix").errors().is_empty(),
            "fixing the text should clear parse errors"
        );
    }
}
