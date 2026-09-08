# Caduceus

Caduceus is an experiment in building a focused, native IDE around the features
its author actually needs.

Modern development increasingly happens through agents, but sometimes you still
want to write and understand code yourself. Caduceus is a place for that:
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

Caduceus is at the beginning of its journey. Design and technical decisions will
be documented as they are made, and work will be tracked in the
[Caduceus project](https://github.com/users/Solrak97/projects/8).

## Architecture

Significant technical choices are recorded as architecture decisions:

- [Foundational stack](docs/architecture/0001-foundational-stack.md) — Rust,
  GPUI, Crop, and the boundaries for future parsing and language intelligence
- [Application and project storage](docs/architecture/0002-application-and-project-storage.md)
  — XDG app directories, `.caduceus/` project files, and the welcome session

## Development

Caduceus currently targets Linux and requires the latest stable Rust toolchain.
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

With no arguments, Caduceus opens a welcome screen. Open a folder from there,
or pass a directory or UTF-8 text file on the command line:

```sh
cargo run -- /path/to/project
cargo run -- README.md
```

Application files live in XDG user directories (`~/.config/caduceus`,
`~/.local/share/caduceus`, `~/.cache/caduceus`). Opening a folder records it in
the recent-project list and creates a `.caduceus/` directory inside that
project for later local state. `Ctrl+O` also opens a folder. Close Project on
the sidebar returns to the welcome screen; unsaved files prompt to save,
discard, or cancel.

The project tree opens files into persistent tabs. The editor supports typing,
newlines, horizontal and vertical movement, scrolling, selection, clipboard
shortcuts, and `Ctrl+S` to save. Long files and the project tree show a
vertical scrollbar that can be dragged or clicked. Closing a modified tab asks
to save, discard, or cancel. Rust and TOML files receive Tree-sitter syntax
highlighting through the first built-in language extensions.

## License

No license has been selected yet.
