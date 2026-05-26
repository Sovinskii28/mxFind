use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "mxfind")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    Index {
        /// SQLite database path.
        #[arg(long)]
        db: Option<PathBuf>,

        /// TOML config path with homeservers to index.
        #[arg(long)]
        config: Option<PathBuf>,
    },

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

    Tui {
        /// SQLite database path.
        #[arg(long)]
        db: Option<PathBuf>,
    },
}
