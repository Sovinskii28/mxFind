# mxfind

English | [Русский](README.ru.md)

`mxfind` — Rust CLI/TUI tool for discovering public Matrix rooms through homeserver public room directories.

## Table of Contents

- [Features](#features)
- [Screenshots](#screenshots)
- [Installation](#installation)
- [Quick Start](#quick-start)
- [Commands](#commands)
- [Example Usage](#example-usage)
- [Configuration](#configuration)
- [Architecture](#architecture)
- [Limitations](#limitations)
- [Roadmap](#roadmap)
- [Development](#development)
- [License](#license)

## Features

- Matrix public room search through the Client-Server `publicRooms` API
- Multi-server search across configured homeservers
- SQLite local index for fast offline/local queries
- Room details lookup by room ID or canonical alias
- JSON output for scripting and automation
- Experimental terminal UI
- Async networking with Tokio and Reqwest

## Installation

### Build from Source

```sh
git clone <repo-url>
cd mxfind
cargo build --release
```

The binary will be available at:

```sh
./target/release/mxfind
```

### Cargo Install

TODO: publish `mxfind` to crates.io.

For local development, install from the repository:

```sh
cargo install --path .
```

### Release Binaries

TODO: provide prebuilt release binaries.

## Quick Start

The normal workflow is:

1. Build or refresh the local SQLite index.
2. Search the local index.
3. Inspect a room.
4. Optionally open the experimental TUI.

```sh
mxfind index
mxfind search rust
mxfind room '#rust:matrix.org'
mxfind tui
```

Run `mxfind index` first. It queries the configured homeservers and stores public room metadata in a local SQLite database.

The TUI uses the local SQLite database too, so it also expects `mxfind index` to have been run.

Default database path:

```text
~/.local/share/mxfind/mxfind.sqlite
```

## Commands

| Command | Purpose | Common Options |
| --- | --- | --- |
| `mxfind search <query>` | Search rooms. Uses the local DB if it exists, otherwise falls back to live search. | `--local`, `--live`, `--json`, `--limit <n>`, `--db <path>`, `--config <path>` |
| `mxfind index` | Fetch public rooms from configured homeservers and store them in SQLite. | `--db <path>`, `--config <path>` |
| `mxfind room <identifier>` | Show details for one indexed room by room ID or canonical alias. | `--json`, `--db <path>` |
| `mxfind tui` | Open the experimental terminal UI backed by the local SQLite index. | `--db <path>` |

## Example Usage

Build the local index:

```sh
mxfind index
```

Search using the default behavior:

```sh
mxfind search rust
```

Force live search against homeserver public directories:

```sh
mxfind search rust --live
```

Force local SQLite search:

```sh
mxfind search linux --local --limit 5
```

Print search results as JSON:

```sh
mxfind search rust --json
```

Use a custom config:

```sh
mxfind search rust --config config.toml
```

Inspect one room from the local index:

```sh
mxfind room '#rust:matrix.org'
```

Print one room as JSON:

```sh
mxfind room '#rust:matrix.org' --json
```

Open the experimental TUI:

```sh
mxfind tui
```

## Configuration

Default config path:

```text
~/.config/mxfind/config.toml
```

Example:

```toml
servers = ["matrix.org", "envs.net"]
```

If no config file exists, `mxfind` uses its built-in default homeserver list.

You can also pass a config file explicitly:

```sh
mxfind index --config config.toml
mxfind search rust --live --config config.toml
```

## Architecture

`mxfind` has two search paths:

- **Live search** queries selected homeservers directly via the Matrix Client-Server `/_matrix/client/v3/publicRooms` endpoint.
- **Local search** queries a SQLite index created by `mxfind index`.

The index command fetches public room directories from configured homeservers, deduplicates rooms, and upserts them into SQLite.

The TUI is intentionally local-only. It searches the SQLite index and does not perform network requests.

## Limitations

Matrix does not provide one global public room search endpoint for the whole federation.

`mxfind` searches public room directories exposed by selected homeservers. That means results depend on which servers are configured and what those servers choose to expose.

Some homeservers may:

- not respond
- time out
- disable or restrict their public directory
- require authentication for directory access

Private rooms are not indexed. Rooms not returned by a queried public directory are not visible to `mxfind`.

## Roadmap

- Better TUI
- Federation analytics
- Room statistics
- Caching improvements
- Incremental indexing
- Full-text search
- Tags/categories

## Development

```sh
cargo fmt
cargo clippy
cargo test
```

Useful local checks:

```sh
cargo check
cargo run -- index
cargo run -- search rust
cargo run -- tui
```

## License

MIT placeholder.
