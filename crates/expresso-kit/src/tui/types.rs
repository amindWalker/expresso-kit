//! TUI types
//!
//! Core type definitions for the TUI application.

use std::{
    collections::HashMap,
    path::PathBuf,
    time::{Duration, Instant},
};

use chrono::Local;
use ratatui::style::Color;

use crate::docker_compose::ServiceEnvConfig;
use super::icons;

// =============================================================================
// Repository Status
// =============================================================================

/// Status of a repository in the TUI
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum RepoStatus {
    #[default]
    Pending,
    Cloning(u8),
    Validating(u8),
    Ready,
    Error(String),
    Warning(String),
}

impl RepoStatus {
    pub fn color(&self) -> Color {
        match self {
            Self::Pending => Color::Gray,
            Self::Cloning(_) => Color::Yellow,
            Self::Validating(_) => Color::Cyan,
            Self::Ready => Color::Green,
            Self::Error(_) => Color::Red,
            Self::Warning(_) => Color::LightYellow,
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            Self::Pending => icons::PENDING,
            Self::Cloning(_) => icons::CLONING,
            Self::Validating(_) => icons::VALIDATING,
            Self::Ready => icons::READY,
            Self::Error(_) => icons::ERROR,
            Self::Warning(_) => icons::WARNING,
        }
    }

    pub fn progress(&self) -> Option<u8> {
        match self {
            Self::Cloning(p) | Self::Validating(p) => Some(*p),
            _ => None,
        }
    }
}

// =============================================================================
// Repository
// =============================================================================

/// Repository representation in the TUI
#[derive(Debug, Clone)]
pub struct Repository {
    pub id: usize,
    pub name: String,
    pub url: String,
    pub path: PathBuf,
    pub base_path: String,
    pub status: RepoStatus,
    pub last_updated: chrono::DateTime<Local>,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
    pub env_status: EnvStatus,
    pub selected: bool,
    pub required_files: Vec<String>,
    pub missing_files: Vec<String>,
    pub docker_compose_status: DockerComposeStatus,
}

impl Repository {
    pub fn new(id: usize, name: impl Into<String>, url: impl Into<String>, path: PathBuf, status: RepoStatus) -> Self {
        Self {
            id,
            name: name.into(),
            url: url.into(),
            path,
            base_path: ".".into(),
            status,
            last_updated: Local::now(),
            errors: vec![],
            warnings: vec![],
            env_status: EnvStatus::default(),
            selected: false,
            required_files: vec![".env".into()],
            missing_files: vec![],
            docker_compose_status: DockerComposeStatus::default(),
        }
    }

    pub fn with_base_path(mut self, base_path: impl Into<String>) -> Self {
        self.base_path = base_path.into();
        self
    }

    pub fn with_selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    pub fn with_required_files(mut self, files: Vec<String>) -> Self {
        self.required_files = files;
        self
    }

    pub fn is_cloned(&self) -> bool {
        self.path.join(".git").exists()
    }

    pub fn is_ready(&self) -> bool {
        matches!(self.status, RepoStatus::Ready)
    }

    pub fn total_issues(&self) -> usize {
        self.errors.len() + self.warnings.len() + self.missing_files.len() + self.env_status.issue_count()
    }

    pub fn has_issues(&self) -> bool {
        !self.errors.is_empty() || !self.warnings.is_empty() || !self.missing_files.is_empty() || self.env_status.has_issues()
    }
}

// =============================================================================
// Environment Status
// =============================================================================

/// Environment validation status for a repository
#[derive(Debug, Clone, Default)]
pub struct EnvStatus {
    pub missing_vars: Vec<String>,
    pub empty_vars: Vec<String>,
    pub mismatched_vars: Vec<(String, String)>,
    pub sample_file_exists: bool,
}

impl EnvStatus {
    pub fn has_issues(&self) -> bool {
        !self.missing_vars.is_empty() || !self.empty_vars.is_empty() || !self.mismatched_vars.is_empty()
    }

    pub fn issue_count(&self) -> usize {
        self.missing_vars.len() + self.empty_vars.len() + self.mismatched_vars.len()
    }
}

// =============================================================================
// Docker Compose Status
// =============================================================================

/// Docker Compose status for a repository
#[derive(Debug, Clone, Default)]
pub struct DockerComposeStatus {
    pub sample_exists: bool,
    pub compose_exists: bool,
    pub services: Vec<ServiceEnvConfig>,
    pub missing_env_vars: HashMap<String, Vec<String>>,
    pub has_issues: bool,
}

// =============================================================================
// Dashboard Tab
// =============================================================================

/// Dashboard tabs
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DashboardTab {
    Repositories,
    Environment,
    Workflows,
    Logs,
}

impl DashboardTab {
    pub const ALL: [Self; 4] = [Self::Repositories, Self::Environment, Self::Workflows, Self::Logs];

    pub fn index(self) -> usize {
        match self {
            Self::Repositories => 0,
            Self::Environment => 1,
            Self::Workflows => 2,
            Self::Logs => 3,
        }
    }

    pub fn next(self) -> Self {
        Self::ALL[(self.index() + 1) % Self::ALL.len()]
    }

    pub fn prev(self) -> Self {
        Self::ALL[(self.index() + Self::ALL.len() - 1) % Self::ALL.len()]
    }
}

// =============================================================================
// Dashboard Stats
// =============================================================================

/// Dashboard statistics
#[derive(Debug, Clone)]
pub struct DashboardStats {
    pub total_repos: usize,
    pub ready: usize,
    pub errors: usize,
    pub warnings: usize,
    pub selected: usize,
    pub last_refresh: Instant,
}

// =============================================================================
// Logging
// =============================================================================

/// Log level
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Info,
    Success,
    Warning,
    Error,
    Debug,
}

impl LogLevel {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Info => "INFO",
            Self::Success => "SUCCESS",
            Self::Warning => "WARN",
            Self::Error => "ERROR",
            Self::Debug => "DEBUG",
        }
    }

    pub const fn color(&self) -> Color {
        match self {
            Self::Info => Color::Cyan,
            Self::Success => Color::Green,
            Self::Warning => Color::Yellow,
            Self::Error => Color::Red,
            Self::Debug => Color::Gray,
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            Self::Info => icons::LOG_INFO,
            Self::Success => icons::LOG_SUCCESS,
            Self::Warning => icons::LOG_WARNING,
            Self::Error => icons::LOG_ERROR,
            Self::Debug => icons::LOG_DEBUG,
        }
    }
}

/// Log entry
#[derive(Debug, Clone)]
pub struct LogEntry {
    pub timestamp: chrono::DateTime<Local>,
    pub level: LogLevel,
    pub message: String,
    pub repo_name: Option<String>,
}

impl LogEntry {
    pub fn new(level: LogLevel, message: impl Into<String>) -> Self {
        Self {
            timestamp: Local::now(),
            level,
            message: message.into(),
            repo_name: None,
        }
    }
}

/// Toast notification
#[derive(Debug, Clone)]
pub struct Toast {
    pub level: LogLevel,
    pub message: String,
    pub created_at: Instant,
    pub duration: Duration,
}

impl Toast {
    pub fn new(level: LogLevel, message: String) -> Self {
        Self {
            level,
            message,
            created_at: Instant::now(),
            duration: Duration::from_secs(5),
        }
    }

    pub fn is_expired(&self) -> bool {
        self.created_at.elapsed() >= self.duration
    }

    pub fn remaining_secs(&self) -> u64 {
        self.duration.as_secs().saturating_sub(self.created_at.elapsed().as_secs())
    }
}

// =============================================================================
// Workflow Template
// =============================================================================

/// Workflow template types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkflowTemplate {
    Rust,
    NodeJs,
    Python,
    Go,
    Docker,
    DockerCompose,
    Custom,
}

impl WorkflowTemplate {
    pub const ALL: [Self; 7] = [
        Self::Rust,
        Self::NodeJs,
        Self::Python,
        Self::Go,
        Self::Docker,
        Self::DockerCompose,
        Self::Custom,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::Rust => "Rust",
            Self::NodeJs => "Node.js",
            Self::Python => "Python",
            Self::Go => "Go",
            Self::Docker => "Docker",
            Self::DockerCompose => "Docker Compose",
            Self::Custom => "Custom (empty)",
        }
    }

    #[allow(dead_code)]
    pub fn description(self) -> &'static str {
        match self {
            Self::Rust => "Build, test, clippy, fmt with stable/beta toolchains",
            Self::NodeJs => "Build, lint, test with Node 18/20/22",
            Self::Python => "Lint, test with Python 3.10/3.11/3.12",
            Self::Go => "Build, test, lint with Go 1.21/1.22/1.23",
            Self::Docker => "Build and test Docker image",
            Self::DockerCompose => "Validate, build, and test compose services",
            Self::Custom => "Start with empty workflow template",
        }
    }
}
