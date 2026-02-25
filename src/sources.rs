//! Registry sources (sources.list).

use std::path::Path;

use crate::paths::Paths;

/// Load registry URLs from sources.list files.
/// Shows all entries from both scopes (no deduplication) so the user can see
/// where each URL is configured. For execution/fetch, user scope takes priority.
pub fn list_sources(paths: &Paths, include_user: bool, include_system: bool) -> Vec<(String, SourceScope)> {
    let mut result = Vec::new();

    if include_user {
        for url in read_sources_file(paths.user_sources_path()) {
            result.push((url, SourceScope::User));
        }
    }

    if include_system {
        for url in read_sources_file(paths.system_sources_path()) {
            result.push((url, SourceScope::System));
        }
    }

    result
}

/// Add a registry URL. Creates file and parent dir if needed.
pub fn add_source(paths: &Paths, url: &str, scope: SourceScope) -> Result<(), SourcesError> {
    let path = match scope {
        SourceScope::User => paths.user_sources_path().to_path_buf(),
        SourceScope::System => paths.system_sources_path().to_path_buf(),
    };

    let url = url.trim();
    if url.is_empty() {
        return Err(SourcesError::InvalidUrl);
    }

    // Ensure parent dir exists (for user scope)
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(SourcesError::CreateDir)?;
    }

    let content = std::fs::read_to_string(&path).unwrap_or_default();
    let urls: Vec<&str> = content
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .collect();

    if urls.contains(&url) {
        return Err(SourcesError::AlreadyExists);
    }

    let mut new_content = content.trim_end().to_string();
    if !new_content.is_empty() && !new_content.ends_with('\n') {
        new_content.push('\n');
    }
    new_content.push_str(url);
    new_content.push('\n');

    std::fs::write(&path, new_content).map_err(|e| SourcesError::WriteFailed(e, path))?;

    Ok(())
}

/// Remove a registry URL.
pub fn remove_source(paths: &Paths, url: &str, scope: SourceScope) -> Result<(), SourcesError> {
    let path = match scope {
        SourceScope::User => paths.user_sources_path().to_path_buf(),
        SourceScope::System => paths.system_sources_path().to_path_buf(),
    };

    let url = url.trim();
    if url.is_empty() {
        return Err(SourcesError::InvalidUrl);
    }

    let content = std::fs::read_to_string(&path).map_err(|e| SourcesError::ReadFailed(e))?;

    let original_urls: Vec<&str> = content
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .collect();
    if !original_urls.contains(&url) {
        return Err(SourcesError::NotFound);
    }

    let lines: Vec<String> = content
        .lines()
        .filter(|l| l.trim() != url)
        .map(String::from)
        .collect();

    let new_content = lines.join("\n");
    let new_content = if new_content.is_empty() || new_content.ends_with('\n') {
        new_content
    } else {
        format!("{}\n", new_content)
    };

    std::fs::write(&path, new_content).map_err(|e| SourcesError::WriteFailed(e, path))?;

    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceScope {
    User,
    System,
}

#[derive(Debug)]
pub enum SourcesError {
    InvalidUrl,
    AlreadyExists,
    NotFound,
    CreateDir(std::io::Error),
    ReadFailed(std::io::Error),
    WriteFailed(std::io::Error, std::path::PathBuf),
}

impl std::fmt::Display for SourcesError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SourcesError::InvalidUrl => write!(f, "Invalid or empty URL"),
            SourcesError::AlreadyExists => write!(f, "Source URL already exists"),
            SourcesError::NotFound => write!(f, "Source URL not found"),
            SourcesError::CreateDir(e) => write!(f, "Failed to create directory: {}", e),
            SourcesError::ReadFailed(e) => write!(f, "Failed to read sources file: {}", e),
            SourcesError::WriteFailed(e, _) => write!(f, "Failed to write sources file: {}", e),
        }
    }
}

impl std::error::Error for SourcesError {}

fn read_sources_file(path: &Path) -> Vec<String> {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return vec![],
    };

    content
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(String::from)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paths::Paths;
    use std::path::{Path, PathBuf};

    fn temp_paths(dir: &Path) -> Paths {
        Paths {
            user_sources: dir.join("user_sources.list"),
            user_install_dir: dir.join("user_install"),
            system_sources: dir.join("system_sources.list"),
            system_install_dir: dir.join("system_install"),
        }
    }

    fn make_temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("dmcp_test_sources_{}", name));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn add_and_list_source() {
        let dir = make_temp_dir("add_list");
        let paths = temp_paths(&dir);
        add_source(&paths, "https://example.com/registry", SourceScope::User).unwrap();
        let sources = list_sources(&paths, true, false);
        assert_eq!(sources.len(), 1);
        assert_eq!(sources[0].0, "https://example.com/registry");
        assert_eq!(sources[0].1, SourceScope::User);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn add_source_duplicate_errors() {
        let dir = make_temp_dir("add_dup");
        let paths = temp_paths(&dir);
        add_source(&paths, "https://example.com/registry", SourceScope::User).unwrap();
        let result = add_source(&paths, "https://example.com/registry", SourceScope::User);
        assert!(matches!(result, Err(SourcesError::AlreadyExists)));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn add_source_empty_url_errors() {
        let dir = make_temp_dir("add_empty");
        let paths = temp_paths(&dir);
        let result = add_source(&paths, "   ", SourceScope::User);
        assert!(matches!(result, Err(SourcesError::InvalidUrl)));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn remove_source_works() {
        let dir = make_temp_dir("remove");
        let paths = temp_paths(&dir);
        add_source(&paths, "https://example.com/registry", SourceScope::User).unwrap();
        remove_source(&paths, "https://example.com/registry", SourceScope::User).unwrap();
        let sources = list_sources(&paths, true, false);
        assert!(sources.is_empty());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn remove_source_not_found_errors() {
        let dir = make_temp_dir("remove_not_found");
        let paths = temp_paths(&dir);
        add_source(&paths, "https://example.com/registry", SourceScope::User).unwrap();
        let result = remove_source(&paths, "https://other.com/different", SourceScope::User);
        assert!(matches!(result, Err(SourcesError::NotFound)));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn list_sources_skips_comments_and_blank_lines() {
        let dir = make_temp_dir("comments");
        let paths = temp_paths(&dir);
        std::fs::write(
            &paths.user_sources,
            "# comment\n\nhttps://example.com\n  \nhttps://other.com\n",
        )
        .unwrap();
        let sources = list_sources(&paths, true, false);
        assert_eq!(sources.len(), 2);
        assert_eq!(sources[0].0, "https://example.com");
        assert_eq!(sources[1].0, "https://other.com");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn list_sources_missing_file_returns_empty() {
        let dir = make_temp_dir("missing");
        let paths = temp_paths(&dir);
        // Don't create the sources file
        let sources = list_sources(&paths, true, false);
        assert!(sources.is_empty());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn add_multiple_sources_preserved() {
        let dir = make_temp_dir("multi");
        let paths = temp_paths(&dir);
        add_source(&paths, "https://alpha.example.com", SourceScope::User).unwrap();
        add_source(&paths, "https://beta.example.com", SourceScope::User).unwrap();
        let sources = list_sources(&paths, true, false);
        assert_eq!(sources.len(), 2);
        std::fs::remove_dir_all(&dir).ok();
    }
}
