//! Expresso-Kit Library
//!
//! A TUI + CLI for validating git repositories with docker-compose support.
//!
//! # Modules
//!
//! - `cli` - Command-line interface definitions and handlers
//! - `config` - Application configuration persistence
//! - `discovery` - Project discovery utilities
//! - `docker_compose` - Docker Compose parsing and validation
//! - `file_patterns` - File pattern matching for compose/env files
//! - `git_ops` - Git operations (clone, URL parsing)
//! - `platform` - Cross-platform utilities
//! - `signals` - Signal handling for graceful shutdown
//! - `tui` - Terminal user interface components
//! - `validation` - Repository validation
//! - `workflow_templates` - GitHub Actions workflow generation

pub mod cli;
pub mod config;
pub mod discovery;
pub mod docker_compose;
pub mod file_patterns;
pub mod git_ops;
pub mod platform;
pub mod signals;
pub mod tui;
pub mod validation;
pub mod workflow_templates;

// Re-exports for convenience
pub use cli::{Cli, Commands, OutputFormat, ValidationResult, run as cli_run, should_run_tui};
pub use config::{AppConfig, CloneSettings, RepoConfig};
pub use discovery::{DiscoveryOptions, DiscoveryResult, ProjectInfo, discover_projects, discover_repo_by_name};
pub use docker_compose::{
    DockerComposeValidation, ServiceComparison, ServiceEnvConfig, compare_compose_services, copy_compose_sample, extract_env_references,
    migrate_env_to_file, parse_compose_file, restore_env_from_file, update_service_environment, validate_docker_compose,
};
pub use file_patterns::{
    copy_env_sample, copy_sample_to_target, find_compose_file, find_compose_sample_file, find_env_sample_file, has_compose_file,
    has_compose_sample_file, has_env_file, has_env_sample_file,
};
pub use git_ops::{CloneProgress, CloneResult, clone_repository_with_progress, extract_repo_name, parse_git_url};
pub use platform::{DependencyCheck, check_dependencies, check_git_available};
pub use signals::setup_signal_handler;
pub use tui::{
    CloneConfig, ConfirmDialog, DashboardStats, DashboardTab, DockerComposePopupState, DockerComposeStatus, EnvStatus, ErrorPopup,
    HelpPopup, InputMode, InputState, LogEntry, LogLevel, LogViewer, ProgressGauges, RepoStatus, RepoTable, Repository, StatsPanel, Toast,
    WorkflowPopupState, WorkflowTemplate, centered_popup, make_progress_bar,
};
pub use validation::{
    DEFAULT_REQUIRED_FILES, EnvValidationResult, RepositoryValidation, RequiredFilesResult, ValidationOptions, validate_repository,
    validate_repository_env, validate_repository_with_options, validate_required_files,
};
pub use workflow_templates::{DetectedProjects, detect_project_types, generate_workflow};
