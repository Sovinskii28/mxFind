# mxfind

English | [Русский](README.ru.md)

```text
 __  __ __  __ _____ ___ _   _ ____
|  \/  |\ \/ /|  ___|_ _| \ | |  _ \
| |\/| | \  / | |_   | ||  \| | | | |
| |  | | /  \ |  _|  | || |\  | |_| |
|_|  |_|/_/\_\|_|   |___|_| \_|____/
```

**Matrix Federation Explorer**

`mxfind` is a Rust CLI/TUI tool for discovering public Matrix rooms through homeserver public room directories.

It can build a local SQLite index, search it quickly, inspect rooms, print compact terminal output, and emit full JSON for scripts.

## Table of Contents

- [Features](#features)
- [How It Works](#how-it-works)
- [Installation](#installation)
- [Quick Start](#quick-start)
- [Commands](#commands)
- [Output](#output)
- [Configuration](#configuration)
- [Database](#database)
- [TUI](#tui)
- [Matrix Federation Limits](#matrix-federation-limits)
- [Troubleshooting](#troubleshooting)
- [Development](#development)
- [Roadmap](#roadmap)
- [License](#license)

## Features

- Search public Matrix rooms through `/_matrix/client/v3/publicRooms`.
- Query multiple configured homeservers.
- Store room metadata in a local SQLite index.
- Use local search by default when the database exists.
- Force live search when needed.
- Print compact human-readable search results.
- Print full JSON without truncating data.
- Inspect a single indexed room by room ID or canonical alias.
- Check homeserver availability with `mxfind status`.
- Show live server status next to rooms in human-readable output.
- Explore rooms through an experimental TUI with live search by default.
- Async networking with Tokio and Reqwest.

## How It Works

Matrix does not provide a single global public room search endpoint for the entire federation. Homeservers may expose their own public room directories.

`mxfind` supports two search paths:

1. **Local search** queries the SQLite database created by `mxfind index`.
2. **Live search** requests configured homeservers and filters the returned rooms in memory.

The recommended workflow is:

```sh
mxfind index
mxfind search rust
```

This keeps repeated searches fast and avoids a network request on every query.

## Installation

### Requirements

- Rust toolchain
- Cargo
- Network access for indexing and live search

Check your toolchain:

```sh
rustc --version
cargo --version
```

### Build from Source

```sh
git clone <repo-url>
cd mxfind
cargo build --release
```

The release binary will be available at:

```sh
./target/release/mxfind
```

### Install Locally

```sh
cargo install --path .
```

### Run Without Installing

```sh
cargo run -- search rust
cargo run -- index
```

## Quick Start

Build or refresh the local index:

```sh
mxfind index
```

Safely remove stale rooms from successfully scanned homeservers:

```sh
mxfind index --prune
```

Search rooms:

```sh
mxfind search rust
```

Inspect a room:

```sh
mxfind room '#rust:matrix.org'
```

Open the TUI with live search:

```sh
mxfind tui
```

Open the TUI with local SQLite search:

```sh
mxfind tui --local
mxfind --local tui
```

Check homeserver status:

```sh
mxfind status
mxfind status --server matrix.org
```

## Commands

### `mxfind`

Prints the banner, version, and a short hint.

```sh
mxfind
```

### `mxfind index`

Fetch public rooms from configured homeservers and store them in SQLite.

```sh
mxfind index
```

Options:

| Option | Purpose |
| --- | --- |
| `--db <path>` | Use a custom SQLite database path. |
| `--config <path>` | Use a custom TOML config path. |
| `-v, --verbose` | Print skipped homeservers and reasons. |
| `--prune` | Remove stale rooms only for homeservers that were successfully scanned. |

By default, indexing is incremental: existing rooms are updated or inserted, and old rooms remain in the database.

Use `--prune` when you want safer cleanup. It deletes stale rooms only for homeservers that successfully responded during the current indexing run. Rooms from skipped, timed-out, offline, or restricted homeservers are preserved.

### `mxfind search <query>`

Search by `room_id`, `canonical_alias`, `name`, and `topic`.

```sh
mxfind search rust
```

Options:

| Option | Purpose |
| --- | --- |
| `-l, --limit <n>` | Maximum number of results. Default: `20`. |
| `--json` | Print full JSON without truncating topic. |
| `--local` | Force local SQLite search. |
| `--live` | Force live search through homeserver public directories. |
| `--db <path>` | Use a custom SQLite database path. |
| `--config <path>` | Use a custom config for live search. |

Examples:

```sh
mxfind search linux --limit 5
mxfind search matrix --local
mxfind search rust --live --config config.toml
mxfind search security --json
```

`--local` and `--live` cannot be used together.

### `mxfind room <identifier>`

Show one indexed room by room ID or canonical alias.

```sh
mxfind room '#rust:matrix.org'
mxfind room '!abcdef:matrix.org' --json
```

Options:

| Option | Purpose |
| --- | --- |
| `--json` | Print the room as JSON. |
| `--db <path>` | Use a custom SQLite database path. |

Room details keep the full topic.

### `mxfind status`

Check Matrix homeserver availability.

```sh
mxfind status
mxfind status --server matrix.org
mxfind status --json
```

Options:

| Option | Purpose |
| --- | --- |
| `--config <path>` | Use a custom TOML config path. |
| `--server <name>` | Check a single homeserver instead of configured homeservers. |
| `--json` | Print server statuses as JSON. |

Status meanings:

| Status | Meaning |
| --- | --- |
| `online` | `publicRooms?limit=1` is reachable. |
| `restricted` | The server is reachable but public rooms are unauthorized or forbidden. |
| `offline` | The request timed out or failed to connect. |
| `unknown` | The server returned an unexpected response. |

### `mxfind tui`

Open the experimental terminal UI. By default, TUI searches live homeserver public directories.

```sh
mxfind tui
```

Options:

| Option | Purpose |
| --- | --- |
| `--db <path>` | Use a custom SQLite database path for local TUI search. |
| `--config <path>` | Use a custom TOML config path for the server status block. |
| `--local` | Search rooms from the local SQLite database instead of live homeservers. |

Run `mxfind index` before opening the TUI only when using `mxfind tui --local`.

## Output

Human-readable search output is compact:

```text
Searching for: rust
Found 2 matching rooms
[1] #rust:matrix.org
    Name:    Rust
    Members: 12000
    Server:  matrix.org
    Status:  online
    Topic:   Rust programming language community
    Link:    https://matrix.to/#/#rust:matrix.org
```

For search output, `topic` is normalized to one line and truncated to a short preview. `name`, `room_id`, and `canonical_alias` are not truncated.

Human-readable search output also performs live status checks for the homeservers in the result set and prints status per room. JSON search output keeps the original room schema for scripting compatibility and does not embed live status.

JSON output keeps full data:

```sh
mxfind search rust --json
```

Example with `jq`:

```sh
mxfind search rust --json | jq '.[].canonical_alias'
```

## Configuration

Default config path:

```text
~/.config/mxfind/config.toml
```

Example:

```toml
servers = ["matrix.org", "envs.net", "tchncs.de"]
```

Pass a config explicitly:

```sh
mxfind index --config config.toml
mxfind search rust --live --config config.toml
```

If no config exists, `mxfind` uses its built-in default server list.

## Database

Default database path:

```text
~/.local/share/mxfind/mxfind.sqlite
```

Override it:

```sh
mxfind index --db ./mxfind.sqlite
mxfind index --prune --db ./mxfind.sqlite
mxfind search rust --db ./mxfind.sqlite
mxfind room '#rust:matrix.org' --db ./mxfind.sqlite
mxfind tui --db ./mxfind.sqlite
```

Stored room metadata includes:

- room ID;
- canonical alias;
- name;
- topic;
- joined member count;
- source homeserver;
- discovery timestamp;
- last seen timestamp.

Data is not truncated before being saved to SQLite.

Indexing behavior:

- `mxfind index` is incremental and does not delete existing rooms.
- `mxfind index --prune` removes stale rooms only for homeservers that were successfully scanned in the current run.
- Prune does not delete rooms from homeservers that timed out, failed, or were skipped.

## TUI

The TUI searches live homeserver public directories by default. Use `mxfind tui --local` or `mxfind --local tui` to search the local SQLite database instead.

In both modes, the TUI may perform live network requests for homeserver and room alias status checks.

Keys:

| Key | Action |
| --- | --- |
| Text input | Edit the search query. |
| `Enter` | Run search. |
| `Up` / `Down` | Move through results or scroll details. |
| `Left` / `Right` | Switch focused panel. |
| `PageUp` / `PageDown` | Scroll details faster. |
| `r` | Refresh server statuses when the query is empty. |
| `Esc` | Quit. |
| `q` | Quit when the query is empty. |

The `Servers` block shows configured homeservers as `online`, `offline`, `restricted`, or `unknown`. Search results and room details also show the live status for each room's source server.

## Matrix Federation Limits

`mxfind` is an explorer for reachable public directories, not a complete map of Matrix.

- There is no single global federation-wide public room endpoint.
- Results depend on configured homeservers.
- Some homeservers disable or restrict their public directory.
- Some homeservers time out or return errors.
- Private rooms are not indexed.

## Troubleshooting

### `Local database not found. Run mxfind index first.`

Create the database:

```sh
mxfind index
```

Or pass an existing database:

```sh
mxfind search rust --local --db ./mxfind.sqlite
```

### Some servers fail during indexing

This is expected in Matrix federation. `mxfind index` keeps going and reports the number of failed servers.

### Live search returns too few results

Live search depends on the configured server list. Add more homeservers to your config:

```toml
servers = ["matrix.org", "envs.net", "tchncs.de", "kde.org", "gnome.org"]
```

## Development

Checks:

```sh
cargo fmt --check
cargo clippy
cargo test
```

Manual checks:

```sh
cargo run
cargo run -- --help
cargo run -- index
cargo run -- index --prune
cargo run -- search rust --limit 5
cargo run -- search rust --json
cargo run -- room '#rust:matrix.org'
cargo run -- status
cargo run -- tui
```

Main modules:

| File | Purpose |
| --- | --- |
| `src/main.rs` | Entrypoint and command routing. |
| `src/cli.rs` | Clap command and option definitions. |
| `src/banner.rs` | CLI banner and branding. |
| `src/config.rs` | TOML config loading and default servers. |
| `src/matrix.rs` | Matrix Client-Server API requests. |
| `src/db.rs` | SQLite schema, upsert, safe pruning, local search, and room lookup. |
| `src/search.rs` | Room filtering and deduplication. |
| `src/server_status.rs` | Server-status use cases and room-to-server status orchestration. |
| `src/output.rs` | Human output, JSON output, and topic preview. |
| `src/tui.rs` | Experimental terminal UI. |
| `src/models.rs` | Shared data models. |

## Roadmap

- SQLite FTS5 full-text search.
- Filters such as `--server`, `--min-members`, and `--has-alias`.
- `stats` command.
- CSV export.
- Bookmarks and user tags.
- Better TUI with search-as-you-type.

## License

MIT. See [LICENSE](LICENSE).
