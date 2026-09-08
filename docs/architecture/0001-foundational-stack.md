# 0001: Foundational stack

- Status: Accepted
- Date: 2026-09-04

## Context

Atelier is intended to be a focused, native IDE that grows through small,
usable milestones. Its foundation should provide responsive text rendering and
desktop integration without tying the editor's document model and behavior to a
particular UI framework.

The Rust desktop UI ecosystem is still evolving. The selected framework must
therefore be useful today without becoming the boundary of every subsystem.
Dependencies should be introduced when a milestone needs them rather than added
speculatively.

## Decision

### Language

Atelier will use stable Rust and the Rust 2024 edition. A newer stable toolchain
may be required when an adopted dependency requires it.

### User interface

[GPUI](https://github.com/zed-industries/zed/tree/main/crates/gpui) will provide
the native application and rendering layer. It is GPU accelerated, supports the
major desktop platforms, and has been exercised by a production code editor.

GPUI is pre-1.0 and can make breaking changes. Atelier will pin it deliberately
and update it as an explicit maintenance task. Framework-specific entities,
views, and tasks will stay in the UI layer; document state, editor commands, and
workspace behavior will use Atelier-owned types.

Linux is the first supported platform. The architecture should preserve a path
to macOS and Windows without requiring feature parity during early development.

### Document storage and positions

[Crop](https://github.com/nomad/crop) will back editable documents. Its UTF-8
byte indexing aligns with Rust strings and Tree-sitter ranges while providing
efficient edits and snapshots.

Atelier will define its own position and range types at subsystem boundaries.
Conversions between UTF-8 byte offsets, display positions, and LSP UTF-16
positions must be explicit and tested. No subsystem should silently assume that
these coordinate systems are interchangeable.

### Parsing and language intelligence

[Tree-sitter](https://tree-sitter.github.io/tree-sitter/) is the planned
incremental parsing and syntax-highlighting foundation.

Language Server Protocol support will likely use
[`async-lsp`](https://github.com/oxalica/async-lsp) and its `lsp-types`
integration. These dependencies will not be added until the first language
intelligence milestone.

### Concurrency

GPUI's foreground and background executors will be the default concurrency
model. UI state remains on the foreground thread, while file I/O, parsing, and
other expensive work run in background tasks.

An additional async runtime such as Tokio will only be introduced if a concrete
integration requires it. It will not become an implicit dependency of the
document core.

### Supporting libraries

Serde and TOML are the expected configuration and persistence tools, and
`tracing` is the expected diagnostics foundation. Like other dependencies, each
will be added with the first feature that uses it.

## Consequences

- The first application milestone can focus on a minimal GPUI window without
  prematurely implementing an editor shell.
- The document model can be developed and tested without a windowing system.
- GPUI upgrades may require adaptation inside the UI layer.
- Position conversion is a first-class correctness concern, especially when LSP
  support is introduced.
- Portability is preserved by architecture, but Linux remains the practical
  priority.
- The repository stays small at the beginning because planned libraries are not
  dependencies until they are needed.

## Alternatives considered

- **GTK4:** mature Linux integration, but less suitable for the desired
  cross-platform path and custom editor rendering.
- **Floem:** a relevant Rust-native, GPU-backed alternative with strong editor
  lineage, but less proven than GPUI for the chosen workload.
- **Iced:** a broad and approachable Rust GUI ecosystem, but less directly
  aligned with an IDE-grade editor surface.
- **Raw winit and wgpu:** maximum control at the cost of building a UI toolkit
  before building the IDE.
- **Ropey:** mature and Unicode-focused, but its code-point indexing does not
  avoid the explicit UTF-16 conversion required by LSP.
