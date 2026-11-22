# ts

`ts` is an interactive terminal todo application written in Rust. It organizes todos by date, sorts them by priority, and stores everything locally in a JSON file. The interface is built with [ratatui](https://github.com/ratatui/ratatui) and [crossterm](https://github.com/crossterm-rs/crossterm).

## Features

- Daily todo lists with previous/next-day navigation and a calendar picker
- High, medium, and low priorities, with priority-ordered lists
- Completion toggles, title editing, and multiline details
- Automatic persistence after every change
- Atomic JSON writes and migration from the legacy data directory
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

## Data storage

Todos are saved after every change. The data file is selected in this order:

1. `TS_DATA_FILE`, when set to a nonempty path
2. `$XDG_CONFIG_HOME/ts/todos.json`, when `XDG_CONFIG_HOME` is set
3. `~/.config/ts/todos.json`

When using the default location, existing data is automatically migrated from `$XDG_DATA_HOME/ts/todos.json` or `~/.local/share/ts/todos.json` if the new file does not exist. Setting `TS_DATA_FILE` disables legacy migration.

The JSON file contains a monotonically increasing `next_id` and a date-keyed map of todos. Each todo records its ID, title, priority, completion state, creation timestamp, and details. There is no network service or cloud synchronization.

To use an isolated data file while developing or testing:

```sh
TS_DATA_FILE=/tmp/ts-todos.json cargo run --locked
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
