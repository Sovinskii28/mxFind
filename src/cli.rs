use std::path::PathBuf;

use clap::{Parser, Subcommand};

use crate::banner::BANNER_TEXT;

#[derive(Parser)]
#[command(
    name = "mxfind",
    version,
    about = "Discover public Matrix rooms through federation public directories.",
    before_help = BANNER_TEXT
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand)]
pub enum Command {
    /// Fetch public rooms from configured homeservers and store them in SQLite.
    Index {
        /// SQLite database path.
        #[arg(long)]
        db: Option<PathBuf>,

        /// TOML config path with homeservers to index.
        #[arg(long)]
        config: Option<PathBuf>,

        /// Print skipped homeservers and reasons.
        #[arg(short, long)]
        verbose: bool,

        /// Remove stale rooms only for homeservers that were successfully scanned.
        #[arg(long)]
        prune: bool,
    },

    /// Show details for one indexed room.
    Room {
        /// Room ID or canonical alias.
        identifier: String,

        /// Print room as JSON.
        #[arg(long)]
        json: bool,

        /// SQLite database path.
        #[arg(long)]
        db: Option<PathBuf>,
    },

    /// Search public Matrix rooms.
    Search {
        /// Search query.
        query: String,

        /// Maximum number of rooms to print.
        #[arg(short, long, default_value_t = 20)]
        limit: usize,

        /// Print results as JSON.
        #[arg(long)]
        json: bool,

        /// TOML config path with homeservers for live search.
        #[arg(long)]
        config: Option<PathBuf>,

        /// Search the local SQLite database.
        #[arg(long)]
        local: bool,

        /// Force live search through homeserver public room directories.
        #[arg(long)]
        live: bool,

        /// SQLite database path.
        #[arg(long)]
        db: Option<PathBuf>,
    },

    /// Check configured Matrix homeservers availability.
    Status {
        /// TOML config path with homeservers to check.
        #[arg(long)]
        config: Option<PathBuf>,

        /// Check a single homeserver instead of configured homeservers.
        #[arg(long)]
        server: Option<String>,

        /// Print server status as JSON.
        #[arg(long)]
        json: bool,
    },

    /// Open the experimental terminal UI.
    Tui {
        /// SQLite database path for local TUI search.
        #[arg(long)]
        db: Option<PathBuf>,

        /// TOML config path with homeservers to show in the server status block.
        #[arg(long)]
        config: Option<PathBuf>,

        /// Search rooms from the local SQLite database instead of live homeservers.
        #[arg(long)]
        local: bool,
    },
}
