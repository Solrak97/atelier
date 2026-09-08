# 0003: Syntax analysis and in-file referencing

- Status: Accepted
- Date: 2026-09-08

## Context

Tree-sitter already colors Rust and TOML, but highlighting throws the parse
away. Lexical/syntactic analysis and code referencing are different problems
with different sources of truth. Mixing them into the highlighter, or waiting
for LSP before either exists, would leave the editor unable to answer "what is
this node?" or "where is this name bound in this file?"

Issue #10 still wants language-agnostic cores. Grammars must not leak into
`atelier-core`.

## Decision

### Layers

1. **Syntax trees** — each language extension retains an incremental Tree-sitter
   tree and projects it into a Atelier `SyntaxTree` (named node kinds and UTF-8
   byte ranges). Highlighting stays a separate consumer of the existing
   highlighter; paint is not rewritten onto the tree in this milestone.
2. **In-file symbols** — tags queries emit language-agnostic `SymbolTag` values.
   `atelier-core` binds them into a `SymbolGraph`: definitions, references,
   optional containers, and duplicate names in the same scope.
3. **LSP later** — a language server fills the same `SymbolGraph` types. It must
   not invent a second navigation model.

### Ownership

`atelier-core` owns `SyntaxTree`, `SymbolGraph`, and document snapshots. The
application crate owns parsers, grammars (`tree-sitter-rust`,
`tree-sitter-toml-ng`), and tags queries. Editor and document cores do not
import a grammar crate.

Edits are applied with Tree-sitter `InputEdit` derived from consecutive
snapshots so re-parse can be incremental. A full parse is used only when there
is no previous tree.

### First languages

Rust uses the grammar's `TAGS_QUERY`. TOML uses a thin key/table query: enough
to prove the same pipeline, not a complete TOML symbol model.

## Consequences

- Opening a Rust or TOML buffer keeps a tree and an in-file graph on each
  revision, even though jump commands and parse-error UI are later issues.
- Go-to-definition (#22) and parse-error display (#20) can consume these types
  without changing the document core.
- Project-wide indexing (#23) must be a separate store keyed by these same
  symbol types, not a second Tree-sitter walk with different names.

## Alternatives considered

- **Store `tree_sitter::Tree` in `atelier-core`:** true incrementality would
  live next to the document, but the core would depend on Tree-sitter and make
  grammar-free tests harder.
- **Rewrite highlighting onto the retained tree now:** extra paint risk with no
  user-visible gain for this slice.
- **Skip tags and wait for rust-analyzer:** jumps would work only while the
  server is up, and TOML/plain Tree-sitter languages would have no graph.
