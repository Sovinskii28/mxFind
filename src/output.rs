use std::collections::HashMap;

use anyhow::Context;

use crate::models::{Room, RoomHealth, RoomStatus, ServerHealth, ServerStatus};

const SEARCH_TOPIC_PREVIEW_LEN: usize = 140;

pub fn print_rooms_with_room_statuses(
    rooms: &[Room],
    limit: usize,
    room_statuses: &HashMap<String, RoomHealth>,
) {
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
        let room_status = room_statuses
            .get(&room.room_id)
            .map(|health| room_status_label(health.status))
            .unwrap_or("not checked");

        println!("[{}] {id}", index + 1);
        println!("    Name:    {name}");
        println!("    Members: {members}");
        println!("    Server:  {}", room.server);
        println!("    Status:  {room_status}");
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

pub fn print_server_statuses(statuses: &[ServerHealth]) {
    println!("Server status:");
    println!();

    if statuses.is_empty() {
        println!("No servers configured.");
        return;
    }

    for health in statuses {
        println!("{}", format_server_status_line(health));
    }
}

pub fn print_server_statuses_json(statuses: &[ServerHealth]) -> anyhow::Result<()> {
    let json = serde_json::to_string_pretty(statuses)
        .context("failed to serialize server statuses as JSON")?;

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

fn server_status_label(status: ServerStatus) -> &'static str {
    match status {
        ServerStatus::Online => "online",
        ServerStatus::Offline => "offline",
        ServerStatus::Restricted => "restricted",
        ServerStatus::Unknown => "unknown",
    }
}

fn room_status_label(status: RoomStatus) -> &'static str {
    match status {
        RoomStatus::Resolvable => "resolvable",
        RoomStatus::NotFound => "not found",
        RoomStatus::NoAlias => "no alias",
        RoomStatus::Unknown => "unknown",
    }
}

fn format_server_status_line(health: &ServerHealth) -> String {
    let latency = health
        .latency_ms
        .map(|latency| format!("{latency}ms"))
        .unwrap_or_else(|| "-".to_string());
    let public_rooms = if health.public_rooms_available {
        "yes"
    } else {
        "no"
    };
    let reason = health
        .reason
        .as_deref()
        .map(|reason| format!(" ({reason})"))
        .unwrap_or_default();

    format!(
        "{:<22} {:<10} {:>8}  publicRooms: {public_rooms}{reason}",
        health.server,
        server_status_label(health.status),
        latency
    )
}

#[cfg(test)]
mod tests {
    use super::{
        format_server_status_line, format_topic_preview, normalize_whitespace, room_status_label,
        server_status_label, truncate_chars,
    };
    use crate::models::{RoomStatus, ServerHealth, ServerStatus};

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

    #[test]
    fn server_status_labels_are_stable() {
        assert_eq!(server_status_label(ServerStatus::Online), "online");
        assert_eq!(server_status_label(ServerStatus::Offline), "offline");
        assert_eq!(server_status_label(ServerStatus::Restricted), "restricted");
        assert_eq!(server_status_label(ServerStatus::Unknown), "unknown");
    }

    #[test]
    fn room_status_labels_are_stable() {
        assert_eq!(room_status_label(RoomStatus::Resolvable), "resolvable");
        assert_eq!(room_status_label(RoomStatus::NotFound), "not found");
        assert_eq!(room_status_label(RoomStatus::NoAlias), "no alias");
        assert_eq!(room_status_label(RoomStatus::Unknown), "unknown");
    }

    #[test]
    fn server_status_line_formats_online_server() {
        let health = ServerHealth {
            server: "matrix.org".to_string(),
            status: ServerStatus::Online,
            public_rooms_available: true,
            latency_ms: Some(42),
            reason: None,
        };

        assert_eq!(
            format_server_status_line(&health),
            "matrix.org             online         42ms  publicRooms: yes"
        );
    }

    #[test]
    fn server_status_line_formats_reason_and_missing_latency() {
        let health = ServerHealth {
            server: "example.org".to_string(),
            status: ServerStatus::Restricted,
            public_rooms_available: false,
            latency_ms: None,
            reason: Some("publicRooms endpoint returned 403 Forbidden".to_string()),
        };

        assert_eq!(
            format_server_status_line(&health),
            "example.org            restricted        -  publicRooms: no (publicRooms endpoint returned 403 Forbidden)"
        );
    }

    #[test]
    fn server_status_json_uses_stable_field_names() {
        let health = ServerHealth {
            server: "matrix.org".to_string(),
            status: ServerStatus::Offline,
            public_rooms_available: false,
            latency_ms: Some(5000),
            reason: Some("timeout".to_string()),
        };

        let json = serde_json::to_value(&health).expect("server health should serialize");

        assert_eq!(json["server"], "matrix.org");
        assert_eq!(json["status"], "Offline");
        assert_eq!(json["public_rooms_available"], false);
        assert_eq!(json["latency_ms"], 5000);
        assert_eq!(json["reason"], "timeout");
    }
}
