use std::time::Duration;
use std::time::Instant;

use reqwest::StatusCode;
use serde::{Deserialize, Serialize};

use crate::models::{Room, ServerHealth, ServerStatus};

const PUBLIC_ROOMS_PAGE_LIMIT: u64 = 100;
const HEALTH_CHECK_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Deserialize)]
struct PublicRoomsResponse {
    chunk: Vec<PublicRoom>,
    next_batch: Option<String>,
}

#[derive(Serialize)]
struct PublicRoomsQuery<'a> {
    limit: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    since: Option<&'a str>,
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
    let client = matrix_http_client(Duration::from_secs(10))?;

    let url = format!("https://{server}/_matrix/client/v3/publicRooms");
    let mut rooms = Vec::new();
    let mut next_batch = None;

    loop {
        let query = PublicRoomsQuery {
            limit: PUBLIC_ROOMS_PAGE_LIMIT,
            since: next_batch.as_deref(),
        };

        let response = client
            .get(&url)
            .query(&query)
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

        rooms.extend(response.chunk.into_iter().map(|room| Room {
            room_id: room.room_id,
            name: room.name,
            topic: room.topic,
            canonical_alias: room.canonical_alias,
            num_joined_members: room.num_joined_members,
            server: server.to_string(),
        }));

        let Some(batch) = response.next_batch else {
            break;
        };

        next_batch = Some(batch);
    }

    Ok(rooms)
}

pub async fn check_server_health(server: &str) -> ServerHealth {
    let server = server.trim().to_string();

    if server.is_empty() {
        return ServerHealth {
            server,
            status: ServerStatus::Unknown,
            public_rooms_available: false,
            latency_ms: None,
            reason: Some("empty server name".to_string()),
        };
    }

    let client = match matrix_http_client(HEALTH_CHECK_TIMEOUT) {
        Ok(client) => client,
        Err(error) => {
            return ServerHealth {
                server,
                status: ServerStatus::Unknown,
                public_rooms_available: false,
                latency_ms: None,
                reason: Some(error.to_string()),
            };
        }
    };

    let public_rooms_url = format!("https://{server}/_matrix/client/v3/publicRooms");
    let started_at = Instant::now();
    let public_rooms_response = client
        .get(&public_rooms_url)
        .query(&PublicRoomsQuery {
            limit: 1,
            since: None,
        })
        .send()
        .await;

    match public_rooms_response {
        Ok(response) => {
            let status = response.status();
            let classification = classify_public_rooms_health_status(status);

            ServerHealth {
                server,
                status: classification.status,
                public_rooms_available: classification.public_rooms_available,
                latency_ms: Some(started_at.elapsed().as_millis()),
                reason: classification
                    .include_reason
                    .then(|| format!("publicRooms endpoint returned {status}")),
            }
        }
        Err(error) => ServerHealth {
            server,
            status: status_from_request_error(&error),
            public_rooms_available: false,
            latency_ms: Some(started_at.elapsed().as_millis()),
            reason: Some(request_error_reason(&error)),
        },
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct HealthStatusClassification {
    status: ServerStatus,
    public_rooms_available: bool,
    include_reason: bool,
}

fn classify_public_rooms_health_status(status: StatusCode) -> HealthStatusClassification {
    if status.is_success() {
        return HealthStatusClassification {
            status: ServerStatus::Online,
            public_rooms_available: true,
            include_reason: false,
        };
    }

    if matches!(status, StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN) {
        return HealthStatusClassification {
            status: ServerStatus::Restricted,
            public_rooms_available: false,
            include_reason: true,
        };
    }

    HealthStatusClassification {
        status: ServerStatus::Unknown,
        public_rooms_available: false,
        include_reason: true,
    }
}

fn matrix_http_client(timeout: Duration) -> Result<reqwest::Client, PublicRoomsError> {
    reqwest::Client::builder()
        .timeout(timeout)
        .build()
        .map_err(|error| {
            PublicRoomsError::new(
                PublicRoomsErrorKind::NetworkError,
                format!("failed to build HTTP client: {error}"),
            )
        })
}

fn status_from_request_error(error: &reqwest::Error) -> ServerStatus {
    if error.is_timeout() || error.is_connect() {
        ServerStatus::Offline
    } else {
        ServerStatus::Unknown
    }
}

fn request_error_reason(error: &reqwest::Error) -> String {
    if error.is_timeout() {
        "timeout".to_string()
    } else {
        format!("network error: {error}")
    }
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

#[cfg(test)]
mod tests {
    use super::{classify_public_rooms_health_status, HealthStatusClassification};
    use crate::models::ServerStatus;
    use reqwest::StatusCode;

    #[test]
    fn successful_public_rooms_status_is_online() {
        assert_eq!(
            classify_public_rooms_health_status(StatusCode::OK),
            HealthStatusClassification {
                status: ServerStatus::Online,
                public_rooms_available: true,
                include_reason: false,
            }
        );
    }

    #[test]
    fn unauthorized_public_rooms_status_is_restricted() {
        assert_eq!(
            classify_public_rooms_health_status(StatusCode::UNAUTHORIZED),
            HealthStatusClassification {
                status: ServerStatus::Restricted,
                public_rooms_available: false,
                include_reason: true,
            }
        );
    }

    #[test]
    fn forbidden_public_rooms_status_is_restricted() {
        assert_eq!(
            classify_public_rooms_health_status(StatusCode::FORBIDDEN),
            HealthStatusClassification {
                status: ServerStatus::Restricted,
                public_rooms_available: false,
                include_reason: true,
            }
        );
    }

    #[test]
    fn unexpected_public_rooms_status_is_unknown() {
        assert_eq!(
            classify_public_rooms_health_status(StatusCode::INTERNAL_SERVER_ERROR),
            HealthStatusClassification {
                status: ServerStatus::Unknown,
                public_rooms_available: false,
                include_reason: true,
            }
        );
    }
}
