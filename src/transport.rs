//! Transport type extraction from manifests.
//!
//! MVP: Pull transport from manifest when registry doesn't include it.
//! TODO: Remove once registry JSON format includes transports.

use std::path::Path;

/// Extract transport type from manifest JSON (e.g. "stdio", "sse", "websocket").
pub fn transport_from_manifest_json(json: &serde_json::Value) -> Option<String> {
    let transports = json.get("transports")?.as_array()?;
    let first = transports.first()?;
    first.get("type").and_then(|t| t.as_str()).map(String::from)
}

/// Fetch manifest from URL and extract transport type.
pub fn transport_from_manifest_url(
    client: &reqwest::blocking::Client,
    url: &str,
) -> Option<String> {
    let resp = client.get(url).send().ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let json: serde_json::Value = resp.json().ok()?;
    transport_from_manifest_json(&json)
}

/// Read manifest from local path and extract transport type.
pub fn transport_from_manifest_path(path: &Path) -> Option<String> {
    let content = std::fs::read_to_string(path).ok()?;
    let json: serde_json::Value = serde_json::from_str(&content).ok()?;
    transport_from_manifest_json(&json)
}
