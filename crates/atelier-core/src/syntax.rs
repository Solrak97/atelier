use crate::{ByteRange, Revision};

/// A language-agnostic named syntax node with UTF-8 byte ranges.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxNode {
    kind: String,
    range: ByteRange,
    children: Vec<SyntaxNode>,
}

impl SyntaxNode {
    pub fn new(kind: impl Into<String>, range: ByteRange, children: Vec<Self>) -> Self {
        Self {
            kind: kind.into(),
            range,
            children,
        }
    }

    pub fn kind(&self) -> &str {
        &self.kind
    }

    pub const fn range(&self) -> ByteRange {
        self.range
    }

    pub fn children(&self) -> &[SyntaxNode] {
        &self.children
    }

    pub fn find_kind(&self, kind: &str) -> Option<&SyntaxNode> {
        if self.kind == kind {
            return Some(self);
        }
        self.children.iter().find_map(|child| child.find_kind(kind))
    }

    pub fn named_node_count(&self) -> usize {
        1 + self
            .children
            .iter()
            .map(Self::named_node_count)
            .sum::<usize>()
    }
}

/// A syntax tree for one document revision.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxTree {
    revision: Revision,
    root: SyntaxNode,
}

impl SyntaxTree {
    pub fn new(revision: Revision, root: SyntaxNode) -> Self {
        Self { revision, root }
    }

    pub const fn revision(&self) -> Revision {
        self.revision
    }

    pub const fn root(&self) -> &SyntaxNode {
        &self.root
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
    fn finds_named_nodes_after_construction() {
        let function = SyntaxNode::new(
            "function_item",
            range(0, 12),
            vec![SyntaxNode::new("identifier", range(3, 7), Vec::new())],
        );
        let tree = SyntaxTree::new(Revision::default(), function);

        assert_eq!(tree.root().kind(), "function_item");
        assert_eq!(
            tree.root().find_kind("identifier").map(SyntaxNode::kind),
            Some("identifier")
        );
        assert_eq!(tree.root().named_node_count(), 2);
    }
}
