# ts

`ts` is an interactive terminal todo application written in Rust. It organizes todos by date, sorts them by priority, and stores each day's todos locally in TOML. The interface is built with [ratatui](https://github.com/ratatui/ratatui) and [crossterm](https://github.com/crossterm-rs/crossterm).

## Features

- Daily todo lists with previous/next-day navigation and a calendar picker
- High, medium, and low priorities, with priority-ordered lists
- Completion toggles, title editing, and multiline details
- Automatic persistence after every change
- Atomic per-day TOML writes and automatic migration from legacy JSON data
- A non-interactive `ts --list` command for all incomplete todos
- Unicode-aware title truncation and terminal cleanup on exit

## Requirements

- A Rust toolchain with Cargo and support for Rust edition 2024 (Rust 1.85 or newer)
- A terminal with keyboard input and ANSI terminal support

## Installation

Install the `ts` binary with Cargo:

```sh
cargo install --path . --locked
```

Make sure Cargo's binary directory, usually `~/.cargo/bin`, is on your `PATH`.

Alternatively, use the included installation script:

```sh
./install.sh
```

The script builds a locked release binary and installs it to `/usr/bin/ts`, using `sudo` when necessary. To choose another destination, pass its full path as the first argument:

```sh
./install.sh "$HOME/.local/bin/ts"
```

The destination directory must already exist.

## Build and run

```sh
cargo build --locked
cargo run --locked
```

For an optimized binary:

```sh
cargo build --release --locked
./target/release/ts
```

The application starts with today's date selected. Use `ts --help` for a compact command reference and `ts --version` to print the installed version.

To list every incomplete todo directly in the terminal, sorted by date and then priority:

```sh
ts --list
```

Each line has the form `YYYY-MM-DD [Priority] Title`. Completed todos are omitted.

## Keyboard controls

### Normal mode

| Key | Action |
| --- | --- |
| `Up` / `Down` or `k` / `j` | Select a todo |
| `Left` / `Right` | Go to the previous or next date |
| `t` | Go to today |
| `c` | Open the calendar |
| `a` or `n` | Add a todo |
| `e` | Edit the selected todo's title and priority |
| `d` | Open and edit the selected todo's details |
| `Space` or `Enter` | Toggle completion |
| `1` / `2` / `3` | Set high, medium, or low priority |
| `x` | Delete the selected todo after confirmation |
| `q` or `Esc` | Quit |

`Ctrl+C` also quits the application.

### Add and edit form

| Key | Action |
| --- | --- |
| `Tab`, `Shift+Tab`, `Up`, or `Down` | Switch between title and priority |
| `Left` / `Right` | Change priority when the priority field is focused |
| `Enter` | Save |
| `Esc` | Cancel |

### Details editor

| Key | Action |
| --- | --- |
| `Enter` | Insert a newline |
| `Tab` | Insert four spaces |
| `Up` / `Down` | Scroll |
| `PageUp` / `PageDown` | Scroll by ten lines |
| `Ctrl+S` | Save and close |
| `Esc` | Cancel without saving |

### Calendar

| Key | Action |
| --- | --- |
| Arrow keys | Move by one day or one week |
| `PageUp` / `PageDown` | Move by one month |
| `t` | Highlight today |
| `Enter` | Select the highlighted date |
| `Esc` | Cancel |

Dates without todos are gray, dates with todos are white, and today has an orange background. The highlighted date retains a contrasting selection background.

## Data storage

Todos are saved after every change. By default, the data directory is `$XDG_CONFIG_HOME/ts` when `XDG_CONFIG_HOME` is set, or `~/.config/ts` otherwise. Set `TS_DATA_DIR` to use another directory.

Each day is stored in a separate TOML file. The subdirectory name is the first 16 hexadecimal characters of the SHA-256 hash of `YYYY-MM`, and the file name is the same 16-character hash of `YYYY-MM-DD` plus `.toml`. For example:

```text
~/.config/ts/
├── metadata.toml
└── 05e01bd602cd4d89/
    ├── a433c8ba83101c75.toml
    └── 059da136dbf487af.toml
```

`metadata.toml` stores the globally monotonically increasing `next_id`. Each daily file contains its ISO date and todos; every todo records its ID, title, priority, completion state, creation timestamp, and details. Daily files and metadata use atomic replacement, and changing one day does not rewrite other days.

On startup, if `metadata.toml` does not exist, existing data is imported from the first available legacy file: `todos.json` in the data directory, `$XDG_DATA_HOME/ts/todos.json`, or `~/.local/share/ts/todos.json`. The legacy JSON file is retained after a successful import.

For backward compatibility, setting `TS_DATA_FILE` selects the original single-file JSON mode and disables daily TOML storage and automatic migration. For isolated daily storage while developing or testing, use `TS_DATA_DIR` instead:

```sh
TS_DATA_DIR=/tmp/ts-todos cargo run --locked
```

## Development

Useful checks before submitting changes:

```sh
cargo fmt --check
cargo check --locked
cargo test --locked
cargo clippy --locked -- -D warnings
```

See [AGENTS.md](AGENTS.md) for the module layout, application architecture, persistence format, and maintenance guidelines.

## License

Licensed under the Apache License, Version 2.0. See [LICENSE](LICENSE) for the full license text.
