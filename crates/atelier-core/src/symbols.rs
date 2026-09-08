use std::collections::HashMap;

use crate::{ByteOffset, ByteRange};

/// Stable identity for a definition inside one document's symbol graph.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct DefinitionId(u32);

impl DefinitionId {
    pub const fn get(self) -> u32 {
        self.0
    }
}

/// Language-agnostic classification for a named symbol.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum SymbolKind {
    Function,
    Method,
    Type,
    Interface,
    Module,
    Macro,
    Constant,
    Field,
    Variable,
}

/// Whether a tag marks a binding site or a use.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SymbolRole {
    Definition,
    Reference,
}

/// A raw definition or reference extracted by a language layer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SymbolTag {
    name: String,
    kind: SymbolKind,
    role: SymbolRole,
    name_range: ByteRange,
    range: ByteRange,
    scope: ByteRange,
}

impl SymbolTag {
    pub fn definition(
        name: impl Into<String>,
        kind: SymbolKind,
        name_range: ByteRange,
        range: ByteRange,
        scope: ByteRange,
    ) -> Self {
        Self {
            name: name.into(),
            kind,
            role: SymbolRole::Definition,
            name_range,
            range,
            scope,
        }
    }

    pub fn reference(
        name: impl Into<String>,
        kind: SymbolKind,
        name_range: ByteRange,
        range: ByteRange,
        scope: ByteRange,
    ) -> Self {
        Self {
            name: name.into(),
            kind,
            role: SymbolRole::Reference,
            name_range,
            range,
            scope,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub const fn kind(&self) -> SymbolKind {
        self.kind
    }

    pub const fn role(&self) -> SymbolRole {
        self.role
    }

    pub const fn name_range(&self) -> ByteRange {
        self.name_range
    }

    pub const fn range(&self) -> ByteRange {
        self.range
    }

    pub const fn scope(&self) -> ByteRange {
        self.scope
    }
}

/// A named binding site in a document.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Definition {
    id: DefinitionId,
    name: String,
    kind: SymbolKind,
    name_range: ByteRange,
    range: ByteRange,
    scope: ByteRange,
    container: Option<DefinitionId>,
}

impl Definition {
    pub const fn id(&self) -> DefinitionId {
        self.id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub const fn kind(&self) -> SymbolKind {
        self.kind
    }

    pub const fn name_range(&self) -> ByteRange {
        self.name_range
    }

    pub const fn range(&self) -> ByteRange {
        self.range
    }

    pub const fn scope(&self) -> ByteRange {
        self.scope
    }

    pub const fn container(&self) -> Option<DefinitionId> {
        self.container
    }
}

/// A use of a name, optionally bound to a definition in the same file.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Reference {
    name: String,
    kind: SymbolKind,
    name_range: ByteRange,
    range: ByteRange,
    definition: Option<DefinitionId>,
}

impl Reference {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub const fn kind(&self) -> SymbolKind {
        self.kind
    }

    pub const fn name_range(&self) -> ByteRange {
        self.name_range
    }

    pub const fn range(&self) -> ByteRange {
        self.range
    }

    pub const fn definition(&self) -> Option<DefinitionId> {
        self.definition
    }
}

/// Multiple definitions of the same name in the same scope.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DuplicateDefinitions {
    name: String,
    kind: SymbolKind,
    definition_ids: Vec<DefinitionId>,
}

impl DuplicateDefinitions {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub const fn kind(&self) -> SymbolKind {
        self.kind
    }

    pub fn definition_ids(&self) -> &[DefinitionId] {
        &self.definition_ids
    }
}

/// Result of asking for the definition under the caret.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DefinitionLookup {
    Found(ByteRange),
    Unresolved(String),
    Missing,
}

/// Result of asking for in-file uses of the symbol under the caret.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReferenceLookup {
    Found(Vec<ByteRange>),
    Unresolved(String),
    Missing,
}

impl ReferenceLookup {
    pub fn next_after(&self, offset: ByteOffset) -> Option<ByteRange> {
        let Self::Found(ranges) = self else {
            return None;
        };
        if ranges.is_empty() {
            return None;
        }
        if let Some(index) = ranges
            .iter()
            .position(|range| range.contains_offset(offset))
        {
            return Some(ranges[(index + 1) % ranges.len()]);
        }
        ranges
            .iter()
            .find(|range| range.start > offset)
            .copied()
            .or_else(|| ranges.first().copied())
    }
}

#[derive(Clone, Copy)]
enum Hit {
    Definition(DefinitionId),
    Reference(usize),
}

/// In-file definitions, references, and duplicate-name groups.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SymbolGraph {
    definitions: Vec<Definition>,
    references: Vec<Reference>,
    duplicates: Vec<DuplicateDefinitions>,
}

impl SymbolGraph {
    pub fn from_tags(tags: impl IntoIterator<Item = SymbolTag>) -> Self {
        let mut definitions_by_name_range = HashMap::new();
        let mut reference_tags = Vec::new();

        for tag in tags {
            match tag.role {
                SymbolRole::Definition => {
                    definitions_by_name_range
                        .entry(tag.name_range)
                        .and_modify(|existing: &mut SymbolTag| {
                            if kind_priority(tag.kind) > kind_priority(existing.kind) {
                                *existing = tag.clone();
                            }
                        })
                        .or_insert(tag);
                }
                SymbolRole::Reference => reference_tags.push(tag),
            }
        }

        let mut definitions: Vec<Definition> = definitions_by_name_range
            .into_values()
            .map(|tag| Definition {
                id: DefinitionId(0),
                name: tag.name,
                kind: tag.kind,
                name_range: tag.name_range,
                range: tag.range,
                scope: tag.scope,
                container: None,
            })
            .collect();
        definitions.sort_by_key(|definition| (definition.range.start, definition.range.end));
        for (index, definition) in definitions.iter_mut().enumerate() {
            definition.id = DefinitionId(index as u32);
        }

        for index in 0..definitions.len() {
            let range = definitions[index].range;
            let container = definitions
                .iter()
                .enumerate()
                .filter(|(other_index, other)| {
                    *other_index != index
                        && other.range.contains_range(range)
                        && other.range != range
                })
                .min_by_key(|(_, other)| other.range.len())
                .map(|(other_index, _)| DefinitionId(other_index as u32));
            definitions[index].container = container;
        }

        let mut grouped: HashMap<(String, SymbolKind, ByteRange), Vec<DefinitionId>> =
            HashMap::new();
        for definition in &definitions {
            grouped
                .entry((definition.name.clone(), definition.kind, definition.scope))
                .or_default()
                .push(definition.id);
        }
        let mut duplicates: Vec<DuplicateDefinitions> = grouped
            .into_iter()
            .filter(|(_, ids)| ids.len() > 1)
            .map(|((name, kind, _), definition_ids)| DuplicateDefinitions {
                name,
                kind,
                definition_ids,
            })
            .collect();
        duplicates.sort_by(|left, right| left.name.cmp(&right.name));

        let mut by_name: HashMap<String, Vec<usize>> = HashMap::new();
        for (index, definition) in definitions.iter().enumerate() {
            by_name
                .entry(definition.name.clone())
                .or_default()
                .push(index);
        }

        let references = reference_tags
            .into_iter()
            .map(|tag| {
                let candidates = by_name
                    .get(&tag.name)
                    .into_iter()
                    .flatten()
                    .copied()
                    .filter(|index| kinds_compatible(tag.kind, definitions[*index].kind))
                    .collect::<Vec<_>>();
                let definition = if candidates.len() == 1 {
                    Some(definitions[candidates[0]].id)
                } else {
                    None
                };
                Reference {
                    name: tag.name,
                    kind: tag.kind,
                    name_range: tag.name_range,
                    range: tag.range,
                    definition,
                }
            })
            .collect();

        Self {
            definitions,
            references,
            duplicates,
        }
    }

    pub fn definitions(&self) -> &[Definition] {
        &self.definitions
    }

    pub fn references(&self) -> &[Reference] {
        &self.references
    }

    pub fn duplicates(&self) -> &[DuplicateDefinitions] {
        &self.duplicates
    }

    pub fn definitions_named(&self, name: &str) -> impl Iterator<Item = &Definition> {
        self.definitions
            .iter()
            .filter(move |definition| definition.name == name)
    }

    pub fn references_to(&self, id: DefinitionId) -> impl Iterator<Item = &Reference> {
        self.references
            .iter()
            .filter(move |reference| reference.definition == Some(id))
    }

    pub fn definition(&self, id: DefinitionId) -> Option<&Definition> {
        self.definitions.get(id.get() as usize)
    }

    pub fn go_to_definition(&self, offset: ByteOffset) -> DefinitionLookup {
        match self.hit(offset) {
            None => DefinitionLookup::Missing,
            Some(Hit::Definition(id)) => self
                .definition(id)
                .map(|definition| DefinitionLookup::Found(definition.name_range))
                .unwrap_or(DefinitionLookup::Missing),
            Some(Hit::Reference(index)) => match self.references[index].definition {
                Some(id) => self
                    .definition(id)
                    .map(|definition| DefinitionLookup::Found(definition.name_range))
                    .unwrap_or(DefinitionLookup::Missing),
                None => DefinitionLookup::Unresolved(self.references[index].name.clone()),
            },
        }
    }

    pub fn find_references(&self, offset: ByteOffset) -> ReferenceLookup {
        let id = match self.hit(offset) {
            None => return ReferenceLookup::Missing,
            Some(Hit::Definition(id)) => id,
            Some(Hit::Reference(index)) => match self.references[index].definition {
                Some(id) => id,
                None => {
                    return ReferenceLookup::Unresolved(self.references[index].name.clone());
                }
            },
        };
        let mut ranges: Vec<ByteRange> = Vec::new();
        if let Some(definition) = self.definition(id) {
            ranges.push(definition.name_range);
        }
        ranges.extend(self.references_to(id).map(Reference::name_range));
        ranges.sort_by_key(|range| (range.start, range.end));
        ranges.dedup();
        ReferenceLookup::Found(ranges)
    }

    fn hit(&self, offset: ByteOffset) -> Option<Hit> {
        let mut best: Option<(usize, bool, Hit)> = None;
        for definition in &self.definitions {
            consider_hit(
                &mut best,
                offset,
                definition.name_range,
                true,
                Hit::Definition(definition.id),
            );
            consider_hit(
                &mut best,
                offset,
                definition.range,
                false,
                Hit::Definition(definition.id),
            );
        }
        for (index, reference) in self.references.iter().enumerate() {
            consider_hit(
                &mut best,
                offset,
                reference.name_range,
                true,
                Hit::Reference(index),
            );
            consider_hit(
                &mut best,
                offset,
                reference.range,
                false,
                Hit::Reference(index),
            );
        }
        best.map(|(_, _, hit)| hit)
    }
}

fn consider_hit(
    best: &mut Option<(usize, bool, Hit)>,
    offset: ByteOffset,
    range: ByteRange,
    is_name: bool,
    hit: Hit,
) {
    if !range.contains_offset(offset) {
        return;
    }
    let length = range.len();
    let replace = match best {
        None => true,
        Some((best_length, best_is_name, _)) => {
            length < *best_length || (length == *best_length && is_name && !*best_is_name)
        }
    };
    if replace {
        *best = Some((length, is_name, hit));
    }
}

fn kind_priority(kind: SymbolKind) -> u8 {
    match kind {
        SymbolKind::Method => 2,
        SymbolKind::Function => 1,
        _ => 0,
    }
}

fn kinds_compatible(reference: SymbolKind, definition: SymbolKind) -> bool {
    match reference {
        SymbolKind::Function => matches!(
            definition,
            SymbolKind::Function | SymbolKind::Method | SymbolKind::Macro
        ),
        SymbolKind::Type => matches!(
            definition,
            SymbolKind::Type | SymbolKind::Interface | SymbolKind::Module
        ),
        _ => reference == definition,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ByteOffset;

    fn range(start: usize, end: usize) -> ByteRange {
        ByteRange::new(ByteOffset::new(start), ByteOffset::new(end))
    }

    #[test]
    fn binds_two_references_to_one_definition() {
        let file = range(0, 40);
        let graph = SymbolGraph::from_tags([
            SymbolTag::definition(
                "greet",
                SymbolKind::Function,
                range(3, 8),
                range(0, 12),
                file,
            ),
            SymbolTag::reference(
                "greet",
                SymbolKind::Function,
                range(20, 25),
                range(20, 27),
                file,
            ),
            SymbolTag::reference(
                "greet",
                SymbolKind::Function,
                range(30, 35),
                range(30, 37),
                file,
            ),
        ]);

        let greet = graph.definitions_named("greet").next().unwrap();
        assert_eq!(graph.definitions().len(), 1);
        assert_eq!(graph.references_to(greet.id()).count(), 2);
        assert!(graph.duplicates().is_empty());
    }

    #[test]
    fn reports_duplicate_definitions_in_the_same_scope() {
        let file = range(0, 30);
        let graph = SymbolGraph::from_tags([
            SymbolTag::definition("foo", SymbolKind::Function, range(3, 6), range(0, 10), file),
            SymbolTag::definition(
                "foo",
                SymbolKind::Function,
                range(14, 17),
                range(11, 21),
                file,
            ),
        ]);

        assert_eq!(graph.definitions_named("foo").count(), 2);
        assert_eq!(graph.duplicates().len(), 1);
        assert_eq!(graph.duplicates()[0].name(), "foo");
        assert_eq!(graph.duplicates()[0].definition_ids().len(), 2);
    }

    #[test]
    fn prefers_method_tags_over_function_tags_for_the_same_name() {
        let impl_body = range(0, 40);
        let graph = SymbolGraph::from_tags([
            SymbolTag::definition(
                "len",
                SymbolKind::Function,
                range(10, 13),
                range(4, 20),
                impl_body,
            ),
            SymbolTag::definition(
                "len",
                SymbolKind::Method,
                range(10, 13),
                range(4, 20),
                impl_body,
            ),
        ]);

        assert_eq!(graph.definitions().len(), 1);
        assert_eq!(graph.definitions()[0].kind(), SymbolKind::Method);
    }

    fn greet_graph() -> SymbolGraph {
        let file = range(0, 40);
        SymbolGraph::from_tags([
            SymbolTag::definition(
                "greet",
                SymbolKind::Function,
                range(3, 8),
                range(0, 12),
                file,
            ),
            SymbolTag::reference(
                "greet",
                SymbolKind::Function,
                range(20, 25),
                range(20, 27),
                file,
            ),
            SymbolTag::reference(
                "greet",
                SymbolKind::Function,
                range(30, 35),
                range(30, 37),
                file,
            ),
        ])
    }

    #[test]
    fn jumps_from_a_call_site_to_the_definition() {
        let graph = greet_graph();
        assert_eq!(
            graph.go_to_definition(ByteOffset::new(21)),
            DefinitionLookup::Found(range(3, 8))
        );
    }

    #[test]
    fn find_references_visits_the_definition_and_each_use() {
        let graph = greet_graph();
        let lookup = graph.find_references(ByteOffset::new(21));
        assert_eq!(
            lookup,
            ReferenceLookup::Found(vec![range(3, 8), range(20, 25), range(30, 35)])
        );
        assert_eq!(lookup.next_after(ByteOffset::new(21)), Some(range(30, 35)));
        assert_eq!(lookup.next_after(ByteOffset::new(32)), Some(range(3, 8)));
    }

    #[test]
    fn unresolved_names_do_not_panic() {
        let file = range(0, 20);
        let graph = SymbolGraph::from_tags([SymbolTag::reference(
            "missing",
            SymbolKind::Function,
            range(0, 7),
            range(0, 9),
            file,
        )]);

        assert_eq!(
            graph.go_to_definition(ByteOffset::new(1)),
            DefinitionLookup::Unresolved("missing".into())
        );
        assert_eq!(
            graph.find_references(ByteOffset::new(1)),
            ReferenceLookup::Unresolved("missing".into())
        );
        assert_eq!(
            graph.go_to_definition(ByteOffset::new(19)),
            DefinitionLookup::Missing
        );
    }
}
