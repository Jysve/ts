#!/usr/bin/env bash
set -euo pipefail

project_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
destination="${1:-/usr/bin/ts}"
destination_dir="$(dirname -- "$destination")"

cd "$project_dir"
cargo build --release --locked

if [[ ! -d "$destination_dir" ]]; then
    printf 'Destination directory does not exist: %s\n' "$destination_dir" >&2
    exit 1
fi

if [[ -w "$destination_dir" ]]; then
    install -m 0755 "$project_dir/target/release/ts" "$destination"
elif command -v sudo >/dev/null 2>&1; then
    sudo install -m 0755 "$project_dir/target/release/ts" "$destination"
else
    printf 'Write access to %s requires root privileges. Re-run with sudo or as root.\n' "$destination_dir" >&2
    exit 1
fi

printf 'Installed ts to %s\n' "$destination"
