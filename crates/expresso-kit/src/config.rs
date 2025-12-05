//! Application configuration management
//!
//! Handles loading and saving of application state to `.expresso-kit.toml`.

use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

/// Configuration filename
const CONFIG_FILENAME: &str = ".expresso-kit.toml";

/// Repository configuration for persistence
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoConfig {
    /// Repository name
    pub name: String,
    /// Git URL
    pub url: String,
    /// Local filesystem path
    pub path: String,
    /// Base path override for this repo
    #[serde(default)]
    pub base_path: Option<String>,
    /// Whether the repo is selected
    pub selected: bool,
    /// Clone/validation progress (0-100)
    #[serde(default)]
    pub progress: u8,
    /// Required files for this repo
    #[serde(default)]
    pub required_files: Vec<String>,
}

/// Clone operation settings
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CloneSettings {
    /// Subdirectory name for cloned repos
    pub dir_name: String,
    /// Base path for cloning
    #[serde(default)]
    pub base_path: String,
    /// Historical base paths used
    #[serde(default)]
    pub base_paths: Vec<String>,
}

/// Main application configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppConfig {
    /// Configuration format version
    pub version: String,
    /// Clone operation settings
    pub clone_settings: CloneSettings,
    /// Tracked repositories
    pub repositories: Vec<RepoConfig>,
    /// Global required files (apply to all repos)
    #[serde(default)]
    pub required_files: Vec<String>,
}

impl AppConfig {
    /// Create a new configuration with defaults
    pub fn new() -> Self {
        Self {
            version: env!("CARGO_PKG_VERSION").to_string(),
            clone_settings: CloneSettings {
                dir_name: "projects".to_string(),
                base_path: ".".to_string(),
                base_paths: vec![".".to_string()],
            },
            repositories: Vec::new(),
            required_files: vec![".env".to_string()],
        }
    }

    /// Get the default config file path
    pub fn config_path() -> PathBuf {
        PathBuf::from(CONFIG_FILENAME)
    }

    /// Get the config file path within a specific directory
    pub fn config_path_in(dir: &Path) -> PathBuf {
        dir.join(CONFIG_FILENAME)
    }

    /// Load configuration from the default location
    pub fn load() -> Result<Self, String> {
        Self::load_from(&Self::config_path())
    }

    /// Load configuration from a specific path
    pub fn load_from(path: &Path) -> Result<Self, String> {
        if !path.exists() {
            return Err(format!("Config file not found: {}", path.display()));
        }
        let content = fs::read_to_string(path).map_err(|e| format!("Failed to read config: {}", e))?;
        toml::from_str(&content).map_err(|e| format!("Failed to parse config: {}", e))
    }

    /// Save configuration to the default location
    pub fn save(&self) -> Result<(), String> {
        self.save_to(&Self::config_path())
    }

    /// Save configuration to a specific path
    pub fn save_to(&self, path: &Path) -> Result<(), String> {
        let content = toml::to_string_pretty(self).map_err(|e| format!("Failed to serialize config: {}", e))?;
        fs::write(path, content).map_err(|e| format!("Failed to write config: {}", e))
    }

    /// Check if a config file exists
    pub fn exists() -> bool {
        Self::config_path().exists()
    }

    /// Merge another config into this one (for repo discovery)
    pub fn merge_repositories(&mut self, other: &[RepoConfig]) {
        for repo in other {
            if !self.repositories.iter().any(|r| r.url == repo.url) {
                self.repositories.push(repo.clone());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_config_should_pass() {
        let config = AppConfig::new();
        assert!(!config.version.is_empty());
        assert_eq!(config.clone_settings.dir_name, "projects");
        assert!(config.repositories.is_empty());
    }

    #[test]
    fn test_config_serialization_should_pass() {
        let config = AppConfig::new();
        let serialized = toml::to_string_pretty(&config).expect("Should serialize");
        let deserialized: AppConfig = toml::from_str(&serialized).expect("Should deserialize");
        assert_eq!(config.version, deserialized.version);
    }
}
