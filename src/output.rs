use anyhow::Context;

use crate::models::Room;

const SEARCH_TOPIC_PREVIEW_LEN: usize = 140;

pub fn print_rooms(rooms: &[Room], limit: usize) {
    let rooms = sorted_limited_rooms(rooms, limit);

    if rooms.is_empty() {
        println!("Ничего не найдено.");
        return;
    }

    for (index, room) in rooms.iter().enumerate() {
        let id = room.canonical_alias.as_deref().unwrap_or(&room.room_id);
        let name = room.name.as_deref().unwrap_or("No name");
        let members = room
            .num_joined_members
            .map(|members| members.to_string())
            .unwrap_or_else(|| "?".to_string());
        let topic = format_topic_preview(room.topic.as_deref(), SEARCH_TOPIC_PREVIEW_LEN)
            .unwrap_or_else(|| "No topic".to_string());

        println!("[{}] {id}", index + 1);
        println!("    Name:    {name}");
        println!("    Members: {members}");
        println!("    Server:  {}", room.server);
        println!("    Topic:   {topic}");
        println!("    Link:    {}", room.matrix_to_url());
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

pub fn normalize_whitespace(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub fn truncate_chars(text: &str, max_len: usize) -> String {
    if text.chars().count() <= max_len {
        return text.to_string();
    }

    let preview = text.chars().take(max_len).collect::<String>();
    format!("{preview}...")
}

pub fn format_topic_preview(topic: Option<&str>, max_len: usize) -> Option<String> {
    topic.map(|topic| truncate_chars(&normalize_whitespace(topic), max_len))
}

#[cfg(test)]
mod tests {
    use super::{format_topic_preview, normalize_whitespace, truncate_chars};

    #[test]
    fn topic_with_newlines_becomes_one_line() {
        let topic = "Rust\n\nMatrix\troom   with    compact\noutput";

        assert_eq!(
            normalize_whitespace(topic),
            "Rust Matrix room with compact output"
        );
    }

    #[test]
    fn long_topic_is_truncated() {
        let topic = "a".repeat(121);

        assert_eq!(
            truncate_chars(&topic, 120),
            format!("{}...", "a".repeat(120))
        );
    }

    #[test]
    fn short_topic_is_unchanged() {
        let topic = "A small Matrix room for Rust chat.";

        assert_eq!(
            format_topic_preview(Some(topic), 120).as_deref(),
            Some(topic)
        );
    }

    #[test]
    fn unicode_is_not_cut_in_the_middle_of_a_byte() {
        let topic = "Привет мир 😀 Rust Matrix";

        assert_eq!(truncate_chars(topic, 12), "Привет мир 😀...");
    }

    #[test]
    fn none_topic_is_handled() {
        assert_eq!(format_topic_preview(None, 120), None);
    }
}
