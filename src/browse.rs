//! Browse MCP servers from registry sources.

use std::error::Error;
use std::time::Duration;

use crate::paths::Paths;
use crate::sources::list_sources;

fn build_http_client() -> Result<reqwest::blocking::Client, reqwest::Error> {
    reqwest::blocking::Client::builder()
        .user_agent("dmcp/1.0")
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(30))
        .build()
}

/// A server entry from a registry (for display).
#[derive(Debug, Clone, serde::Serialize)]
pub struct RegistryServer {
    pub id: String,
    pub name: String,
    pub summary: String,
    pub version: String,
    pub transport: String,
    pub source: String,
    /// Whether this server is installed (user or system scope).
    pub installed: bool,
}

/// Fetch and list servers from a specific registry URL.
pub fn list_registry_servers_from_url(url: &str) -> Result<Vec<RegistryServer>, BrowseError> {
    let client = build_http_client().map_err(BrowseError::HttpClient)?;
    fetch_registry(&client, url).map_err(|e| BrowseError::FetchFailed {
        url: url.to_string(),
        cause: e,
    })
}

/// Fetch and list all servers from configured registry sources.
/// Returns (servers, errors). Servers may be duplicated across sources.
pub fn list_registry_servers(
    paths: &Paths,
    include_user: bool,
    include_system: bool,
) -> (Vec<RegistryServer>, Vec<BrowseError>) {
    let sources = list_sources(paths, include_user, include_system);
    let mut servers = Vec::new();
    let mut errors = Vec::new();

    let client = match build_http_client() {
        Ok(c) => c,
        Err(e) => {
            errors.push(BrowseError::HttpClient(e));
            return (servers, errors);
        }
    };

    for (url, _scope) in sources {
        match fetch_registry(&client, &url) {
            Ok(registry_servers) => {
                servers.extend(registry_servers);
            }
            Err(e) => {
                errors.push(BrowseError::FetchFailed {
                    url: url.clone(),
                    cause: e,
                });
            }
        }
    }

    (servers, errors)
}

fn fetch_registry(
    client: &reqwest::blocking::Client,
    url: &str,
) -> Result<Vec<RegistryServer>, reqwest::Error> {
    let resp = client.get(url).send()?;
    if !resp.status().is_success() {
        return Err(resp.error_for_status().unwrap_err());
    }
    let registry: serde_json::Value = resp.json()?;
    let servers_val = registry.get("servers");
    let servers_array: Vec<serde_json::Value> = match servers_val {
        Some(s) if s.is_array() => s.as_array().unwrap().clone(),
        Some(s) if s.is_object() => {
            let obj = s.as_object().unwrap();
            obj.values().cloned().collect()
        }
        _ => return Ok(vec![]),
    };

    let mut result = Vec::new();
    for server in servers_array {
        let id = server
            .get("id")
            .and_then(|i| i.as_str())
            .unwrap_or("?")
            .to_string();
        let name = server
            .get("name")
            .and_then(|n| n.as_str())
            .unwrap_or("?")
            .to_string();
        let summary = server
            .get("summary")
            .and_then(|s| s.as_str())
            .unwrap_or("")
            .to_string();
        let version = server
            .get("version")
            .and_then(|v| v.as_str())
            .unwrap_or("?")
            .to_string();

        let transport = server
            .get("transports")
            .and_then(|t| t.as_array())
            .and_then(|a| a.first())
            .and_then(|t| t.get("type").and_then(|x| x.as_str()))
            .unwrap_or("?")
            .to_string();

        result.push(RegistryServer {
            id,
            name,
            summary,
            version,
            transport,
            source: url.to_string(),
            installed: false,
        });
    }

    Ok(result)
}

#[derive(Debug)]
pub enum BrowseError {
    HttpClient(reqwest::Error),
    FetchFailed {
        url: String,
        cause: reqwest::Error,
    },
}

impl std::fmt::Display for BrowseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BrowseError::HttpClient(e) => {
                write!(f, "HTTP client error: {}", e)?;
                if let Some(s) = e.source() {
                    write!(f, "\n  Caused by: {}", s)?;
                }
                Ok(())
            }
            BrowseError::FetchFailed { url, cause } => {
                write!(f, "Failed to fetch {}: {}", url, cause)?;
                // Show error chain for more diagnostic detail
                let mut source: Option<&(dyn Error + '_)> = cause.source();
                while let Some(s) = source {
                    write!(f, "\n  Caused by: {}", s)?;
                    source = s.source();
                }
                Ok(())
            }
        }
    }
}

impl std::error::Error for BrowseError {}
