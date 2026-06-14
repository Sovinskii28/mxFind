use std::collections::HashMap;

use futures::stream::{FuturesUnordered, StreamExt};

use crate::matrix::check_room_health;
use crate::models::{Room, RoomHealth, RoomStatus};

pub async fn check_rooms_status(rooms: &[Room]) -> HashMap<String, RoomHealth> {
    let mut checks = FuturesUnordered::new();
    let mut statuses = HashMap::new();

    for room in rooms.iter().cloned() {
        if room.canonical_alias.is_none() {
            statuses.insert(
                room.room_id.clone(),
                RoomHealth {
                    room_id: room.room_id,
                    alias: None,
                    status: RoomStatus::NoAlias,
                    resolved_room_id: None,
                    latency_ms: None,
                    reason: Some("room has no canonical alias".to_string()),
                },
            );
            continue;
        }

        checks.push(async move {
            let health = check_room_health(&room).await;
            (room.room_id, health)
        });
    }

    while let Some((room_id, health)) = checks.next().await {
        statuses.insert(room_id, health);
    }

    statuses
}
