//! File pattern utilities for locating configuration files
//!
//! Common patterns for docker-compose, environment, and sample files

use std::path::{Path, PathBuf};

// =============================================================================
// Docker Compose Patterns
// =============================================================================

/// Standard docker-compose file patterns
pub const COMPOSE_PATTERNS: &[&str] = &["docker-compose.yml", "docker-compose.yaml", "compose.yml", "compose.yaml"];

/// Sample docker-compose file patterns
pub const COMPOSE_SAMPLE_PATTERNS: &[&str] = &[
    "docker-compose.sample.yml",
    "docker-compose.sample.yaml",
    "docker-compose.example.yml",
    "docker-compose.example.yaml",
    "docker-compose-sample.yml",
    "docker-compose-sample.yaml",
    "compose.sample.yml",
    "compose.sample.yaml",
];

// =============================================================================
// Environment File Patterns
// =============================================================================

/// Sample environment file patterns
pub const ENV_SAMPLE_PATTERNS: &[&str] = &[
    ".env.sample",
    ".env.example",
    ".env_sample",
    ".env_example",
    "env.sample",
    "env.example",
];

// =============================================================================
// Pattern Matching Functions
// =============================================================================

/// Find the first file matching any of the given patterns
pub fn find_matching_file(dir: &Path, patterns: &[&str]) -> Option<PathBuf> {
    patterns.iter().map(|p| dir.join(p)).find(|path| path.exists())
}

/// Find docker-compose file in directory
pub fn find_compose_file(dir: &Path) -> Option<PathBuf> {
    find_matching_file(dir, COMPOSE_PATTERNS)
}

/// Find sample docker-compose file in directory
pub fn find_compose_sample_file(dir: &Path) -> Option<PathBuf> {
    find_matching_file(dir, COMPOSE_SAMPLE_PATTERNS)
}

/// Find sample environment file in directory
pub fn find_env_sample_file(dir: &Path) -> Option<PathBuf> {
    find_matching_file(dir, ENV_SAMPLE_PATTERNS)
}

// =============================================================================
// Existence Checks
// =============================================================================

/// Check if directory has a docker-compose file
pub fn has_compose_file(dir: &Path) -> bool {
    find_compose_file(dir).is_some()
}

/// Check if directory has a sample docker-compose file
pub fn has_compose_sample_file(dir: &Path) -> bool {
    find_compose_sample_file(dir).is_some()
}

/// Check if directory has a .env file
pub fn has_env_file(dir: &Path) -> bool {
    dir.join(".env").exists()
}

/// Check if directory has a sample environment file
pub fn has_env_sample_file(dir: &Path) -> bool {
    find_env_sample_file(dir).is_some()
}

// =============================================================================
// Copy Operations
// =============================================================================

/// Copy a sample file to its target location
pub fn copy_sample_to_target(sample_path: &Path, target_path: &Path, file_type: &str) -> Result<PathBuf, String> {
    if target_path.exists() {
        return Err(format!("{} already exists", target_path.display()));
    }

    std::fs::copy(sample_path, target_path).map_err(|e| format!("Failed to copy {} sample: {}", file_type, e))?;

    Ok(target_path.to_path_buf())
}

/// Copy .env.sample or .env.example to .env
pub fn copy_env_sample(repo_path: &Path) -> Result<PathBuf, String> {
    let sample_path = find_env_sample_file(repo_path).ok_or_else(|| "No .env.sample or .env.example found".to_string())?;

    let target_path = repo_path.join(".env");

    copy_sample_to_target(&sample_path, &target_path, ".env")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compose_patterns_should_pass() {
        // Verify patterns contain expected docker-compose variants
        assert!(COMPOSE_PATTERNS.iter().any(|p| p.contains("docker-compose")));
    }

    #[test]
    fn test_env_sample_patterns_should_pass() {
        // Verify patterns contain expected .env variants
        assert!(ENV_SAMPLE_PATTERNS.iter().any(|p| p.contains(".env")));
    }
}
