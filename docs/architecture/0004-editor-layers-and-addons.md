# 0004: Editor layers and add-ons

- Status: Accepted
- Date: 2026-09-08

## Context

A usable editor needs a solid buffer, caret, selection, and undo model before
it needs language intelligence. Mixing those primitives with highlighting,
diagnostics, or LSP would make later features fight the text representation.

Atelier also needs a place for language-specific *editing* behavior (comments,
pairs) without treating every language feature as part of the text core.

## Decision

The product is three models plus a UI, not one crate that does everything:

```text
┌─────────────────────────────────┐
│              IDE                │
├─────────────────────────────────┤
│ LSP │ Debugger │ Git │ Terminal │
├─────────────────────────────────┤
│       Editor Features           │
│ multi-cursor, search, commands  │
├─────────────────────────────────┤
│        Text Editor Core         │
│ buffer │ cursor │ selection     │
│ undo   │ layout │ viewport      │
└─────────────────────────────────┘
```

1. **Text editor core** (`atelier_core::Document`, `Editor`) — rope buffer,
   UTF-8 offsets, selections, edits, undo/redo, indent, pairs, line operations,
   find/replace, and matching brackets. Layout and viewport stay UI-owned, but
   they consume this model rather than inventing a second one.
2. **Editor features** — commands and search options (`SearchQuery`,
   `EditorSettings`) that are still text operations. Multiple cursors can later
   become several selections over the same edit/undo path.
3. **IDE intelligence** — Tree-sitter highlighting, parse errors, in-file
   symbols, and later LSP, debug, git, and the terminal. These sit on top of
   snapshots and ranges. They do not own the buffer.
4. **UI** — the GPUI application: paint, keybindings, menus, find bar, and
   file dialogs.

Language add-ons start as `LanguageAddon`: a path match plus
`LanguageEditRules` (line comment marker and auto-pairs). Highlighting and
syntax sessions implement a separate application trait and only reuse those
edit rules. Rust does not auto-close `'`, because that fights lifetimes.

## Consequences

- New editing commands land in `atelier-core` and are covered there first.
- Highlighting, hover, go-to-definition via LSP, formatting, and diagnostics
  stay out of the editor core.
- A future extension host can grow from `LanguageAddon` plus feature traits; it
  is not a marketplace and is not required for the first languages.
- Regex search, save-as, horizontal scrolling, and multiple cursors are
  deferred. They should still apply edits through the same replacement/history
  API.
