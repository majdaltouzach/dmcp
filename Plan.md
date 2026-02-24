# dmcp Implementation Plan

## High Priority

| Feature | Description | Status |
|---------|-------------|--------|
| **`dmcp config <id> set <key> <value>`** | Write config (API keys, endpoints). Update manifest and persist. | Done |
| **`dmcp sources add <url>`** | Add a registry URL to sources.list | Done |
| **`dmcp sources remove <url>`** | Remove a registry URL from sources.list | Done |

## Medium Priority

| Feature | Description | Status |
|---------|-------------|--------|
| **`dmcp run <id>`** | Spawn stdio servers (config as env); SSE/WebSocket: print URL | Done |
| **`dmcp browse [url]`** | Fetch registries, list available servers (or specific URL) | Done |
| **`dmcp install <id>`** | Install from registry (clone for stdio, metadata for remote) | Done |
| **`dmcp uninstall <id>`** | Remove installed server | Done |
| **`dmcp connect <url>`** | Add remote server: fetch manifest from URL if valid JSON, else treat as raw endpoint | Done |
| **`dmcp setup <id>`** | Run setup script (dependencies, config) for installed server | Done |

## Lower Priority

| Feature | Description | Status |
|---------|-------------|--------|
| **`dmcp get-connection-info <id>`** | Output connection descriptor (JSON) for clients | Pending |
| **`dmcp validate <id>`** | Check manifest and executable | Pending |
| **`dmcp update <id>`** | Update local server (git pull) or refresh remote metadata | Pending |

---

## Current Focus

**`dmcp get-connection-info <id>`** or **`dmcp validate <id>`** — Next up.
