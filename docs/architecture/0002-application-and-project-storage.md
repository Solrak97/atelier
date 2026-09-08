# 0002: Application and project storage

- Status: Accepted
- Date: 2026-09-08

## Context

Atelier started by treating the process working directory as the open project.
That is enough for `cargo run` during bootstrapping, but not for an installed
editor that must remember folders, keep its own configuration, and leave
project-specific files inside the folder they belong to.

Linux package installs cannot write next to the binary. Application state must
live in user-owned directories. Project state must not be mixed into those
global roots.

## Decision

### Application directories

Atelier uses the XDG Base Directory layout through the `directories` crate:

- config: `~/.config/atelier`
- data: `~/.local/share/atelier`
- cache: `~/.cache/atelier`

The three roots are created on first launch. This milestone only writes a
recent-project registry to the data directory as `projects.toml`. Config and
cache remain reserved for later settings and derived files.

Serde and TOML are introduced here as the persistence format, matching ADR 0001.

### Project directories

Opening a folder as a project creates `.atelier/` at the project root when it
does not already exist. That directory is the home for future project-local
state. The explorer hides `.atelier` the same way it hides `.git` and
`target`.

### Launch and sessions

Launching with no arguments restores the last open project when one was left
open. That path is stored in `projects.toml` as `last_open`. Closing the
project returns to the welcome screen and clears `last_open`, so the next
launch stays on welcome. Passing a directory or file still opens that project
immediately and records it as last open. Only one project is open in a window
at a time; closing the window still quits.

## Consequences

- `cargo run` no longer opens the repository by default; pass a path or use
  Open Folder.
- Tests must inject temporary application directories instead of writing to the
  real XDG roots.
- Later profile, cache, and language-server work have known homes and should
  not invent parallel storage layouts.

## Alternatives considered

- **Files next to the binary:** convenient for a zip-distributed portable
  build, but unusable for a system install and easy to lose on upgrade.
- **All state inside `.atelier/`:** cannot remember projects the user has not
  opened in this session, and cannot store app-wide preferences.
- **SQLite registry:** stronger for later querying, but heavier than a small
  TOML list at this stage.
