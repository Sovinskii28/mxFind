use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{query, sqlite::SqliteRow, Row, SqlitePool};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

use crate::models::Room;

pub fn default_db_path() -> anyhow::Result<PathBuf> {
    let home_dir = dirs::home_dir().ok_or_else(|| anyhow!("failed to find home directory"))?;
    let db_dir = home_dir.join(".local").join("share").join("mxfind");

    fs::create_dir_all(&db_dir)
        .with_context(|| format!("failed to create database directory {}", db_dir.display()))?;

    Ok(db_dir.join("mxfind.sqlite"))
}

pub async fn open_db(path: &Path) -> anyhow::Result<SqlitePool> {
    let options = SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(true);

    SqlitePoolOptions::new()
        .connect_with(options)
        .await
        .with_context(|| format!("failed to open database {}", path.display()))
}

pub async fn init_db(pool: &SqlitePool) -> anyhow::Result<()> {
    query(
        r#"
        CREATE TABLE IF NOT EXISTS rooms (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            room_id TEXT NOT NULL,
            canonical_alias TEXT,
            name TEXT,
            topic TEXT,
            num_joined_members INTEGER,
            server TEXT NOT NULL,
            discovered_at TEXT NOT NULL,
            last_seen_at TEXT NOT NULL
        )
        "#,
    )
    .execute(pool)
    .await
    .context("failed to create rooms table")?;

    query(
        r#"
        CREATE UNIQUE INDEX IF NOT EXISTS rooms_room_id_server_idx
        ON rooms (room_id, server)
        "#,
    )
    .execute(pool)
    .await
    .context("failed to create rooms unique index")?;

    Ok(())
}

pub async fn upsert_rooms(pool: &SqlitePool, rooms: &[Room]) -> anyhow::Result<usize> {
    let now = OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .context("failed to format current UTC time")?;

    for room in rooms {
        let num_joined_members = room
            .num_joined_members
            .map(i64::try_from)
            .transpose()
            .context("room member count does not fit into SQLite integer")?;

        query(
            r#"
            INSERT INTO rooms (
                room_id,
                canonical_alias,
                name,
                topic,
                num_joined_members,
                server,
                discovered_at,
                last_seen_at
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(room_id, server) DO UPDATE SET
                canonical_alias = excluded.canonical_alias,
                name = excluded.name,
                topic = excluded.topic,
                num_joined_members = excluded.num_joined_members,
                last_seen_at = excluded.last_seen_at
            "#,
        )
        .bind(&room.room_id)
        .bind(&room.canonical_alias)
        .bind(&room.name)
        .bind(&room.topic)
        .bind(num_joined_members)
        .bind(&room.server)
        .bind(&now)
        .bind(&now)
        .execute(pool)
        .await
        .with_context(|| {
            format!(
                "failed to upsert room {} from server {}",
                room.room_id, room.server
            )
        })?;
    }

    Ok(rooms.len())
}

pub async fn search_rooms(
    pool: &SqlitePool,
    search_query: &str,
    limit: usize,
) -> anyhow::Result<Vec<Room>> {
    if limit == 0 {
        return Ok(Vec::new());
    }

    let pattern = format!("%{}%", search_query.to_lowercase());
    let limit = i64::try_from(limit).context("search limit does not fit into SQLite integer")?;
    let rows = query(
        r#"
        SELECT
            room_id,
            canonical_alias,
            name,
            topic,
            num_joined_members,
            server
        FROM rooms
        WHERE
            lower(room_id) LIKE ?
            OR lower(coalesce(canonical_alias, '')) LIKE ?
            OR lower(coalesce(name, '')) LIKE ?
            OR lower(coalesce(topic, '')) LIKE ?
        ORDER BY coalesce(num_joined_members, 0) DESC
        LIMIT ?
        "#,
    )
    .bind(&pattern)
    .bind(&pattern)
    .bind(&pattern)
    .bind(&pattern)
    .bind(limit)
    .fetch_all(pool)
    .await
    .context("failed to search rooms in local database")?;

    rows.into_iter().map(room_from_row).collect()
}

#[allow(dead_code)]
pub async fn find_room(pool: &SqlitePool, identifier: &str) -> anyhow::Result<Option<Room>> {
    let row = query(
        r#"
        SELECT
            room_id,
            canonical_alias,
            name,
            topic,
            num_joined_members,
            server
        FROM rooms
        WHERE canonical_alias = ? OR room_id = ?
        ORDER BY coalesce(num_joined_members, 0) DESC
        LIMIT 1
        "#,
    )
    .bind(identifier)
    .bind(identifier)
    .fetch_optional(pool)
    .await
    .with_context(|| format!("failed to find room {identifier} in local database"))?;

    row.map(room_from_row).transpose()
}

fn room_from_row(row: SqliteRow) -> anyhow::Result<Room> {
    let num_joined_members = row
        .try_get::<Option<i64>, _>("num_joined_members")?
        .map(u64::try_from)
        .transpose()
        .context("stored room member count is negative")?;

    Ok(Room {
        room_id: row.try_get("room_id")?,
        canonical_alias: row.try_get("canonical_alias")?,
        name: row.try_get("name")?,
        topic: row.try_get("topic")?,
        num_joined_members,
        server: row.try_get("server")?,
    })
}
