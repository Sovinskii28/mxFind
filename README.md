# mxfind

[English](README.md) | [Русский](README.ru.md)

**Matrix room discovery from your terminal.**

`mxfind` is a small Rust CLI/TUI for searching public Matrix rooms through homeserver public room directories. Run it as an interactive terminal app, search live homeservers, or build a local SQLite index for fast repeated lookups.

![mxfind TUI screenshot](docs/screenshots/19283.png)

## Install

Download the latest binary for your platform from:

https://github.com/Sovinskii28/mxFind/releases/latest

Linux:

```sh
curl -L https://github.com/Sovinskii28/mxFind/releases/latest/download/mxfind-linux-x86_64.tar.gz -o mxfind-linux-x86_64.tar.gz
tar -xzf mxfind-linux-x86_64.tar.gz
chmod +x mxfind
./mxfind
```

macOS Apple Silicon:

```sh
curl -L https://github.com/Sovinskii28/mxFind/releases/latest/download/mxfind-macos-arm64.tar.gz -o mxfind-macos-arm64.tar.gz
tar -xzf mxfind-macos-arm64.tar.gz
chmod +x mxfind
./mxfind
```

macOS Intel:

```sh
curl -L https://github.com/Sovinskii28/mxFind/releases/latest/download/mxfind-macos-x86_64.tar.gz -o mxfind-macos-x86_64.tar.gz
tar -xzf mxfind-macos-x86_64.tar.gz
chmod +x mxfind
./mxfind
```

Windows:

Download `mxfind-windows-x86_64.zip` from the latest release, unzip it, then run:

```powershell
.\mxfind.exe
```

Move the binary into a directory from your `PATH` if you want to run `mxfind` from anywhere.

## Quick Start

Open the live TUI:

```sh
mxfind
```

Search from the command line:

```sh
mxfind search rust
mxfind search linux --limit 10
mxfind search security --json
```

Create a local index and search it:

```sh
mxfind index
mxfind search rust --local
mxfind tui --local
```

Check homeserver availability:

```sh
mxfind status
mxfind status --server matrix.org
```

Inspect an indexed room:

```sh
mxfind room '#rust:matrix.org'
mxfind room '#rust:matrix.org' --json
```

## Commands

| Command | What it does |
| --- | --- |
| `mxfind` | Opens the live terminal UI. |
| `mxfind tui` | Opens the terminal UI explicitly. |
| `mxfind search <query>` | Searches public Matrix rooms. |
| `mxfind index` | Stores public rooms from configured homeservers in SQLite. |
| `mxfind room <id-or-alias>` | Shows one room from the local index. |
| `mxfind status` | Checks homeserver public directory availability. |

## Useful Arguments

Search:

```sh
mxfind search rust --limit 20
mxfind search rust --live
mxfind search rust --local
mxfind search rust --json
mxfind search rust --config config.toml
mxfind search rust --db ./mxfind.sqlite
```

Index:

```sh
mxfind index
mxfind index --prune
mxfind index --verbose
mxfind index --config config.toml
mxfind index --db ./mxfind.sqlite
```

TUI:

```sh
mxfind tui
mxfind tui --local
mxfind tui --config config.toml
mxfind tui --db ./mxfind.sqlite
```

Status:

```sh
mxfind status
mxfind status --server matrix.org
mxfind status --json
mxfind status --config config.toml
```

Room details:

```sh
mxfind room '#rust:matrix.org'
mxfind room '!abcdef:matrix.org' --json
mxfind room '#rust:matrix.org' --db ./mxfind.sqlite
```

## How Search Works

Matrix does not have one global public room catalog. Each homeserver may expose its own public room directory, and some servers restrict or disable it.

`mxfind` supports two search paths:

- **Live search** asks configured homeservers directly.
- **Local search** queries the SQLite database created by `mxfind index`.

Local search is better for repeated searches. Live search is better when you want quick results without preparing a database.

## Configuration

Default config path:

```text
~/.config/mxfind/config.toml
```

Example:

```toml
servers = ["matrix.org", "tchncs.de", "midov.pl", "matrix.tchncs.de"]
```

Use it with:

```sh
mxfind search linux --live --config config.toml
mxfind index --config config.toml
mxfind tui --config config.toml
```

If no config exists, `mxfind` uses a built-in server list.

## Database

Default database path:

```text
~/.local/share/mxfind/mxfind.sqlite
```

The database stores room IDs, aliases, names, topics, member counts, source homeservers, and discovery timestamps.

`mxfind index` is incremental. `mxfind index --prune` removes stale rooms only for homeservers that were successfully scanned during the current run.

## Build From Source

Requirements:

- Rust stable
- Cargo

Build:

```sh
git clone https://github.com/Sovinskii28/mxFind.git
cd mxFind
cargo build --release
./target/release/mxfind
```

Install from the local checkout:

```sh
cargo install --path .
mxfind
```

Run during development:

```sh
cargo run
cargo run -- search rust
cargo run -- tui
```

Checks:

```sh
cargo fmt --check
cargo test
```

## License

MIT. See [LICENSE](LICENSE).
