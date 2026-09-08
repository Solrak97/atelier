# Atelier

Atelier is an experiment in building a focused, native IDE around the features
its author actually needs.

Modern development increasingly happens through agents, but sometimes you still
want to write and understand code yourself. Atelier is a place for that:
keyboard-first, deliberate, and free of built-in AI features.

Yes, it is being bootstrapped with AI in order to get away from AI. The irony is
not lost on us.

## Direction

The project will grow slowly through small, usable milestones rather than trying
to become a complete IDE all at once.

The initial direction is:

- Linux-first and native
- Fast startup and responsive editing
- A restrained interface with good keyboard navigation
- Language intelligence through standard tools such as LSP and Tree-sitter
- No accounts, telemetry, AI assistant, or extension marketplace
- Features chosen because they solve a real need

## Status

Atelier is at the beginning of its journey. Design and technical decisions will
be documented as they are made, and work will be tracked in the
[Atelier project](https://github.com/users/Solrak97/projects/8).

## Architecture

Significant technical choices are recorded as architecture decisions:

- [Foundational stack](docs/architecture/0001-foundational-stack.md) — Rust,
  GPUI, Crop, and the boundaries for future parsing and language intelligence
- [Application and project storage](docs/architecture/0002-application-and-project-storage.md)
  — XDG app directories, `.atelier/` project files, and the welcome session
- [Syntax analysis and in-file referencing](docs/architecture/0003-syntax-and-in-file-referencing.md)
  — retained Tree-sitter trees and a language-agnostic per-file symbol graph
- [Editor layers and add-ons](docs/architecture/0004-editor-layers-and-addons.md)
  — text core, editor features, IDE intelligence, and language edit rules

## Development

Atelier currently targets Linux and requires the latest stable Rust toolchain.
GPUI also needs a C/C++ build toolchain, Fontconfig, XKB, and either Wayland or
X11 development libraries.

On Arch Linux and derivatives:

```sh
sudo pacman -S --needed base-devel clang cmake pkgconf fontconfig freetype2 \
  libx11 libxcb libxkbcommon wayland vulkan-icd-loader
```

Build and run the current application with:

```sh
cargo run
```

Install a user-local launcher icon and desktop entry with:

```sh
./scripts/install-desktop.sh
```

That copies the current binary to `~/.local/bin/atelier` and registers
`atelier.desktop` plus hicolor icons under `~/.local/share`.

With no arguments, Atelier reopens the last project you left open. If you
closed that project, or have never opened one, it shows the welcome screen.
Use File → Open Folder, pick a recent project, or pass a directory or UTF-8
text file on the command line:

```sh
cargo run -- /path/to/project
cargo run -- README.md
```

Application files live in XDG user directories (`~/.config/atelier`,
`~/.local/share/atelier`, `~/.cache/atelier`). Opening a folder records it in
the recent-project list and creates a `.atelier/` directory inside that
project for later local state. `Ctrl+O` also opens a folder. File → Close Project
returns to the welcome screen. Unsaved files prompt to save, discard, or cancel.

The project tree starts expanded and uses folder and file-type icons;
click a folder to collapse or expand it.
Files open into persistent tabs. File → Close closes the
active tab. The editor is a text core (buffer, caret, selection, undo) with
editor commands on top: indent, comments, line moves, find/replace, and
bracket matching. Language highlighting and in-file navigation stay outside
that core. Tabs show a dirty mark; the editor title adds `•` when a file is
unsaved.

Editing keys include `Tab` / `Shift+Tab`, `Ctrl+/`, `Ctrl+Shift+K` to delete a
line, `Ctrl+Shift+D` to duplicate, `Alt+Up` / `Alt+Down` to move lines, word
and document motion with `Ctrl` plus arrows or Home/End, and `Ctrl+G` to go to
a line number selected in the buffer. `Ctrl+F` finds the current selection,
`F3` / `Shift+F3` walk matches, `Alt+C` / `Alt+W` toggle case and whole-word,
and `Ctrl+H` / `Ctrl+Alt+Enter` replace from the clipboard. `Escape` closes
the find bar. Long files and the project tree show a vertical scrollbar that
can be dragged or clicked. Closing a modified tab asks to save, discard, or
cancel. Rust and TOML files
keep an incremental Tree-sitter
syntax tree and an in-file definition/reference graph, and receive syntax
highlighting through the first built-in language extensions. Invalid syntax is
underlined and counted in the status bar. `F12` jumps to a
definition in the current file, `Shift+F12` cycles through its uses, and
`Alt+Left` goes back. `Ctrl+click` also jumps. Unresolved names show a status
message instead of leaving the file.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.
