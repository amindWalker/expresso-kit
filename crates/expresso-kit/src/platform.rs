//! Platform-specific utilities and cross-platform abstractions
//!
//! This module provides platform detection, path normalization, and
//! system dependency checking for cross-platform compatibility.

use std::{path::PathBuf, process::Command};

// =============================================================================
// Git Executable Configuration
// =============================================================================

/// Get the git executable name for the current platform
#[cfg(target_os = "windows")]
pub const fn git_executable() -> &'static str {
    "git.exe"
}

/// Get the git executable name for Unix platforms
#[cfg(not(target_os = "windows"))]
pub const fn git_executable() -> &'static str {
    "/usr/bin/env"
}

/// Build git command arguments for Windows
#[cfg(target_os = "windows")]
pub fn git_args(args: &[&str]) -> Vec<String> {
    args.iter().map(ToString::to_string).collect()
}

/// Build git command arguments for Unix (prepend "git" to args)
#[cfg(not(target_os = "windows"))]
pub fn git_args(args: &[&str]) -> Vec<String> {
    std::iter::once("git".to_string())
        .chain(args.iter().copied().map(String::from))
        .collect()
}

/// Create a new git Command with platform-appropriate settings
pub fn new_git_command() -> Command {
    Command::new(git_executable())
}

// =============================================================================
// Path Normalization
// =============================================================================

/// Normalize a path string for Windows (convert forward slashes)
#[cfg(target_os = "windows")]
pub fn normalize_path(path: &str) -> PathBuf {
    PathBuf::from(path.replace('/', "\\"))
}

/// Normalize a path string for Unix (no conversion needed)
#[cfg(not(target_os = "windows"))]
pub fn normalize_path(path: &str) -> PathBuf {
    PathBuf::from(path)
}

// =============================================================================
// Dependency Checking
// =============================================================================

/// Check if git is available on the system
pub fn check_git_available() -> bool {
    let mut cmd = new_git_command();
    cmd.args(git_args(&["--version"]));
    cmd.output().map(|o| o.status.success()).unwrap_or(false)
}

/// Result of system dependency checks
#[derive(Debug, Clone, Default)]
pub struct DependencyCheck {
    /// Whether git is available
    pub git_available: bool,
    /// Whether we're in a git repository
    pub in_git_repo: bool,
}

impl DependencyCheck {
    /// Check if there are any critical missing dependencies
    pub fn has_errors(&self) -> bool {
        !self.git_available
    }
}

/// Check all system dependencies
pub fn check_dependencies() -> DependencyCheck {
    DependencyCheck {
        git_available: check_git_available(),
        in_git_repo: std::path::Path::new(".git").exists(),
    }
}

// =============================================================================
// File Patterns
// =============================================================================

pub mod file_patterns {
    //! Common file pattern matching utilities for docker-compose and env files

    use std::path::{Path, PathBuf};

    /// Standard docker-compose file patterns
    pub const COMPOSE_PATTERNS: &[&str] = &["docker-compose.yml", "docker-compose.yaml", "compose.yml", "compose.yaml"];

    /// Docker-compose sample file patterns
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

    /// Environment sample file patterns
    pub const ENV_SAMPLE_PATTERNS: &[&str] = &[
        ".env.sample",
        ".env.example",
        ".env_sample",
        ".env_example",
        "env.sample",
        "env.example",
    ];

    /// Find the first matching file from a list of patterns
    pub fn find_matching_file(dir: &Path, patterns: &[&str]) -> Option<PathBuf> {
        patterns.iter().map(|p| dir.join(p)).find(|path| path.exists())
    }

    /// Find docker-compose file in directory
    pub fn find_compose_file(dir: &Path) -> Option<PathBuf> {
        find_matching_file(dir, COMPOSE_PATTERNS)
    }

    /// Find docker-compose sample file in directory
    pub fn find_compose_sample_file(dir: &Path) -> Option<PathBuf> {
        find_matching_file(dir, COMPOSE_SAMPLE_PATTERNS)
    }

    /// Find environment sample file in directory
    pub fn find_env_sample_file(dir: &Path) -> Option<PathBuf> {
        find_matching_file(dir, ENV_SAMPLE_PATTERNS)
    }

    /// Check if directory has docker-compose file
    pub fn has_compose_file(dir: &Path) -> bool {
        find_compose_file(dir).is_some()
    }

    /// Check if directory has docker-compose sample file
    pub fn has_compose_sample_file(dir: &Path) -> bool {
        find_compose_sample_file(dir).is_some()
    }

    /// Check if directory has .env file
    pub fn has_env_file(dir: &Path) -> bool {
        dir.join(".env").exists()
    }

    /// Check if directory has env sample file
    pub fn has_env_sample_file(dir: &Path) -> bool {
        find_env_sample_file(dir).is_some()
    }

    /// Copy a sample file to its target location
    pub fn copy_sample_to_target(sample_path: &Path, target_path: &Path, file_type: &str) -> Result<PathBuf, String> {
        if target_path.exists() {
            return Err(format!("{} already exists", target_path.display()));
        }
        std::fs::copy(sample_path, target_path).map_err(|e| format!("Failed to copy {} sample: {}", file_type, e))?;
        Ok(target_path.to_path_buf())
    }

    /// Copy .env sample to .env
    pub fn copy_env_sample(repo_path: &Path) -> Result<PathBuf, String> {
        let sample_path = find_env_sample_file(repo_path).ok_or_else(|| "No .env.sample or .env.example found".to_string())?;
        let target_path = repo_path.join(".env");
        copy_sample_to_target(&sample_path, &target_path, ".env")
    }
}

// =============================================================================
// Signal Handling
// =============================================================================

pub mod signals {
    //! Cross-platform signal handling for graceful shutdown

    #[cfg(unix)]
    use std::sync::atomic::Ordering;
    use std::sync::{Arc, atomic::AtomicBool};

    #[cfg(unix)]
    use signal_hook::consts::signal::{SIGHUP, SIGINT, SIGTERM};
    #[cfg(unix)]
    use signal_hook::iterator::Signals;

    /// Set up signal handlers for graceful shutdown (Unix)
    #[cfg(unix)]
    pub fn setup_signal_handler(should_quit: Arc<AtomicBool>) -> std::io::Result<()> {
        let mut signals = Signals::new([SIGINT, SIGTERM, SIGHUP])?;
        std::thread::spawn(move || {
            for sig in signals.forever() {
                if matches!(sig, SIGINT | SIGTERM | SIGHUP) {
                    should_quit.store(true, Ordering::SeqCst);
                    break;
                }
            }
        });
        Ok(())
    }

    /// Set up signal handlers for graceful shutdown (Windows - no-op)
    #[cfg(windows)]
    #[allow(clippy::needless_pass_by_value)]
    pub fn setup_signal_handler(should_quit: Arc<AtomicBool>) -> std::io::Result<()> {
        let _ = should_quit;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_git_available_should_pass() {
        // This test verifies the function runs without panicking
        // The result depends on the environment (git may or may not be installed)
        let _available = check_git_available();
    }

    #[test]
    fn test_normalize_path_should_pass() {
        let path = normalize_path("foo/bar/baz");
        assert!(path.components().count() >= 1);
    }
}
