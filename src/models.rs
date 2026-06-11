use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerHealth {
    pub server: String,
    pub status: ServerStatus,
    pub public_rooms_available: bool,
    pub latency_ms: Option<u128>,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ServerStatus {
    Online,
    Offline,
    Restricted,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Room {
    pub room_id: String,
    pub name: Option<String>,
    pub topic: Option<String>,
    pub canonical_alias: Option<String>,
    pub num_joined_members: Option<u64>,
    pub server: String,
}

impl Room {
    pub fn matrix_to_url(&self) -> String {
        let identifier = self.canonical_alias.as_deref().unwrap_or(&self.room_id);

        format!("https://matrix.to/#/{identifier}")
    }
}
