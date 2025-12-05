//! Project discovery module
//!
//! Provides functionality to discover projects with docker-compose or .env files
//! within a directory tree.

use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::Serialize;

use crate::platform::{self, file_patterns};

// =============================================================================
// Discovery Types
// =============================================================================

/// Information about a discovered project
#[derive(Debug, Clone, Serialize)]
pub struct ProjectInfo {
    /// Project name (usually directory name)
    pub name: String,
    /// Path to the project
    pub path: PathBuf,
    /// Has docker-compose file
    pub has_compose: bool,
    /// Has docker-compose sample file
    pub has_compose_sample: bool,
    /// Has .env file
    pub has_env: bool,
    /// Has .env.sample file
    pub has_env_sample: bool,
    /// Number of docker-compose services
    pub service_count: usize,
    /// Directory depth from search root
    pub depth: usize,
}

/// Options for project discovery
#[derive(Debug, Clone, Default)]
pub struct DiscoveryOptions {
    /// Maximum directory depth to search
    pub max_depth: usize,
    /// Only include projects with docker-compose
    pub require_compose: bool,
    /// Only include projects with .env files
    pub require_env: bool,
    /// Directories to skip during search
    pub skip_dirs: Vec<String>,
}

impl DiscoveryOptions {
    /// Create default discovery options
    pub fn new() -> Self {
        Self {
            max_depth: 3,
            require_compose: false,
            require_env: false,
            skip_dirs: vec![
                "node_modules".into(),
                "target".into(),
                ".git".into(),
                "vendor".into(),
                "dist".into(),
                "build".into(),
                "__pycache__".into(),
                ".venv".into(),
                "venv".into(),
            ],
        }
    }

    /// Set maximum search depth
    pub const fn with_max_depth(mut self, depth: usize) -> Self {
        self.max_depth = depth;
        self
    }

    /// Only find projects with docker-compose files
    pub const fn compose_only(mut self) -> Self {
        self.require_compose = true;
        self
    }

    /// Only find projects with .env files
    pub const fn env_only(mut self) -> Self {
        self.require_env = true;
        self
    }
}

/// Result of project discovery
#[derive(Debug, Clone, Serialize)]
pub struct DiscoveryResult {
    /// Root path that was searched
    pub root: PathBuf,
    /// Discovered projects
    pub projects: Vec<ProjectInfo>,
    /// Number of directories scanned
    pub directories_scanned: usize,
}

impl DiscoveryResult {
    /// Get projects with docker-compose files
    pub fn with_compose(&self) -> Vec<&ProjectInfo> {
        self.projects.iter().filter(|p| p.has_compose || p.has_compose_sample).collect()
    }

    /// Get projects with .env files
    pub fn with_env(&self) -> Vec<&ProjectInfo> {
        self.projects.iter().filter(|p| p.has_env || p.has_env_sample).collect()
    }

    /// Get projects that need setup (have samples but not actual files)
    pub fn needs_setup(&self) -> Vec<&ProjectInfo> {
        self.projects
            .iter()
            .filter(|p| (p.has_compose_sample && !p.has_compose) || (p.has_env_sample && !p.has_env))
            .collect()
    }
}

// =============================================================================
// Discovery Functions
// =============================================================================

/// Discover projects starting from a root path
pub fn discover_projects(root: &Path, options: &DiscoveryOptions) -> DiscoveryResult {
    let mut projects = Vec::new();
    let mut dirs_scanned = 0;

    discover_recursive(root, 0, options, &mut projects, &mut dirs_scanned);

    DiscoveryResult {
        root: root.to_path_buf(),
        projects,
        directories_scanned: dirs_scanned,
    }
}

/// Recursive directory discovery
fn discover_recursive(current: &Path, depth: usize, options: &DiscoveryOptions, projects: &mut Vec<ProjectInfo>, dirs_scanned: &mut usize) {
    if depth > options.max_depth {
        return;
    }

    *dirs_scanned += 1;

    // Check current directory for project markers
    if let Some(project) = analyze_directory(current, depth) {
        let include = (!options.require_compose || project.has_compose || project.has_compose_sample)
            && (!options.require_env || project.has_env || project.has_env_sample);

        let has_relevant_files = project.has_compose || project.has_compose_sample || project.has_env || project.has_env_sample;

        if include && has_relevant_files {
            projects.push(project);
        }
    }

    // Scan subdirectories
    let Ok(entries) = fs::read_dir(current) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let Some(dir_name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };

        // Skip hidden directories (except .github)
        if dir_name.starts_with('.') && dir_name != ".github" {
            continue;
        }

        // Skip configured directories
        if options.skip_dirs.iter().any(|skip| skip == dir_name) {
            continue;
        }

        discover_recursive(&path, depth + 1, options, projects, dirs_scanned);
    }
}

/// Analyze a directory for project markers
fn analyze_directory(path: &Path, depth: usize) -> Option<ProjectInfo> {
    let name = path.file_name()?.to_str()?.to_string();

    let has_compose = file_patterns::has_compose_file(path);
    let has_compose_sample = file_patterns::has_compose_sample_file(path);
    let has_env = file_patterns::has_env_file(path);
    let has_env_sample = file_patterns::has_env_sample_file(path);
    let service_count = if has_compose { count_compose_services(path) } else { 0 };

    Some(ProjectInfo {
        name,
        path: path.to_path_buf(),
        has_compose,
        has_compose_sample,
        has_env,
        has_env_sample,
        service_count,
        depth,
    })
}

/// Count services in a docker-compose file
fn count_compose_services(path: &Path) -> usize {
    if let Some(compose_path) = file_patterns::find_compose_file(path)
        && let Ok(content) = fs::read_to_string(&compose_path)
        && let Ok(yaml) = serde_yml::from_str::<serde_yml::Value>(&content)
        && let Some(services) = yaml.get("services").and_then(|s| s.as_mapping())
    {
        return services.len();
    }
    0
}

// =============================================================================
// Convenience Functions
// =============================================================================

/// Find projects with docker-compose files
pub fn find_compose_projects(root: &Path, max_depth: usize) -> Vec<ProjectInfo> {
    let options = DiscoveryOptions::new().with_max_depth(max_depth).compose_only();
    discover_projects(root, &options).projects
}

/// Find projects with .env files
pub fn find_env_projects(root: &Path, max_depth: usize) -> Vec<ProjectInfo> {
    let options = DiscoveryOptions::new().with_max_depth(max_depth).env_only();
    discover_projects(root, &options).projects
}

/// Find projects that need setup
pub fn find_projects_needing_setup(root: &Path, max_depth: usize) -> Vec<ProjectInfo> {
    let options = DiscoveryOptions::new().with_max_depth(max_depth);
    let result = discover_projects(root, &options);
    result.needs_setup().into_iter().cloned().collect()
}

/// Try to find a repository by name in common locations
pub fn discover_repo_by_name(repo_name: &str, base_path: &str, dir_name: &str) -> Option<(PathBuf, bool)> {
    let mut search_paths = Vec::new();

    let base = if base_path.is_empty() { "." } else { base_path };
    let dir = if dir_name.is_empty() { "projects" } else { dir_name };

    // Standard location: base/dir/repo
    let default_path = platform::normalize_path(&format!("{}/{}/{}", base, dir, repo_name));
    search_paths.push(default_path);

    // Flat structure: base/repo
    let flat_path = platform::normalize_path(&format!("{}/{}", base, repo_name));
    search_paths.push(flat_path);

    // Current directory: ./repo
    let current_dir_path = platform::normalize_path(&format!("./{}", repo_name));
    search_paths.push(current_dir_path);

    // Direct name as path
    let direct_path = platform::normalize_path(repo_name);
    search_paths.push(direct_path);

    // Search all paths
    for path in search_paths {
        if path.exists() && path.is_dir() {
            let is_cloned = path.join(".git").exists();
            return Some((path, is_cloned));
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_discovery_options_should_pass() {
        let opts = DiscoveryOptions::new().with_max_depth(5).compose_only();
        assert_eq!(opts.max_depth, 5);
        assert!(opts.require_compose);
        assert!(!opts.require_env);
    }

    #[test]
    fn test_discovery_result_filters_should_pass() {
        let result = DiscoveryResult {
            root: PathBuf::from("."),
            projects: vec![
                ProjectInfo {
                    name: "with-compose".into(),
                    path: PathBuf::from("./with-compose"),
                    has_compose: true,
                    has_compose_sample: false,
                    has_env: false,
                    has_env_sample: false,
                    service_count: 3,
                    depth: 1,
                },
                ProjectInfo {
                    name: "with-env".into(),
                    path: PathBuf::from("./with-env"),
                    has_compose: false,
                    has_compose_sample: false,
                    has_env: true,
                    has_env_sample: false,
                    service_count: 0,
                    depth: 1,
                },
            ],
            directories_scanned: 10,
        };

        assert_eq!(result.with_compose().len(), 1);
        assert_eq!(result.with_env().len(), 1);
        assert!(result.needs_setup().is_empty());
    }
}
