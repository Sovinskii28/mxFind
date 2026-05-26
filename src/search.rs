use std::collections::HashMap;

use crate::models::Room;

pub fn room_matches(query: &str, room: &Room) -> bool {
    let query = query.to_lowercase();

    field_matches(&query, &room.room_id)
        || option_field_matches(&query, room.name.as_deref())
        || option_field_matches(&query, room.topic.as_deref())
        || option_field_matches(&query, room.canonical_alias.as_deref())
}

pub fn filter_rooms(query: &str, rooms: &[Room]) -> Vec<Room> {
    rooms
        .iter()
        .filter(|room| room_matches(query, room))
        .cloned()
        .collect()
}

pub fn dedup_rooms(rooms: Vec<Room>) -> Vec<Room> {
    let mut unique_rooms: HashMap<String, Room> = HashMap::new();

    for room in rooms {
        let key = room
            .canonical_alias
            .clone()
            .unwrap_or_else(|| room.room_id.clone());

        match unique_rooms.get_mut(&key) {
            Some(existing_room) => {
                if room.num_joined_members.unwrap_or(0)
                    > existing_room.num_joined_members.unwrap_or(0)
                {
                    *existing_room = room;
                }
            }
            None => {
                unique_rooms.insert(key, room);
            }
        }
    }

    unique_rooms.into_values().collect()
}

fn field_matches(query: &str, value: &str) -> bool {
    value.to_lowercase().contains(query)
}

fn option_field_matches(query: &str, value: Option<&str>) -> bool {
    value.is_some_and(|value| field_matches(query, value))
}
