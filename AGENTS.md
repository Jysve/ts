# Project Guide for Agents

## Overview

`ts` is a single-binary, interactive terminal todo application written in Rust. It uses the Rust 2024 edition and organizes todos by calendar date. The application is entirely local: it has no server, network protocol, database, or external service. Its user interface runs in the terminal's alternate screen and each day's data is persisted as TOML.

The package and binary are both named `ts`. User-facing documentation is in `README.md`; this file describes the implementation and the constraints to preserve when changing it.

## Repository structure

```text
.
├── Cargo.toml          Package metadata, dependencies, and release profile
├── Cargo.lock          Resolved dependency versions; use locked builds
├── README.md           Installation, usage, controls, storage, and development docs
├── AGENTS.md           Architecture and contributor guidance for coding agents
├── LICENSE             Apache License 2.0
├── install.sh          Locked release build and binary installation script
├── .gitignore          Git exclusions, including Cargo build output
├── src/
│   ├── main.rs         CLI handling, terminal setup, event loop, and cleanup
│   ├── app.rs          Application state, modes, keyboard handling, and commands
│   ├── model.rs        Serializable data model and todo operations
│   ├── storage.rs      Data-location resolution, daily TOML loading/saving, and JSON migration
│   └── ui.rs           ratatui rendering for the main view and modal dialogs
└── target/             Generated Cargo artifacts; never edit or commit these
```

There is no separate library target. Modules are private to the binary crate and communicate through `crate::...` imports.

## Module responsibilities

### `src/main.rs`

This is the process entry point and terminal lifecycle owner.

- `handle_arguments` implements `-h`/`--help`, `-V`/`--version`, and `--list`. Any other argument is an error; the interactive UI takes no positional arguments.
- `main` resolves the data location, loads the store, and either prints all incomplete todos for `--list` or constructs `App` with the current local date and enters the terminal session.
- `TerminalSession` enables raw mode, enters the alternate screen, hides the cursor, and owns the ratatui terminal backed by crossterm.
- The run loop draws the current state, waits for a key event, forwards key presses and repeats to `App::handle_key`, and exits when `App::should_quit` is set.
- `TerminalSession::drop` restores raw mode, cursor visibility, and the normal screen. Preserve this cleanup behavior when changing startup or event-loop code so errors do not leave the terminal corrupted.

Keep CLI help synchronized with the actual controls in `app.rs` and the key tables in `README.md`.

### `src/app.rs`

This module contains interaction logic and mutable application state.

`App` owns:

- The persistent `Store`
- The selected date and selected todo ID
- The current modal `Mode`
- An optional status or error message
- The quit flag
- The private `DataLocation` used for persistence

`Mode` defines six interaction states:

- `Normal`: todo selection, date navigation, and top-level commands
- `Calendar(CalendarState)`: highlighted-date navigation
- `Adding(AddForm)`: shared form for creating and editing title/priority
- `Details(DetailsForm)`: multiline details editing and scrolling
- `Dependencies(DependencyForm)`: cross-date prerequisite selection and removal
- `Deleting(u64)`: confirmation for one todo

`App::handle_key` handles global `Ctrl+C`, clears the previous status, and dispatches according to a clone of the current mode. Individual handlers update state and call storage only after persistent changes. Navigation and selection alone do not write files.

The model is intentionally manipulated through `Store` methods rather than by duplicating todo mutation rules in event handlers. Selection is tracked by stable todo ID because changing priority or completion can reorder the visible list. Date helpers use checked arithmetic, and month navigation clamps the day to the target month's length.

Normal mode and the calendar use Colemak-DH `u`/`e`/`n`/`i` for up/down/left/right alongside arrow keys. Common normal-mode actions use `a` (add), `r` (edit), `s` (details), and `t` (today); `d` manages prerequisites. The details editor retains arrow-key scrolling because letter keys insert text.

When adding a command, update all relevant layers: key dispatch, any modal rendering or footer help, CLI help, and README documentation. Preserve mode-specific `Esc` behavior and immediate persistence semantics.

### `src/model.rs`

This module defines the serialized domain model and core operations.

- `Priority` is `High`, `Medium`, `Normal`, or `Low`; serde stores lowercase names. Its rank controls display order and its offset supports cyclic priority selection. Missing priorities still default to `Medium`.
- `Todo` contains a globally allocated `u64` ID, title, priority, completion flag, fixed-offset creation timestamp, details string, and prerequisite ID list. Defaults on serde fields provide backward compatibility with older JSON and TOML files; empty prerequisite lists are not serialized.
- `Store` contains `next_id` and a `BTreeMap<NaiveDate, Vec<Todo>>`. The ordered map gives deterministic date serialization.
- `normalize_next_id` prevents ID reuse after loading old or manually edited data.
- `todos_on` returns references sorted by priority, then completion state, then ID. The underlying vector is not physically reordered. `all_todos` provides date-ordered global lookup for prerequisite selection.
- CRUD methods operate on a specific date and ID. Deleting the final todo removes the empty date entry. Prerequisite changes reject self-references and cycles. Completion is blocked by unfinished or missing prerequisite IDs, and deletion is blocked while another todo references the target.

Keep serialization compatibility in mind: renaming fields or changing enum representations changes the on-disk format. New fields should normally have serde defaults. IDs must remain unique across dates and must not be reset when a date becomes empty.

### `src/storage.rs`

This module owns all filesystem interaction for todo data. `DataLocation` distinguishes the default daily TOML directory from the backward-compatible single JSON file selected by `TS_DATA_FILE`.

Location precedence is:

1. Nonempty `TS_DATA_FILE`, selecting legacy single-file JSON mode
2. Nonempty `TS_DATA_DIR`, selecting a daily TOML directory
3. Nonempty `XDG_CONFIG_HOME`, plus `ts`
4. Nonempty `HOME`, plus `.config/ts`

If none is available, startup fails with an actionable error. In daily mode, `metadata.toml` stores `next_id`. Each day is stored under a subdirectory named with the first 16 hexadecimal characters of the SHA-256 hash of `YYYY-MM`; the file is the equivalent hash of `YYYY-MM-DD` plus `.toml`. Every daily file includes its plain ISO date so loading can reconstruct and validate the `Store`.

When `metadata.toml` is absent, migration checks `todos.json` in the daily data directory, then `XDG_DATA_HOME/ts/todos.json` or `HOME/.local/share/ts/todos.json`. A successfully decoded legacy store is saved as daily TOML before its metadata. Legacy files are retained. An explicit `TS_DATA_FILE` disables migration and continues to use JSON.

`save_day` writes only the changed day and metadata in daily mode, removing the daily file when its final todo is deleted. Writes create parent directories, serialize pretty TOML or JSON, write and sync a temporary sibling file named with the process ID, and rename it over the destination. Failed saves attempt to remove the temporary file. Preserve atomic replacement and path-specific error context when changing persistence code.

Loading normalizes `next_id`; saving itself does not mutate the store. Corrupt TOML or JSON is reported rather than silently replaced.

### `src/ui.rs`

This is the rendering layer. It reads `App` and its mode but does not mutate application state or perform storage operations.

- `draw` lays out the header, todo list, status line, and shortcut footer, then overlays the active modal.
- The header shows the selected date and active/completed counts.
- The todo list displays selection, checkbox, priority, prerequisite status, title, and creation time when width permits.
- Modal renderers cover the calendar, add/edit form, details editor, prerequisite manager, and deletion confirmation.
- Calendar dates without todos are dark gray, dates with todos are white, and today has an orange background. Highlighted dates retain a contrasting selection background.
- `unicode-width` helpers truncate titles by terminal cell width rather than byte count or character count.

Keep rendering side-effect free. Use ratatui layout constraints rather than terminal-size assumptions, retain popup bounds clamping, and account for wide Unicode characters when calculating visible widths. Any new mode requires a corresponding rendering branch.

### `install.sh`

The Bash installer resolves its own project directory, runs `cargo build --release --locked`, and installs `target/release/ts` with mode `0755`. Its optional first argument is the complete destination path; the default is `/usr/bin/ts`. It installs directly when the destination directory is writable, otherwise it uses `sudo` if available. It does not create the destination directory.

## Runtime and data flow

Startup follows this path:

```text
CLI arguments → data_location() → load()/legacy migration
             → --list output, or App::new() → TerminalSession::new() → draw/event loop
```

A typical persistent command follows this path:

```text
crossterm key event → App mode handler → Store mutation → storage::save_day()
                   → status/selection update → ui::draw()
```

The UI is redrawn before each blocking event read. There are no background jobs or asynchronous tasks. Errors from loading and saving propagate through `anyhow::Result`; terminal restoration is handled by `Drop`.

## Persistence contract

`metadata.toml` contains:

```toml
next_id = 2
```

Each hashed daily file contains its unhashed date and an array of todos:

```toml
date = "2026-01-31"

[[todos]]
id = 1
title = "Example"
priority = "medium"
completed = false
created_at = "2026-01-31T12:00:00+08:00"
details = ""
```

Dates use `YYYY-MM-DD`; timestamps use chrono's serde representation with a fixed UTC offset. Missing optional-in-practice fields are handled by the defaults declared in `model.rs`. The legacy JSON shape retains `next_id` and a date-keyed `days` map containing the same todo fields. Treat all data files as user data: never silently discard them to recover from a parse or write error.

## Dependencies

- `anyhow`: application errors and filesystem context
- `chrono`: local dates, timestamps, and calendar arithmetic; serde support enabled
- `crossterm`: keyboard events and terminal control
- `ratatui`: terminal widgets and rendering
- `serde` / `serde_json`: data serialization and legacy JSON compatibility
- `sha2`: SHA-256-based month and day path names
- `toml`: daily files and metadata serialization
- `unicode-width`: display-cell-aware text layout

Use the versions and idioms already recorded in `Cargo.toml` and `Cargo.lock`. Avoid adding a dependency for functionality that the standard library or current dependencies already provide.

The release profile enables LTO, uses one codegen unit, and strips symbols. Development and release artifacts belong under `target/`.

## Change guidelines

- Keep module boundaries intact: event/state transitions in `app.rs`, domain rules in `model.rs`, filesystem work in `storage.rs`, and rendering in `ui.rs`.
- Save after every successful user-visible data mutation, including priority changes and details edits. Do not save canceled forms.
- Keep IDs stable and sorting deterministic. Selection should continue to use IDs, not vector positions.
- Preserve terminal restoration on every return path.
- Keep user-facing key documentation consistent across the footer, modal help, `ts --help`, and `README.md`.
- Preserve backward compatibility for existing JSON and legacy-path migration unless an explicit migration design replaces it.
- Use `anyhow` context for I/O failures so the affected path is visible.
- Do not write to real user data during manual verification. Set `TS_DATA_DIR` to a temporary path, or use `TS_DATA_FILE` when specifically verifying legacy JSON mode.
- Update this file when module responsibilities, repository structure, storage behavior, or development workflows change.

## Verification

Run the standard checks from the repository root:

```sh
cargo fmt --check
cargo check --locked
cargo test --locked
cargo clippy --locked -- -D warnings
```

For interactive changes, also run the real binary against an isolated directory and exercise the affected mode end to end:

```sh
TS_DATA_DIR=/tmp/ts-agent-todos cargo run --locked
```

Verify terminal restoration after both normal quit and error paths where practical. For installation changes, test `install.sh` with a destination inside an existing temporary directory rather than the system default.
