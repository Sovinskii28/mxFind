mod banner;
mod cli;
mod config;
mod db;
mod matrix;
mod models;
mod output;
mod search;
mod server_status;
mod tui;

use clap::Parser;
use futures::stream::{FuturesUnordered, StreamExt};

use crate::banner::print_banner;
use crate::cli::{Cli, Command};
use crate::config::load_config;
use crate::db::{
    default_db_path, find_room, init_db, open_db, prune_stale_rooms, search_rooms, upsert_rooms,
};
use crate::matrix::fetch_public_rooms;
use crate::models::Room;
use crate::output::{
    print_room_card, print_room_json, print_rooms_json, print_rooms_with_server_statuses,
    print_server_statuses, print_server_statuses_json,
};
use crate::search::{dedup_rooms, filter_rooms};
use crate::server_status::{check_room_server_statuses, check_servers_status};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        None => {
            print_banner();
            println!();
            println!("Run `mxfind --help` to see available commands.");
        }

        Some(Command::Index {
            db,
            config,
            verbose,
            prune,
        }) => {
            let config = load_config(config.as_deref())?;
            let db_path = match db {
                Some(path) => path,
                None => default_db_path()?,
            };

            let pool = open_db(&db_path).await?;
            init_db(&pool).await?;

            println!("Indexing public rooms...");
            println!();

            let index_result =
                fetch_rooms_from_servers(config.servers, Some(IndexProgress { verbose })).await;
            println!();

            if index_result.servers_scanned > 0 && index_result.servers_available == 0 {
                if verbose {
                    anyhow::bail!("No homeservers returned public rooms.");
                } else {
                    anyhow::bail!(
                        "No homeservers returned public rooms. Run `mxfind index --verbose` for details."
                    );
                }
            }

            let servers_scanned = index_result.servers_scanned;
            let servers_available = index_result.servers_available;
            let servers_skipped = index_result.servers_skipped;
            let available_servers = index_result.available_servers;
            let rooms = index_result.rooms;
            let rooms_fetched = rooms.len();
            let rooms = dedup_rooms(rooms);
            let rooms_saved = upsert_rooms(&pool, &rooms).await?;
            let rooms_pruned = if prune {
                prune_stale_rooms(&pool, &available_servers, &rooms).await?
            } else {
                0
            };

            println!("Servers scanned: {servers_scanned}");
            println!("Servers available: {servers_available}");
            println!("Servers skipped: {servers_skipped}");
            println!("Rooms fetched: {rooms_fetched}");
            println!("Rooms saved: {rooms_saved}");
            if prune {
                println!("Rooms pruned: {rooms_pruned}");
            }
            println!("Database path: {}", db_path.display());
            println!();
            println!("Done.");
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
                print_search_results(&matches, limit, json).await?;
            } else {
                let matches = search_live_rooms(config.as_deref(), &query).await?;
                print_search_results(&matches, limit, json).await?;
            }
        }

        Some(Command::Status {
            config,
            server,
            json,
        }) => {
            let servers = match server {
                Some(server) => vec![server],
                None => load_config(config.as_deref())?.servers,
            };
            let statuses = check_servers_status(servers).await;

            if json {
                print_server_statuses_json(&statuses)?;
            } else {
                print_server_statuses(&statuses);
            }
        }

        Some(Command::Tui { db, config }) => {
            print_banner();
            println!();

            let config = load_config(config.as_deref())?;
            let db_path = match db {
                Some(path) => path,
                None => default_db_path()?,
            };

            if !db_path.exists() {
                anyhow::bail!("Local database not found. Run `mxfind index` first.");
            }

            let pool = open_db(&db_path).await?;
            tui::run(pool, config.servers).await?;
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
    let index_result = fetch_rooms_from_servers(config.servers, None).await;
    let rooms = index_result.rooms;
    let rooms = dedup_rooms(rooms);

    Ok(filter_rooms(query, &rooms))
}

async fn print_search_results(rooms: &[Room], limit: usize, json: bool) -> anyhow::Result<()> {
    if json {
        print_rooms_json(rooms, limit)
    } else {
        let server_statuses = check_room_server_statuses(rooms).await;

        println!("Found {} matching rooms", rooms.len());
        print_rooms_with_server_statuses(rooms, limit, &server_statuses);
        Ok(())
    }
}

struct FetchRoomsResult {
    rooms: Vec<Room>,
    available_servers: Vec<String>,
    servers_scanned: usize,
    servers_available: usize,
    servers_skipped: usize,
}

#[derive(Clone, Copy)]
struct IndexProgress {
    verbose: bool,
}

async fn fetch_rooms_from_servers(
    servers: Vec<String>,
    progress: Option<IndexProgress>,
) -> FetchRoomsResult {
    let servers: Vec<String> = servers
        .into_iter()
        .map(|server| server.trim().to_string())
        .filter(|server| !server.is_empty())
        .collect();
    let mut rooms: Vec<Room> = Vec::new();
    let mut available_servers = Vec::new();
    let mut servers_available = 0;
    let mut servers_skipped = 0;
    let servers_scanned = servers.len();
    let mut requests = FuturesUnordered::new();

    for server in servers {
        requests.push(async move {
            let result = fetch_public_rooms(&server).await;
            (server, result)
        });
    }

    let mut servers_completed = 0;

    while let Some((server, result)) = requests.next().await {
        servers_completed += 1;

        match result {
            Ok(mut server_rooms) => {
                servers_available += 1;
                available_servers.push(server.clone());
                let rooms_count = server_rooms.len();
                if progress.is_some() {
                    println!(
                        "[{servers_completed}/{servers_scanned}] {server} ... ok, {rooms_count} rooms"
                    );
                }
                rooms.append(&mut server_rooms);
            }
            Err(reason) => {
                servers_skipped += 1;

                if let Some(progress) = progress {
                    if progress.verbose {
                        println!(
                            "[{servers_completed}/{servers_scanned}] {server} ... skipped ({reason})"
                        );
                    } else {
                        println!("[{servers_completed}/{servers_scanned}] {server} ... skipped");
                    }
                }
            }
        }
    }

    FetchRoomsResult {
        rooms,
        available_servers,
        servers_scanned,
        servers_available,
        servers_skipped,
    }
}
