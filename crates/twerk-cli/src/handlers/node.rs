//! Node command handlers
//!
//! HTTP client functions for node API operations.

use serde::Deserialize;

use crate::error::CliError;

#[derive(Debug, Deserialize)]
pub struct Node {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub hostname: Option<String>,
    #[serde(default)]
    pub port: Option<u16>,
    pub status: String,
    #[serde(default)]
    pub role: Option<String>,
    #[serde(default)]
    pub version: Option<String>,
}

pub async fn node_list(endpoint: &str, json_mode: bool) -> Result<String, CliError> {
    let url = format!("{}/nodes", endpoint.trim_end_matches('/'));

    let response = reqwest::get(&url).await.map_err(CliError::Http)?;

    let status = response.status();

    if !status.is_success() {
        return Err(CliError::HttpStatus {
            status: status.as_u16(),
            reason: status
                .canonical_reason()
                .map_or_else(|| "Unknown".to_string(), |s| s.to_string()),
        });
    }

    let body = response
        .text()
        .await
        .map_err(|e| CliError::InvalidBody(e.to_string()))?;

    let nodes: Vec<Node> =
        serde_json::from_str(&body).map_err(|e| CliError::InvalidBody(e.to_string()))?;

    if json_mode {
        println!("{}", body);
    } else {
        if nodes.is_empty() {
            println!("No nodes found.");
        } else {
            println!(
                "{:<20} {:<30} {:<15} {:<10}",
                "ID", "NAME", "STATUS", "ROLE"
            );
            println!("{}", "-".repeat(80));
            for n in &nodes {
                println!(
                    "{:<20} {:<30} {:<15} {:<10}",
                    n.id,
                    n.name,
                    n.status,
                    n.role.as_deref().map_or("-", |s| s)
                );
            }
        }
    }

    Ok(body)
}

pub async fn node_get(endpoint: &str, id: &str, json_mode: bool) -> Result<String, CliError> {
    let url = format!("{}/nodes/{}", endpoint.trim_end_matches('/'), id);

    let response = reqwest::get(&url).await.map_err(CliError::Http)?;

    let status = response.status();

    if status == reqwest::StatusCode::NOT_FOUND {
        return Err(CliError::NotFound(format!("node {} not found", id)));
    }

    if !status.is_success() {
        return Err(CliError::HttpStatus {
            status: status.as_u16(),
            reason: status
                .canonical_reason()
                .map_or_else(|| "Unknown".to_string(), |s| s.to_string()),
        });
    }

    let body = response
        .text()
        .await
        .map_err(|e| CliError::InvalidBody(e.to_string()))?;

    let node: Node =
        serde_json::from_str(&body).map_err(|e| CliError::InvalidBody(e.to_string()))?;

    if json_mode {
        println!("{}", body);
    } else {
        println!("Node: {}", node.id);
        println!("Name: {}", node.name);
        if let Some(hostname) = &node.hostname {
            println!("Hostname: {}", hostname);
        }
        if let Some(port) = &node.port {
            println!("Port: {}", port);
        }
        println!("Status: {}", node.status);
        if let Some(role) = &node.role {
            println!("Role: {}", role);
        }
        if let Some(version) = &node.version {
            println!("Version: {}", version);
        }
    }

    Ok(body)
}
