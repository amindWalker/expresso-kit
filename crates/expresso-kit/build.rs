//! # Build script for expresso-kit
//!
//! This script automatically sets up git hooks on first build,
//! ensuring code quality checks run before every push.

use std::{
    path::{Path, PathBuf},
    process::Command,
};

fn main() {
    // Only run in debug builds and when not in CI
    if std::env::var("PROFILE").unwrap_or_default() == "release" {
        return;
    }

    if std::env::var("CI").is_ok() {
        return;
    }

    setup_git_hooks();
}

fn setup_git_hooks() {
    // Get the workspace root (where Cargo.toml with [workspace] is)
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default();
    let workspace_root = PathBuf::from(&manifest_dir)
        .parent() // crates/
        .and_then(|p| p.parent()) // workspace root
        .map_or_else(|| PathBuf::from("."), Path::to_path_buf);

    // Check if we're in a git repository
    let git_dir = workspace_root.join(".git");
    if !git_dir.exists() {
        return;
    }

    // Check if hooks are already configured
    let output = Command::new("git")
        .current_dir(&workspace_root)
        .args(["config", "--local", "--get", "core.hooksPath"])
        .output();

    if let Ok(output) = output {
        let current = String::from_utf8_lossy(&output.stdout);
        if current.trim() == ".githooks" {
            // Already configured
            return;
        }
    }

    // Check if .githooks directory exists
    let hooks_dir = workspace_root.join(".githooks");
    if !hooks_dir.exists() {
        return;
    }

    // Configure git to use .githooks
    println!("cargo:warning=Setting up git hooks for code quality checks...");

    let result = Command::new("git")
        .current_dir(&workspace_root)
        .args(["config", "--local", "core.hooksPath", ".githooks"])
        .status();

    match result {
        Ok(status) if status.success() => {
            println!("cargo:warning=✓ Git hooks configured! Pre-push validation enabled.");
        }
        _ => {
            println!("cargo:warning=Could not configure git hooks automatically.");
            println!("cargo:warning=Run './.githooks/setup.sh' manually to enable.");
        }
    }

    // Make hooks executable (Unix only)
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        for hook in ["pre-commit", "pre-push", "setup.sh"] {
            let path = hooks_dir.join(hook);
            if path.exists()
                && let Ok(metadata) = std::fs::metadata(&path)
            {
                let mut perms = metadata.permissions();
                perms.set_mode(0o755);
                let _ = std::fs::set_permissions(&path, perms);
            }
        }
    }
}
