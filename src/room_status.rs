use std::collections::HashMap;

use futures::stream::{FuturesUnordered, StreamExt};

use crate::matrix::check_room_health;
use crate::models::{Room, RoomHealth};

pub async fn check_rooms_status(rooms: &[Room]) -> HashMap<String, RoomHealth> {
    let mut checks = FuturesUnordered::new();

    for room in rooms.iter().cloned() {
        checks.push(async move {
            let health = check_room_health(&room).await;
            (room.room_id, health)
        });
    }

    let mut statuses = HashMap::new();

    while let Some((room_id, health)) = checks.next().await {
        statuses.insert(room_id, health);
    }

    statuses
}
