//! Path resolution for user and system scope.
//!
//! Uses env vars when set, otherwise XDG defaults.

use std::path::{Path, PathBuf};

/// Resolved paths for MCP directories.
#[derive(Debug, Clone)]
pub struct Paths {
    pub user_sources: PathBuf,
    pub user_install_dir: PathBuf,
    pub system_sources: PathBuf,
    pub system_install_dir: PathBuf,
}

impl Paths {
    /// Resolve paths from environment, falling back to XDG/defaults.
    pub fn resolve() -> Self {
        let user_sources = resolve_path(
            "MCP_USER_SOURCES_PATH",
            dirs::config_dir().map(|p| p.join("mcp/sources.list")),
            "~/.config/mcp/sources.list",
        );
        let user_install_dir = resolve_path(
            "MCP_USER_INSTALL_DIR",
            dirs::data_local_dir().map(|p| p.join("mcp/installed")),
            "~/.local/share/mcp/installed/",
        );
        let system_sources = resolve_path(
            "MCP_SYSTEM_SOURCES_PATH",
            Some(PathBuf::from("/etc/mcp/sources.list")),
            "/etc/mcp/sources.list",
        );
        let system_install_dir = resolve_path(
            "MCP_SYSTEM_INSTALL_DIR",
            Some(PathBuf::from("/usr/share/mcp/installed/")),
            "/usr/share/mcp/installed/",
        );

        Self {
            user_sources,
            user_install_dir,
            system_sources,
            system_install_dir,
        }
    }

    /// User sources list path.
    pub fn user_sources_path(&self) -> &Path {
        &self.user_sources
    }

    /// User install directory (index + manifests).
    pub fn user_install_dir(&self) -> &Path {
        &self.user_install_dir
    }

    /// System sources list path.
    pub fn system_sources_path(&self) -> &Path {
        &self.system_sources
    }

    /// System install directory.
    pub fn system_install_dir(&self) -> &Path {
        &self.system_install_dir
    }
}

fn resolve_path(
    env_var: &str,
    xdg_default: Option<PathBuf>,
    fallback: &str,
) -> PathBuf {
    if let Ok(val) = std::env::var(env_var) {
        let trimmed = val.trim();
        if !trimmed.is_empty() {
            return expand_tilde(trimmed);
        }
    }
    xdg_default.unwrap_or_else(|| expand_tilde(fallback))
}

fn expand_tilde(path: &str) -> PathBuf {
    let expanded = shellexpand::tilde(path);
    PathBuf::from(expanded.as_ref())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expand_tilde_replaces_home() {
        let result = expand_tilde("~/foo/bar");
        let s = result.to_string_lossy();
        assert!(!s.starts_with('~'), "tilde should be expanded: {s}");
        assert!(s.ends_with("foo/bar"), "path suffix should be preserved: {s}");
    }

    #[test]
    fn expand_tilde_absolute_path_unchanged() {
        let result = expand_tilde("/absolute/path");
        assert_eq!(result, PathBuf::from("/absolute/path"));
    }

    #[test]
    fn expand_tilde_relative_path_unchanged() {
        let result = expand_tilde("relative/path");
        assert_eq!(result, PathBuf::from("relative/path"));
    }

    #[test]
    fn resolve_path_uses_env_var() {
        std::env::set_var("DMCP_TEST_RESOLVE_CUSTOM", "/custom/path");
        let result = resolve_path("DMCP_TEST_RESOLVE_CUSTOM", None, "/fallback");
        std::env::remove_var("DMCP_TEST_RESOLVE_CUSTOM");
        assert_eq!(result, PathBuf::from("/custom/path"));
    }

    #[test]
    fn resolve_path_whitespace_env_falls_back_to_xdg() {
        std::env::set_var("DMCP_TEST_RESOLVE_BLANK", "   ");
        let xdg = PathBuf::from("/xdg/path");
        let result = resolve_path("DMCP_TEST_RESOLVE_BLANK", Some(xdg.clone()), "/fallback");
        std::env::remove_var("DMCP_TEST_RESOLVE_BLANK");
        assert_eq!(result, xdg);
    }

    #[test]
    fn resolve_path_uses_xdg_when_env_absent() {
        std::env::remove_var("DMCP_TEST_RESOLVE_ABSENT");
        let xdg = PathBuf::from("/xdg/default");
        let result = resolve_path("DMCP_TEST_RESOLVE_ABSENT", Some(xdg.clone()), "/fallback");
        assert_eq!(result, xdg);
    }

    #[test]
    fn resolve_path_uses_fallback_when_no_env_and_no_xdg() {
        std::env::remove_var("DMCP_TEST_RESOLVE_NOENV");
        let result = resolve_path("DMCP_TEST_RESOLVE_NOENV", None, "/fallback/path");
        assert_eq!(result, PathBuf::from("/fallback/path"));
    }
}
