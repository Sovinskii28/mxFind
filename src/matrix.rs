use std::time::Duration;

use anyhow::Context;
use serde::Deserialize;

use crate::models::Room;

#[derive(Deserialize)]
struct PublicRoomsResponse {
    chunk: Vec<PublicRoom>,
}

#[derive(Deserialize)]
struct PublicRoom {
    room_id: String,
    name: Option<String>,
    topic: Option<String>,
    canonical_alias: Option<String>,
    num_joined_members: Option<u64>,
}

pub async fn fetch_public_rooms(server: &str) -> anyhow::Result<Vec<Room>> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .context("failed to build HTTP client")?;

    let url = format!("https://{server}/_matrix/client/v3/publicRooms");
    let response = client
        .get(&url)
        .send()
        .await
        .with_context(|| format!("failed to request public rooms from {server}"))?
        .error_for_status()
        .with_context(|| format!("public rooms request failed for {server}"))?
        .json::<PublicRoomsResponse>()
        .await
        .with_context(|| format!("failed to parse public rooms response from {server}"))?;

    let rooms = response
        .chunk
        .into_iter()
        .map(|room| Room {
            room_id: room.room_id,
            name: room.name,
            topic: room.topic,
            canonical_alias: room.canonical_alias,
            num_joined_members: room.num_joined_members,
            server: server.to_string(),
        })
        .collect();

    Ok(rooms)
}
