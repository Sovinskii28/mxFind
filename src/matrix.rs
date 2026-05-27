use std::time::Duration;

use reqwest::StatusCode;
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PublicRoomsErrorKind {
    NotFound,
    Unauthorized,
    Timeout,
    NetworkError,
    InvalidResponse,
}

#[derive(Debug)]
pub struct PublicRoomsError {
    kind: PublicRoomsErrorKind,
    detail: String,
}

impl PublicRoomsError {
    fn new(kind: PublicRoomsErrorKind, detail: impl Into<String>) -> Self {
        Self {
            kind,
            detail: detail.into(),
        }
    }
}

impl std::fmt::Display for PublicRoomsError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.kind {
            PublicRoomsErrorKind::NotFound | PublicRoomsErrorKind::Unauthorized => {
                formatter.write_str(&self.detail)
            }
            PublicRoomsErrorKind::Timeout => formatter.write_str("timeout"),
            PublicRoomsErrorKind::NetworkError => {
                write!(formatter, "network error: {}", self.detail)
            }
            PublicRoomsErrorKind::InvalidResponse => {
                write!(formatter, "invalid response: {}", self.detail)
            }
        }
    }
}

impl std::error::Error for PublicRoomsError {}

pub async fn fetch_public_rooms(server: &str) -> Result<Vec<Room>, PublicRoomsError> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|error| {
            PublicRoomsError::new(
                PublicRoomsErrorKind::NetworkError,
                format!("failed to build HTTP client: {error}"),
            )
        })?;

    let url = format!("https://{server}/_matrix/client/v3/publicRooms");
    let response = client
        .get(&url)
        .send()
        .await
        .map_err(classify_request_error)?;

    let status = response.status();
    if !status.is_success() {
        return Err(classify_http_status(status));
    }

    let response = response
        .json::<PublicRoomsResponse>()
        .await
        .map_err(|error| {
            PublicRoomsError::new(PublicRoomsErrorKind::InvalidResponse, error.to_string())
        })?;

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

fn classify_request_error(error: reqwest::Error) -> PublicRoomsError {
    if error.is_timeout() {
        PublicRoomsError::new(PublicRoomsErrorKind::Timeout, "request timed out")
    } else {
        PublicRoomsError::new(PublicRoomsErrorKind::NetworkError, error.to_string())
    }
}

fn classify_http_status(status: StatusCode) -> PublicRoomsError {
    let kind = match status {
        StatusCode::NOT_FOUND => PublicRoomsErrorKind::NotFound,
        StatusCode::UNAUTHORIZED => PublicRoomsErrorKind::Unauthorized,
        _ => PublicRoomsErrorKind::NetworkError,
    };

    PublicRoomsError::new(kind, status.to_string())
}
