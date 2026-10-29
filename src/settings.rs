//! Shared settings (config/windups.toml), with sane defaults when no file exists.
use crate::paths::{expand, shared_config};
use serde::Deserialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub database: String,
    pub raw_dir: String,
    pub raw_max_age_days: u64,
    pub allow_remote_metadata: bool,
    /// Time-based unresolved reasons are retried after this many days (by a tidy-up windup).
    pub unresolved_retry_days: u64,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            database: "~/.local/share/windups/media.db".into(),
            raw_dir: "~/.local/share/windups/raw".into(),
            raw_max_age_days: 0,
            allow_remote_metadata: false,
            unresolved_retry_days: 7,
        }
    }
}

impl Settings {
    /// Load the shared file; a missing file means defaults (like a person with no notes yet).
    pub fn load() -> Result<Self, String> {
        Self::load_from(&shared_config())
    }

    pub fn load_from(path: &Path) -> Result<Self, String> {
        match std::fs::read_to_string(path) {
            Ok(s) => Self::parse(&s),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(format!("{}: {e}", path.display())),
        }
    }

    pub fn parse(s: &str) -> Result<Self, String> {
        toml::from_str(s).map_err(|e| e.to_string())
    }

    pub fn database_path(&self) -> PathBuf {
        expand(&self.database)
    }

    pub fn raw_path(&self) -> PathBuf {
        expand(&self.raw_dir)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn missing_file_gives_defaults() {
        let s = Settings::load_from(Path::new("/nonexistent/windups.toml")).unwrap();
        assert_eq!(s.raw_max_age_days, 0);
        assert!(!s.allow_remote_metadata);
    }
    #[test]
    fn shipped_config_parses() {
        let s = Settings::parse(include_str!("../config/windups.toml")).unwrap();
        assert!(s.database.ends_with("media.db"));
    }
    #[test]
    fn partial_file_keeps_defaults() {
        let s = Settings::parse("raw_max_age_days = 30").unwrap();
        assert_eq!(s.raw_max_age_days, 30);
        assert!(s.raw_dir.ends_with("raw"));
    }
}
