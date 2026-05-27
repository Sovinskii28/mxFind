use anyhow::Context;

use crate::models::Room;

const SEARCH_TOPIC_PREVIEW_LEN: usize = 90;

pub fn print_rooms(rooms: &[Room], limit: usize) {
    let rooms = sorted_limited_rooms(rooms, limit);

    if rooms.is_empty() {
        println!("Ничего не найдено.");
        return;
    }

    for room in &rooms {
        let id = room.canonical_alias.as_deref().unwrap_or(&room.room_id);

        println!("{id}");

        if let Some(name) = &room.name {
            println!("  name: {name}");
        }

        if let Some(topic) = &room.topic {
            println!(
                "  topic: {}",
                format_topic_preview(topic, SEARCH_TOPIC_PREVIEW_LEN)
            );
        }

        if let Some(members) = room.num_joined_members {
            println!("  members: {members}");
        }

        println!("  server: {}", room.server);
        println!();
    }
}

pub fn print_rooms_json(rooms: &[Room], limit: usize) -> anyhow::Result<()> {
    let rooms = sorted_limited_rooms(rooms, limit);
    let json = serde_json::to_string_pretty(&rooms).context("failed to serialize rooms as JSON")?;

    println!("{json}");
    Ok(())
}

pub fn print_room_card(room: &Room) {
    let id = room.canonical_alias.as_deref().unwrap_or(&room.room_id);

    println!("{id}");
    println!("  room_id: {}", room.room_id);

    if let Some(name) = &room.name {
        println!("  name: {name}");
    }

    if let Some(topic) = &room.topic {
        println!("  topic: {topic}");
    }

    if let Some(members) = room.num_joined_members {
        println!("  members: {members}");
    }

    println!("  server: {}", room.server);

    println!("  matrix.to: {}", room.matrix_to_url());
}

pub fn print_room_json(room: Option<&Room>) -> anyhow::Result<()> {
    let json = serde_json::to_string_pretty(&room).context("failed to serialize room as JSON")?;

    println!("{json}");
    Ok(())
}

fn sorted_limited_rooms(rooms: &[Room], limit: usize) -> Vec<Room> {
    let mut rooms = rooms.to_vec();
    rooms.sort_by(|left, right| {
        right
            .num_joined_members
            .unwrap_or(0)
            .cmp(&left.num_joined_members.unwrap_or(0))
    });

    rooms.into_iter().take(limit).collect()
}

pub fn format_topic_preview(topic: &str, max_len: usize) -> String {
    let topic = topic.split_whitespace().collect::<Vec<_>>().join(" ");

    if topic.chars().count() <= max_len {
        return topic;
    }

    let preview = topic.chars().take(max_len).collect::<String>();
    format!("{preview}...")
}

#[cfg(test)]
mod tests {
    use super::format_topic_preview;

    #[test]
    fn short_topic_is_unchanged() {
        let topic = "A small Matrix room for Rust chat.";

        assert_eq!(format_topic_preview(topic, 120), topic);
    }

    #[test]
    fn long_topic_is_truncated() {
        let topic = "a".repeat(121);

        assert_eq!(
            format_topic_preview(&topic, 120),
            format!("{}...", "a".repeat(120))
        );
    }

    #[test]
    fn newlines_tabs_and_multiple_spaces_become_single_spaces() {
        let topic = "Rust\n\nMatrix\troom   with    compact\noutput";

        assert_eq!(
            format_topic_preview(topic, 120),
            "Rust Matrix room with compact output"
        );
    }

    #[test]
    fn unicode_strings_are_not_broken() {
        let topic = "Привет мир 😀 Rust Matrix";

        assert_eq!(format_topic_preview(topic, 12), "Привет мир 😀...");
    }
}
