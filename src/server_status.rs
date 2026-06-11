use std::collections::HashSet;

use futures::stream::{FuturesUnordered, StreamExt};

use crate::matrix::check_server_health;
use crate::models::ServerHealth;

pub async fn check_servers_status(servers: Vec<String>) -> Vec<ServerHealth> {
    let servers = normalize_servers(servers);
    let mut checks = FuturesUnordered::new();

    for (index, server) in servers.into_iter().enumerate() {
        checks.push(async move {
            let health = check_server_health(&server).await;
            (index, health)
        });
    }

    let mut statuses = Vec::new();

    while let Some(status) = checks.next().await {
        statuses.push(status);
    }

    statuses.sort_by_key(|(index, _)| *index);
    statuses
        .into_iter()
        .map(|(_, health)| health)
        .collect::<Vec<_>>()
}

fn normalize_servers(servers: Vec<String>) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut normalized = Vec::new();

    for server in servers {
        let server = server.trim().to_lowercase();

        if server.is_empty() || !seen.insert(server.clone()) {
            continue;
        }

        normalized.push(server);
    }

    normalized
}

#[cfg(test)]
mod tests {
    use super::normalize_servers;

    #[test]
    fn servers_are_trimmed_lowercased_and_deduplicated() {
        let servers = vec![
            " matrix.org ".to_string(),
            "MATRIX.ORG".to_string(),
            "".to_string(),
            " tchncs.de ".to_string(),
        ];

        assert_eq!(
            normalize_servers(servers),
            vec!["matrix.org".to_string(), "tchncs.de".to_string()]
        );
    }

    #[test]
    fn first_server_order_is_preserved() {
        let servers = vec![
            "b.example".to_string(),
            "a.example".to_string(),
            "b.example".to_string(),
        ];

        assert_eq!(
            normalize_servers(servers),
            vec!["b.example".to_string(), "a.example".to_string()]
        );
    }
}
