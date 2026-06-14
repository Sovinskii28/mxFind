use std::time::Duration;
use std::time::Instant;

use reqwest::StatusCode;
use serde::{Deserialize, Serialize};

use crate::models::{Room, RoomHealth, RoomStatus, ServerHealth, ServerStatus};

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

#[derive(Serialize)]
struct PublicRoomsSearchRequest<'a> {
    limit: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    since: Option<&'a str>,
    filter: PublicRoomsFilter<'a>,
}

#[derive(Serialize)]
struct PublicRoomsFilter<'a> {
    generic_search_term: &'a str,
}

#[derive(Deserialize)]
struct PublicRoom {
    room_id: String,
    name: Option<String>,
    topic: Option<String>,
    canonical_alias: Option<String>,
    num_joined_members: Option<u64>,
}

#[derive(Deserialize)]
struct RoomDirectoryResponse {
    room_id: String,
}

#[derive(Deserialize)]
struct ClientWellKnown {
    #[serde(rename = "m.homeserver")]
    homeserver: Option<ClientWellKnownHomeserver>,
}

#[derive(Deserialize)]
struct ClientWellKnownHomeserver {
    base_url: String,
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

    pub fn kind(&self) -> PublicRoomsErrorKind {
        self.kind
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

pub async fn fetch_public_rooms_search(
    server: &str,
    search_term: &str,
    result_limit: usize,
) -> Result<Vec<Room>, PublicRoomsError> {
    let search_term = search_term.trim();

    if result_limit == 0 {
        return Ok(Vec::new());
    }

    if search_term.is_empty() {
        return fetch_public_rooms(server).await;
    }

    let client = matrix_http_client(Duration::from_secs(10))?;
    let url = format!("https://{server}/_matrix/client/v3/publicRooms");
    let mut rooms = Vec::new();
    let mut next_batch = None;

    while rooms.len() < result_limit {
        let remaining = result_limit - rooms.len();
        let limit = PUBLIC_ROOMS_PAGE_LIMIT.min(remaining as u64);
        let request = PublicRoomsSearchRequest {
            limit,
            since: next_batch.as_deref(),
            filter: PublicRoomsFilter {
                generic_search_term: search_term,
            },
        };

        let response = client
            .post(&url)
            .json(&request)
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

    rooms.truncate(result_limit);
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

pub async fn check_room_health(room: &Room) -> RoomHealth {
    let Some(alias) = room.canonical_alias.as_deref() else {
        return RoomHealth {
            room_id: room.room_id.clone(),
            alias: None,
            status: RoomStatus::NoAlias,
            resolved_room_id: None,
            latency_ms: None,
            reason: Some("room has no canonical alias".to_string()),
        };
    };

    let client = match matrix_http_client(HEALTH_CHECK_TIMEOUT) {
        Ok(client) => client,
        Err(error) => {
            return RoomHealth {
                room_id: room.room_id.clone(),
                alias: Some(alias.to_string()),
                status: RoomStatus::Unknown,
                resolved_room_id: None,
                latency_ms: None,
                reason: Some(error.to_string()),
            };
        }
    };

    let encoded_alias = percent_encode_path_segment(alias);
    let alias_server = room_alias_server_name(alias).unwrap_or(&room.server);
    let base_url = discover_client_base_url(&client, alias_server).await;
    let url = format!("{base_url}/_matrix/client/v3/directory/room/{encoded_alias}");
    let started_at = Instant::now();
    let response = client.get(&url).send().await;

    match response {
        Ok(response) if response.status().is_success() => {
            let latency_ms = Some(started_at.elapsed().as_millis());
            match response.json::<RoomDirectoryResponse>().await {
                Ok(directory) => RoomHealth {
                    room_id: room.room_id.clone(),
                    alias: Some(alias.to_string()),
                    status: RoomStatus::Resolvable,
                    resolved_room_id: Some(directory.room_id),
                    latency_ms,
                    reason: None,
                },
                Err(error) => RoomHealth {
                    room_id: room.room_id.clone(),
                    alias: Some(alias.to_string()),
                    status: RoomStatus::Unknown,
                    resolved_room_id: None,
                    latency_ms,
                    reason: Some(format!("invalid directory response: {error}")),
                },
            }
        }
        Ok(response) if response.status() == StatusCode::NOT_FOUND => RoomHealth {
            room_id: room.room_id.clone(),
            alias: Some(alias.to_string()),
            status: RoomStatus::NotFound,
            resolved_room_id: None,
            latency_ms: Some(started_at.elapsed().as_millis()),
            reason: Some(format!("directory endpoint returned {}", response.status())),
        },
        Ok(response) => RoomHealth {
            room_id: room.room_id.clone(),
            alias: Some(alias.to_string()),
            status: RoomStatus::Unknown,
            resolved_room_id: None,
            latency_ms: Some(started_at.elapsed().as_millis()),
            reason: Some(format!("directory endpoint returned {}", response.status())),
        },
        Err(error) => RoomHealth {
            room_id: room.room_id.clone(),
            alias: Some(alias.to_string()),
            status: RoomStatus::Unknown,
            resolved_room_id: None,
            latency_ms: Some(started_at.elapsed().as_millis()),
            reason: Some(request_error_reason(&error)),
        },
    }
}

async fn discover_client_base_url(client: &reqwest::Client, server_name: &str) -> String {
    let fallback = format!("https://{server_name}");
    let url = format!("{fallback}/.well-known/matrix/client");

    let Ok(response) = client.get(url).send().await else {
        return fallback;
    };

    if !response.status().is_success() {
        return fallback;
    }

    let Ok(well_known) = response.json::<ClientWellKnown>().await else {
        return fallback;
    };

    well_known
        .homeserver
        .and_then(|homeserver| normalize_client_base_url(&homeserver.base_url))
        .unwrap_or(fallback)
}

fn normalize_client_base_url(base_url: &str) -> Option<String> {
    let base_url = base_url.trim().trim_end_matches('/');

    if base_url.starts_with("https://") || base_url.starts_with("http://") {
        Some(base_url.to_string())
    } else {
        None
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

fn percent_encode_path_segment(value: &str) -> String {
    let mut encoded = String::new();

    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                encoded.push(byte as char);
            }
            _ => encoded.push_str(&format!("%{byte:02X}")),
        }
    }

    encoded
}

fn room_alias_server_name(alias: &str) -> Option<&str> {
    let (_, server_name) = alias.rsplit_once(':')?;

    if alias.starts_with('#') && !server_name.is_empty() {
        Some(server_name)
    } else {
        None
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
        StatusCode::BAD_REQUEST | StatusCode::METHOD_NOT_ALLOWED => {
            PublicRoomsErrorKind::InvalidResponse
        }
        _ => PublicRoomsErrorKind::NetworkError,
    };

    PublicRoomsError::new(kind, status.to_string())
}

#[cfg(test)]
mod tests {
    use super::{
        classify_public_rooms_health_status, normalize_client_base_url,
        percent_encode_path_segment, room_alias_server_name, HealthStatusClassification,
    };
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

    #[test]
    fn room_alias_is_encoded_as_one_path_segment() {
        assert_eq!(
            percent_encode_path_segment("#rabbithole:kawaiiloli.twilightparadox.com"),
            "%23rabbithole%3Akawaiiloli.twilightparadox.com"
        );
    }

    #[test]
    fn room_alias_server_name_is_extracted_from_alias() {
        assert_eq!(
            room_alias_server_name("#rabbithole:kawaiiloli.twilightparadox.com"),
            Some("kawaiiloli.twilightparadox.com")
        );
    }

    #[test]
    fn malformed_room_alias_has_no_server_name() {
        assert_eq!(room_alias_server_name("!room:matrix.org"), None);
        assert_eq!(room_alias_server_name("#room:"), None);
    }

    #[test]
    fn client_base_url_is_normalized() {
        assert_eq!(
            normalize_client_base_url("https://matrix.example.org/"),
            Some("https://matrix.example.org".to_string())
        );
        assert_eq!(normalize_client_base_url("matrix.example.org"), None);
    }
}
