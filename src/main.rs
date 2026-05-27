mod banner;
mod cli;
mod config;
mod db;
mod matrix;
mod models;
mod output;
mod search;
mod tui;

use clap::Parser;
use futures::future::join_all;

use crate::banner::print_banner;
use crate::cli::{Cli, Command};
use crate::config::load_config;
use crate::db::{default_db_path, find_room, init_db, open_db, search_rooms, upsert_rooms};
use crate::matrix::fetch_public_rooms;
use crate::models::Room;
use crate::output::{print_room_card, print_room_json, print_rooms, print_rooms_json};
use crate::search::{dedup_rooms, filter_rooms};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        None => {
            print_banner();
            println!();
            println!("Run `mxfind --help` to see available commands.");
        }

        Some(Command::Index { db, config }) => {
            let config = load_config(config.as_deref())?;
            let db_path = match db {
                Some(path) => path,
                None => default_db_path()?,
            };

            let pool = open_db(&db_path).await?;
            init_db(&pool).await?;

            let servers_scanned = config.servers.len();
            let (rooms, servers_failed) = fetch_rooms_from_servers(config.servers).await;
            let rooms_fetched = rooms.len();
            let rooms = dedup_rooms(rooms);
            let rooms_saved = upsert_rooms(&pool, &rooms).await?;

            println!("Servers scanned: {servers_scanned}");
            println!("Servers failed: {servers_failed}");
            println!("Rooms fetched: {rooms_fetched}");
            println!("Rooms saved: {rooms_saved}");
            println!("Database path: {}", db_path.display());
        }

        Some(Command::Room {
            identifier,
            json,
            db,
        }) => {
            let db_path = match db {
                Some(path) => path,
                None => default_db_path()?,
            };

            if !db_path.exists() {
                anyhow::bail!("Local database not found. Run `mxfind index` first.");
            }

            let pool = open_db(&db_path).await?;
            let room = find_room(&pool, &identifier).await?;

            if json {
                print_room_json(room.as_ref())?;
            } else {
                match room {
                    Some(room) => print_room_card(&room),
                    None => println!("Room not found in local index."),
                }
            }
        }

        Some(Command::Search {
            query,
            limit,
            json,
            config,
            local,
            live,
            db,
        }) => {
            if local && live {
                anyhow::bail!("--live and --local cannot be used together");
            }

            if !json {
                println!("Searching for: {query}");
            }

            let db_path = match db {
                Some(path) => path,
                None => default_db_path()?,
            };
            let use_local = local || (!live && db_path.exists());

            if use_local {
                let matches = search_local_rooms(&db_path, &query, limit).await?;
                print_search_results(&matches, limit, json)?;
            } else {
                let matches = search_live_rooms(config.as_deref(), &query).await?;
                print_search_results(&matches, limit, json)?;
            }
        }

        Some(Command::Tui { db }) => {
            print_banner();
            println!();

            let db_path = match db {
                Some(path) => path,
                None => default_db_path()?,
            };

            if !db_path.exists() {
                anyhow::bail!("Local database not found. Run `mxfind index` first.");
            }

            let pool = open_db(&db_path).await?;
            tui::run(pool).await?;
        }
    }

    Ok(())
}

async fn search_local_rooms(
    db_path: &std::path::Path,
    query: &str,
    limit: usize,
) -> anyhow::Result<Vec<Room>> {
    if !db_path.exists() {
        anyhow::bail!("Local database not found. Run `mxfind index` first.");
    }

    let pool = open_db(db_path).await?;
    search_rooms(&pool, query, limit).await
}

async fn search_live_rooms(
    config_path: Option<&std::path::Path>,
    query: &str,
) -> anyhow::Result<Vec<Room>> {
    let config = load_config(config_path)?;
    let (rooms, _) = fetch_rooms_from_servers(config.servers).await;
    let rooms = dedup_rooms(rooms);

    Ok(filter_rooms(query, &rooms))
}

fn print_search_results(rooms: &[Room], limit: usize, json: bool) -> anyhow::Result<()> {
    if json {
        print_rooms_json(rooms, limit)
    } else {
        println!("Found {} matching rooms", rooms.len());
        print_rooms(rooms, limit);
        Ok(())
    }
}

async fn fetch_rooms_from_servers(servers: Vec<String>) -> (Vec<Room>, usize) {
    let mut rooms: Vec<Room> = Vec::new();
    let mut servers_failed = 0;
    let requests = servers.into_iter().map(|server| async move {
        let result = fetch_public_rooms(&server).await;
        (server, result)
    });

    for (server, result) in join_all(requests).await {
        match result {
            Ok(mut server_rooms) => rooms.append(&mut server_rooms),
            Err(error) => {
                servers_failed += 1;
                eprintln!("warning: failed to fetch rooms from {server}: {error:#}");
            }
        }
    }

    (rooms, servers_failed)
}
