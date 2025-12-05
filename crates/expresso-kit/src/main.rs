//! Expresso-Kit: TUI + CLI for validating git repositories with docker-compose support
use std::{
    collections::VecDeque,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use anyhow::Result;
use chrono::Local;
use clap::Parser;
use expresso_kit::{
    Cli,
    DetectedProjects,
    ServiceEnvConfig,
    // CLI functions
    cli_run,
    config,
    // File patterns
    copy_env_sample,
    detect_project_types,
    // Discovery, Config & Validation
    discovery,
    // Docker
    docker_compose,
    find_env_sample_file,
    git_ops,
    // Platform, Signals & Git
    platform,
    should_run_tui,
    signals,
    tui::{
        CloneConfig, ConfirmDialog, DashboardStats, DashboardTab, DockerComposePopupState, DockerComposeStatus, EnvStatus, ErrorPopup,
        HelpPopup, InputMode, InputState, LogEntry, LogLevel, LogViewer, ProgressGauges, RepoStatus, RepoTable, Repository, StatsPanel,
        Toast, WorkflowPopupState, WorkflowTemplate, centered_popup,
    },
    validation,
    workflow_templates,
};
use parking_lot::{Mutex, RwLock};
use ratatui::{
    crossterm::{
        cursor::MoveTo,
        event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
        execute,
        terminal::{Clear as TermClear, ClearType, EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
    },
    prelude::Widget,
};
pub mod shared_modules {
    pub use expresso_kit::{
        config, discovery, docker_compose, file_patterns, git_ops, platform, signals, validation as env_validation, validation,
        validation as required_files, workflow_templates,
    };
}
use ratatui::{
    Frame, Terminal,
    backend::{Backend, CrosstermBackend},
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, TableState, Tabs, Wrap},
};
use tokio::{
    sync::mpsc::{self, UnboundedReceiver, UnboundedSender},
    task::JoinHandle,
};
pub struct App {
    repos: Arc<RwLock<Vec<Repository>>>,
    repo_table_state: TableState,
    next_repo_id: usize,
    current_tab: DashboardTab,
    dashboard_stats: DashboardStats,
    last_update: chrono::DateTime<Local>,
    last_status_check: Instant,
    show_confirm_dialog: bool,
    confirm_dialog_expanded: bool,
    confirm_dialog_scroll: usize,
    confirm_action: Option<String>,
    show_error_popup: bool,
    error_message: String,
    show_help: bool,
    show_save_dialog: bool,
    toasts: VecDeque<Toast>,
    input_mode: InputMode,
    input_state: InputState,
    editing_repo_id: Option<usize>,
    clone_config: CloneConfig,
    docker_compose_state: DockerComposePopupState,
    workflow_state: WorkflowPopupState,
    logs: Arc<Mutex<VecDeque<LogEntry>>>,
    max_logs: usize,
    clone_progress_tx: UnboundedSender<(usize, git_ops::CloneProgress)>,
    #[allow(clippy::type_complexity)]
    clone_progress_rx: Arc<Mutex<Option<UnboundedReceiver<(usize, git_ops::CloneProgress)>>>>,
    should_quit: Arc<AtomicBool>,
    bg_tasks: Vec<JoinHandle<()>>,
    consecutive_refresh_count: usize,
    refresh_paused: bool,
}
impl App {
    pub fn new() -> Result<Self> {
        let saved_config = config::AppConfig::load().ok();
        let clone_config = CloneConfig {
            dir_name: saved_config
                .as_ref()
                .map_or("projects", |c| &c.clone_settings.dir_name)
                .to_string(),
            base_path: saved_config.as_ref().map_or(".", |c| &c.clone_settings.base_path).to_string(),
            ..Default::default()
        };
        let global_required_files: Vec<String> = saved_config
            .as_ref()
            .map_or_else(|| vec![".env".into()], |c| c.required_files.clone());
        let mut repos: Vec<Repository> = Vec::new();
        let mut next_repo_id = 1usize;
        let mut discovered_count = 0;
        if let Some(ref cfg) = saved_config {
            for repo_cfg in &cfg.repositories {
                let saved_path = PathBuf::from(&repo_cfg.path);
                let (path, status) = if saved_path.join(".git").exists() {
                    (saved_path, RepoStatus::Ready)
                } else if saved_path.exists() && saved_path.is_dir() {
                    (saved_path, RepoStatus::Pending)
                } else if let Some((found_path, is_cloned)) =
                    discovery::discover_repo_by_name(&repo_cfg.name, &cfg.clone_settings.base_path, &cfg.clone_settings.dir_name)
                {
                    discovered_count += 1;
                    let status = if is_cloned { RepoStatus::Ready } else { RepoStatus::Pending };
                    (found_path, status)
                } else {
                    (saved_path, RepoStatus::Pending)
                };
                let mut required_files = global_required_files.clone();
                for f in &repo_cfg.required_files {
                    if !required_files.contains(f) {
                        required_files.push(f.clone());
                    }
                }
                repos.push(
                    Repository::new(next_repo_id, &repo_cfg.name, &repo_cfg.url, path, status)
                        .with_base_path(repo_cfg.base_path.as_deref().unwrap_or("."))
                        .with_selected(repo_cfg.selected)
                        .with_required_files(required_files),
                );
                next_repo_id += 1;
            }
        }
        let stats = DashboardStats {
            total_repos: repos.len(),
            ready: repos.iter().filter(|r| r.is_ready() && !r.has_issues()).count(),
            errors: 0,
            warnings: 0,
            selected: repos.iter().filter(|r| r.selected).count(),
            last_refresh: Instant::now(),
        };
        let mut logs: VecDeque<LogEntry> = [LogEntry::new(LogLevel::Info, "Expresso-Kit started")].into();
        if saved_config.is_some() {
            logs.push_back(LogEntry::new(
                LogLevel::Success,
                format!("Loaded configuration with {} repository(ies)", repos.len()),
            ));
            if discovered_count > 0 {
                logs.push_back(LogEntry::new(
                    LogLevel::Info,
                    format!("Auto-discovered {} repo(s) at different location(s)", discovered_count),
                ));
            }
        }
        let deps = platform::check_dependencies();
        logs.push_back(LogEntry::new(
            if deps.git_available { LogLevel::Success } else { LogLevel::Error },
            if deps.git_available {
                "Git is available on this system"
            } else {
                "Git not found! Please install git to clone repositories"
            },
        ));
        logs.push_back(LogEntry::new(LogLevel::Info, "Press 'a' to add repositories, 'h' for help"));
        let (clone_progress_tx, clone_progress_rx) = mpsc::unbounded_channel();
        Ok(Self {
            repos: Arc::new(RwLock::new(repos)),
            repo_table_state: TableState::default(),
            next_repo_id,
            current_tab: DashboardTab::Repositories,
            dashboard_stats: stats,
            last_update: Local::now(),
            last_status_check: Instant::now(),
            show_confirm_dialog: false,
            confirm_dialog_expanded: false,
            confirm_dialog_scroll: 0,
            confirm_action: None,
            show_error_popup: false,
            error_message: String::new(),
            show_help: false,
            show_save_dialog: false,
            toasts: VecDeque::new(),
            input_mode: InputMode::Normal,
            input_state: InputState::new("Enter repository URL (e.g., https://github.com/user/repo.git)"),
            editing_repo_id: None,
            clone_config,
            docker_compose_state: DockerComposePopupState::default(),
            workflow_state: WorkflowPopupState::default(),
            logs: Arc::new(Mutex::new(logs)),
            max_logs: 1000,
            clone_progress_tx,
            clone_progress_rx: Arc::new(Mutex::new(Some(clone_progress_rx))),
            should_quit: Arc::new(AtomicBool::new(false)),
            bg_tasks: Vec::new(),
            consecutive_refresh_count: 0,
            refresh_paused: false,
        })
    }
    pub fn save_config(&mut self) {
        let repos = self.repos.read();
        let repo_configs: Vec<config::RepoConfig> = repos
            .iter()
            .map(|r| {
                let progress = r
                    .status
                    .progress()
                    .unwrap_or_else(|| if r.is_cloned() || r.is_ready() { 100 } else { 0 });
                config::RepoConfig {
                    name: r.name.clone(),
                    url: r.url.clone(),
                    path: r.path.to_string_lossy().to_string(),
                    base_path: Some(r.base_path.clone()),
                    selected: r.selected,
                    progress,
                    required_files: r.required_files.clone(),
                }
            })
            .collect();
        drop(repos);
        let app_config = config::AppConfig {
            version: env!("CARGO_PKG_VERSION").to_string(),
            clone_settings: config::CloneSettings {
                dir_name: self.clone_config.dir_name.clone(),
                base_path: self.clone_config.base_path.clone(),
                base_paths: vec![self.clone_config.base_path.clone()],
            },
            repositories: repo_configs,
            required_files: vec![".env".to_string()], // Global required files
        };
        match app_config.save() {
            Ok(()) => {
                self.add_log(LogLevel::Success, "Configuration saved to .expresso-kit.toml".to_string(), None);
            }
            Err(e) => {
                self.add_log(LogLevel::Error, format!("Failed to save configuration: {}", e), None);
            }
        }
    }
    fn discover_repo_on_filesystem(&self, repo_name: &str) -> Option<(PathBuf, bool)> {
        discovery::discover_repo_by_name(repo_name, &self.clone_config.base_path, &self.clone_config.dir_name)
    }
    pub fn add_repositories_from_urls(&mut self, urls: Vec<String>) {
        let mut repos = self.repos.write();
        for url in urls {
            let trimmed = url.trim();
            if trimmed.is_empty() {
                continue;
            }
            let parsed_url = match git_ops::parse_git_url(trimmed) {
                Ok(url) => url,
                Err(error) => {
                    drop(repos);
                    self.add_log(LogLevel::Error, error, None);
                    repos = self.repos.write();
                    continue;
                }
            };
            if repos.iter().any(|r| r.url == parsed_url) {
                drop(repos);
                self.add_log(LogLevel::Warning, format!("Repository already added: {}", parsed_url), None);
                repos = self.repos.write();
                continue;
            }
            let name = git_ops::extract_repo_name(&parsed_url);
            drop(repos); // Release lock before filesystem operations
            let (path, status) = if let Some((found_path, is_cloned)) = self.discover_repo_on_filesystem(&name) {
                let status = if is_cloned {
                    self.add_log(LogLevel::Success, format!("Discovered existing repo '{}' at: {}", name, found_path.display()), None);
                    RepoStatus::Ready
                } else {
                    self.add_log(
                        LogLevel::Warning,
                        format!("Found directory '{}' but not a git repo: {}", name, found_path.display()),
                        None,
                    );
                    RepoStatus::Pending
                };
                (found_path, status)
            } else {
                let default_path = platform::normalize_path(&format!(
                    "{}/{}/{}",
                    if self.clone_config.base_path.is_empty() {
                        "."
                    } else {
                        &self.clone_config.base_path
                    },
                    if self.clone_config.dir_name.is_empty() {
                        "projects"
                    } else {
                        &self.clone_config.dir_name
                    },
                    name
                ));
                (default_path, RepoStatus::Pending)
            };
            repos = self.repos.write();
            let repo = Repository::new(self.next_repo_id, name, parsed_url, path, status).with_base_path(&self.clone_config.base_path);
            self.next_repo_id += 1;
            let repo_name = repo.name.clone();
            let repo_id = repo.id;
            repos.push(repo);
            drop(repos);
            self.add_log(LogLevel::Info, format!("Added repository: {}", repo_name), Some(repo_id));
            repos = self.repos.write();
        }
        drop(repos);
        self.update_stats();
        self.refresh_repository_status(); // Ensure icons are updated based on filesystem state
        self.record_operation(); // Track this operation for "Last Update"
        if self.repo_table_state.selected().is_none() {
            let repos = self.repos.read();
            if !repos.is_empty() {
                drop(repos);
                self.repo_table_state.select(Some(0));
            }
        }
    }
    fn clone_selected_repositories(&mut self) {
        let repos_to_clone: Vec<(usize, String, PathBuf)> = {
            let repos = self.repos.read();
            repos
                .iter()
                .filter(|r| r.selected || repos.len() == 1) // Clone if selected, or if only one repo
                .map(|r| (r.id, r.url.clone(), r.path.clone()))
                .collect()
        };
        if repos_to_clone.is_empty() {
            self.add_log(LogLevel::Warning, "No repositories selected for cloning".to_string(), None);
            return;
        }
        self.add_log(LogLevel::Info, format!("Starting clone of {} repository(ies)...", repos_to_clone.len()), None);
        for (id, url, path) in repos_to_clone {
            {
                let mut repos = self.repos.write();
                if let Some(repo) = repos.iter_mut().find(|r| r.id == id) {
                    repo.status = RepoStatus::Cloning(0);
                    repo.last_updated = Local::now();
                }
            }
            self.add_log(LogLevel::Info, format!("Cloning {}...", url), Some(id));
            git_ops::clone_repository_with_progress(url, path, self.clone_progress_tx.clone(), id);
        }
        self.update_stats();
        self.record_operation(); // Track this operation for "Last Update"
    }
    fn validate_repository_environment(&mut self, repo_id: usize) {
        self.validate_repository_environment_internal(repo_id, true);
    }
    fn validate_repository_environment_silent(&mut self, repo_id: usize) {
        self.validate_repository_environment_internal(repo_id, false);
    }
    fn validate_repository_environment_internal(&mut self, repo_id: usize, show_toast: bool) {
        let (repo_path, required_files_list) = {
            let repos = self.repos.read();
            repos
                .iter()
                .find(|r| r.id == repo_id)
                .map(|r| (r.path.clone(), r.required_files.clone()))
        }
        .unzip();
        let Some(path) = repo_path else {
            self.add_log_internal(LogLevel::Error, format!("Repository {} not found", repo_id), None, show_toast);
            return;
        };
        let required_files_list = required_files_list.unwrap_or_default();
        self.add_log_internal(LogLevel::Info, "Validating repository...".into(), Some(repo_id), show_toast);
        let v = validation::validate_repository(&path, &required_files_list);
        {
            let mut repos = self.repos.write();
            if let Some(repo) = repos.iter_mut().find(|r| r.id == repo_id) {
                repo.env_status.sample_file_exists = v.env.sample_exists;
                repo.env_status.missing_vars.clone_from(&v.env.missing_vars);
                repo.env_status.empty_vars.clone_from(&v.env.empty_vars);
                repo.missing_files.clone_from(&v.files.missing);
                repo.docker_compose_status.sample_exists = v.compose.sample_exists;
                repo.docker_compose_status.compose_exists = v.compose.compose_exists;
                // Convert from local docker_compose::ServiceEnvConfig to library's ServiceEnvConfig
                repo.docker_compose_status.services = v
                    .compose
                    .services
                    .iter()
                    .map(|s| ServiceEnvConfig {
                        name: s.name.clone(),
                        environment: s.environment.clone(),
                        env_file: s.env_file.clone(),
                        has_environment: s.has_environment,
                        has_env_file: s.has_env_file,
                    })
                    .collect();
                repo.docker_compose_status.missing_env_vars.clone_from(&v.compose_missing_env);
                repo.docker_compose_status.has_issues = v.compose.sample_exists && !v.compose.compose_exists;
                repo.errors.clear();
                repo.warnings.clear();
                repo.errors.extend(v.errors.clone());
                repo.warnings.extend(v.warnings.clone());
                if matches!(repo.status, RepoStatus::Validating(_)) {
                    repo.status = if v.has_errors() {
                        RepoStatus::Error("Validation failed".to_string())
                    } else {
                        RepoStatus::Ready
                    };
                }
                repo.last_updated = Local::now();
            }
        }
        self.log_validation_results(&v, repo_id, show_toast);
        self.update_stats();
    }
    fn log_validation_results(&mut self, v: &validation::RepositoryValidation, repo_id: usize, show_toast: bool) {
        match (v.env.sample_exists, v.env.missing_vars.is_empty() && v.env.empty_vars.is_empty()) {
            (false, _) => self.add_log_internal(
                LogLevel::Warning,
                "No .env.sample file found - skipping env validation".into(),
                Some(repo_id),
                show_toast,
            ),
            (true, true) => self.add_log_internal(LogLevel::Success, "Environment validation passed!".into(), Some(repo_id), show_toast),
            (true, false) => {
                if !v.env.missing_vars.is_empty() {
                    self.add_log_internal(
                        LogLevel::Error,
                        format!("Missing {} environment variable(s)", v.env.missing_vars.len()),
                        Some(repo_id),
                        show_toast,
                    );
                }
                if !v.env.empty_vars.is_empty() {
                    self.add_log_internal(
                        LogLevel::Warning,
                        format!("{} environment variable(s) are empty", v.env.empty_vars.len()),
                        Some(repo_id),
                        show_toast,
                    );
                }
            }
        }
        if !v.files.missing.is_empty() {
            self.add_log_internal(
                LogLevel::Error,
                format!("Missing {} required file(s): {}", v.files.missing.len(), v.files.missing.join(", ")),
                Some(repo_id),
                show_toast,
            );
        } else if !v.files.required.is_empty() {
            self.add_log_internal(LogLevel::Success, "All required files present".into(), Some(repo_id), show_toast);
        }
        if v.compose.sample_exists {
            match (v.compose.compose_exists, v.compose_missing_env.is_empty()) {
                (false, _) => self.add_log_internal(
                    LogLevel::Error,
                    "docker-compose.yml missing (sample file exists)".into(),
                    Some(repo_id),
                    show_toast,
                ),
                (true, true) => {
                    self.add_log_internal(LogLevel::Success, "docker-compose validation passed!".into(), Some(repo_id), show_toast);
                }
                (true, false) => {
                    let total: usize = v.compose_missing_env.values().map(Vec::len).sum();
                    if total > 0 {
                        self.add_log_internal(
                            LogLevel::Warning,
                            format!("{total} missing env var(s) across docker-compose services"),
                            Some(repo_id),
                            show_toast,
                        );
                    }
                }
            }
        }
    }
    fn refresh_repository_statuses(&mut self) {
        let repo_ids_to_refresh: Vec<usize> = {
            let repos = self.repos.read();
            repos
                .iter()
                .filter(|r| matches!(r.status, RepoStatus::Ready | RepoStatus::Warning(_) | RepoStatus::Error(_)) && r.path.exists())
                .map(|r| r.id)
                .collect()
        };
        for id in repo_ids_to_refresh {
            self.validate_repository_environment_silent(id);
        }
        self.last_status_check = Instant::now();
    }
    fn copy_env_sample_to_env(&mut self, repo_index: usize) {
        let Some((repo_path, repo_id, repo_name)) = ({
            let repos = self.repos.read();
            repos.get(repo_index).map(|r| (r.path.clone(), r.id, r.name.clone()))
        }) else {
            self.add_log(LogLevel::Error, "Repository not found".to_string(), None);
            return;
        };
        if !repo_path.exists() {
            self.add_log(LogLevel::Warning, format!("Repository '{}' not cloned yet", repo_name), Some(repo_id));
            return;
        }
        match copy_env_sample(&repo_path) {
            Ok(target_path) => {
                let sample_name = find_env_sample_file(&repo_path)
                    .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
                    .unwrap_or_else(|| ".env.sample".to_string());
                self.add_log(
                    LogLevel::Success,
                    format!(
                        "Copied {} to {} for '{}'",
                        sample_name,
                        target_path.file_name().unwrap_or_default().to_string_lossy(),
                        repo_name
                    ),
                    Some(repo_id),
                );
                self.validate_repository_environment(repo_id);
                self.record_operation();
            }
            Err(e) => {
                if e.contains("already exists") {
                    self.add_log(
                        LogLevel::Warning,
                        format!(".env already exists for '{}'. Use 'e' to edit it.", repo_name),
                        Some(repo_id),
                    );
                } else if e.contains("No .env") {
                    self.add_log(LogLevel::Warning, format!("No .env.sample found for '{}'", repo_name), Some(repo_id));
                } else {
                    self.add_log(LogLevel::Error, format!("Failed to copy to .env: {}", e), Some(repo_id));
                }
            }
        }
    }
    fn save_env_vars_to_file(&mut self) {
        let Some(selected) = self.repo_table_state.selected() else {
            self.add_log(LogLevel::Error, "No repository selected".to_string(), None);
            return;
        };
        let Some((repo_path, repo_id, repo_name)) = ({
            let repos = self.repos.read();
            repos.get(selected).map(|r| (r.path.clone(), r.id, r.name.clone()))
        }) else {
            self.add_log(LogLevel::Error, "Repository not found".to_string(), None);
            return;
        };
        if !repo_path.exists() {
            self.add_log(LogLevel::Error, format!("Repository path does not exist: {}", repo_path.display()), Some(repo_id));
            return;
        }
        match std::fs::write(repo_path.join(".env"), &self.input_state.buffer) {
            Ok(_) => {
                self.add_log(LogLevel::Success, format!("Saved .env file for '{}'", repo_name), Some(repo_id));
                self.validate_repository_environment(repo_id);
                self.record_operation();
            }
            Err(e) => self.add_log(LogLevel::Error, format!("Failed to save .env: {}", e), Some(repo_id)),
        }
    }
    pub fn add_log(&mut self, level: LogLevel, message: String, repo_id: Option<usize>) {
        self.add_log_internal(level, message, repo_id, true);
    }
    fn add_log_internal(&mut self, level: LogLevel, message: String, repo_id: Option<usize>, show_toast: bool) {
        let repo_name = repo_id.and_then(|id| {
            let repos = self.repos.read();
            repos.iter().find(|r| r.id == id).map(|r| r.name.clone())
        });
        if show_toast && level != LogLevel::Debug {
            let toast_msg = repo_name
                .as_ref()
                .map_or_else(|| message.clone(), |name| format!("[{}] {}", name, message));
            let grouping_threshold = Duration::from_secs(1);
            let mut grouped = false;
            for toast in &mut self.toasts {
                if toast.level == level && toast.created_at.elapsed() < grouping_threshold {
                    toast.message = format!("{}\n{}", toast.message, toast_msg);
                    toast.created_at = Instant::now();
                    grouped = true;
                    break;
                }
            }
            if !grouped {
                self.toasts.push_back(Toast::new(level, toast_msg));
                while self.toasts.len() > 5 {
                    self.toasts.pop_front();
                }
            }
        }
        let entry = LogEntry { timestamp: Local::now(), level, message, repo_name };
        let mut logs = self.logs.lock();
        logs.push_back(entry);
        while logs.len() > self.max_logs {
            if let Some(old_entry) = logs.pop_front() {
                self.save_log_to_file(&old_entry);
            }
        }
    }
    fn save_log_to_file(&self, entry: &LogEntry) {
        let log_path = std::env::current_dir().unwrap_or_default().join("expresso-kit.log");
        let repo_str = entry.repo_name.as_deref().map_or(String::new(), |n| format!(" [{}]", n));
        let log_line = format!(
            "[{}] [{}]{} {}\n",
            entry.timestamp.format("%Y-%m-%d %H:%M:%S"),
            entry.level.as_str(),
            repo_str,
            entry.message
        );
        let _ = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)
            .and_then(|mut f| std::io::Write::write_all(&mut f, log_line.as_bytes()));
    }
    fn update_stats(&mut self) {
        let repos = self.repos.read();
        self.dashboard_stats = repos.iter().fold(
            DashboardStats {
                total_repos: repos.len(),
                ready: 0,
                errors: 0,
                warnings: 0,
                selected: 0,
                last_refresh: Instant::now(),
            },
            |mut s, r| {
                s.errors += matches!(r.status, RepoStatus::Error(_)) as usize + (!r.errors.is_empty()) as usize;
                s.warnings += matches!(r.status, RepoStatus::Warning(_)) as usize + (!r.warnings.is_empty()) as usize;
                s.selected += r.selected as usize;
                s.ready += (r.is_ready() && !r.has_issues()) as usize;
                s
            },
        );
    }
    fn record_operation(&mut self) {
        self.last_update = Local::now();
        if std::mem::take(&mut self.refresh_paused) {
            self.add_log(LogLevel::Info, "Refresh re-enabled after other operation".to_string(), None);
        }
        self.consecutive_refresh_count = 0;
    }
    fn handle_clone_progress(&mut self, repo_id: usize, progress: git_ops::CloneProgress) {
        match progress {
            git_ops::CloneProgress::Started => {
                self.add_log(LogLevel::Info, "Clone started...".to_string(), Some(repo_id));
            }
            git_ops::CloneProgress::Progress(percent) => {
                let mut repos = self.repos.write();
                if let Some(repo) = repos.iter_mut().find(|r| r.id == repo_id) {
                    repo.status = RepoStatus::Cloning(percent);
                    repo.last_updated = Local::now();
                }
            }
            git_ops::CloneProgress::Completed(result) => {
                let success = result.success;
                let message = result.message; // Take ownership
                {
                    let mut repos = self.repos.write();
                    if let Some(repo) = repos.iter_mut().find(|r| r.id == repo_id) {
                        if success {
                            repo.status = RepoStatus::Ready;
                            repo.last_updated = Local::now();
                            repo.errors.clear();
                        } else {
                            repo.status = RepoStatus::Error(message.clone());
                            repo.last_updated = Local::now();
                            repo.errors.push(message.clone());
                        }
                    }
                } // repos lock dropped here
                if success {
                    self.add_log(LogLevel::Success, format!("Cloned successfully: {}", message), Some(repo_id));
                    self.validate_repository_environment(repo_id);
                    self.save_config();
                } else {
                    self.add_log(LogLevel::Error, format!("Clone failed: {}", message), Some(repo_id));
                }
                self.update_stats();
                self.record_operation(); // Track this operation for "Last Update"
            }
        }
    }
    fn handle_key_event(&mut self, key: KeyEvent) -> Result<()> {
        if key.kind != KeyEventKind::Press {
            return Ok(());
        }
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            self.add_log(LogLevel::Info, "Received interrupt signal, shutting down...".to_string(), None);
            self.should_quit.store(true, Ordering::SeqCst);
            return Ok(());
        }
        if self.input_mode == InputMode::AddingRepos || self.input_mode == InputMode::AddingEnvSample {
            return self.handle_input_mode_keys(key);
        }
        if self.input_mode == InputMode::ConfiguringClone {
            return self.handle_clone_config_keys(key);
        }
        if self.input_mode == InputMode::EditingEnvVars {
            return self.handle_env_vars_edit_keys(key);
        }
        if self.input_mode == InputMode::EditingRequiredFiles {
            return self.handle_required_files_edit_keys(key);
        }
        if self.input_mode == InputMode::EditingDockerCompose {
            return self.handle_docker_compose_edit_keys(key);
        }
        if self.input_mode == InputMode::EditingRepoPath {
            return self.handle_repo_path_edit_keys(key);
        }
        if self.input_mode == InputMode::EditingBasePath {
            return self.handle_base_path_edit_keys(key);
        }
        if self.input_mode == InputMode::EditingRepoBasePath {
            return self.handle_repo_base_path_edit_keys(key);
        }
        if self.input_mode == InputMode::SelectingWorkflow {
            self.handle_workflow_tab_keys(key);
            return Ok(());
        }
        if self.input_mode == InputMode::EditingWorkflow {
            self.handle_workflow_tab_keys(key);
            return Ok(());
        }
        if self.show_help {
            if matches!(key.code, KeyCode::Esc | KeyCode::Char('h' | 'q')) {
                self.show_help = false;
            }
            return Ok(());
        }
        if !self.toasts.is_empty() && key.code == KeyCode::Esc {
            self.toasts.clear();
            return Ok(());
        }
        if self.show_error_popup {
            if matches!(key.code, KeyCode::Esc) {
                self.show_error_popup = false;
            }
            return Ok(());
        }
        if self.show_save_dialog {
            match key.code {
                KeyCode::Enter | KeyCode::Char('y' | 'Y') => {
                    self.show_save_dialog = false;
                    self.save_config();
                }
                KeyCode::Esc | KeyCode::Char('n' | 'N') => {
                    self.show_save_dialog = false;
                    self.add_log(LogLevel::Info, "Save cancelled".to_string(), None);
                }
                _ => {}
            }
            return Ok(());
        }
        if self.show_confirm_dialog {
            match key.code {
                KeyCode::Enter => {
                    let action = self.confirm_action.take();
                    self.show_confirm_dialog = false;
                    self.confirm_dialog_scroll = 0;
                    match action.as_deref() {
                        Some("quit") => {
                            self.add_log(LogLevel::Info, "Quitting application...".to_string(), None);
                            self.should_quit.store(true, Ordering::SeqCst);
                        }
                        Some("clone_selected") => self.clone_selected_repositories(),
                        Some(a) if a.starts_with("validate") => {
                            if let Some(id) = a.rsplit('_').next().and_then(|s| s.parse::<usize>().ok()) {
                                self.validate_repository_environment(id);
                            }
                        }
                        _ => self.add_log(LogLevel::Success, "Action confirmed".to_string(), None),
                    }
                }
                KeyCode::Esc => {
                    self.show_confirm_dialog = false;
                    self.confirm_action = None;
                    self.confirm_dialog_scroll = 0;
                    self.add_log(LogLevel::Info, "Action cancelled".to_string(), None);
                }
                KeyCode::Right | KeyCode::Char('+') => self.confirm_dialog_expanded = true,
                KeyCode::Left | KeyCode::Char('-') => self.confirm_dialog_expanded = false,
                KeyCode::Up | KeyCode::Char('k') => {
                    self.confirm_dialog_scroll = self.confirm_dialog_scroll.saturating_sub(1);
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    self.confirm_dialog_scroll = self.confirm_dialog_scroll.saturating_add(1);
                }
                KeyCode::PageUp => {
                    self.confirm_dialog_scroll = self.confirm_dialog_scroll.saturating_sub(5);
                }
                KeyCode::PageDown => {
                    self.confirm_dialog_scroll = self.confirm_dialog_scroll.saturating_add(5);
                }
                _ => {}
            }
            return Ok(());
        }
        match key.code {
            KeyCode::Char('q' | 'Q') => {
                if key.modifiers.contains(KeyModifiers::CONTROL) {
                    self.should_quit.store(true, Ordering::SeqCst);
                } else {
                    self.show_confirm_dialog = true;
                    self.confirm_action = Some("quit".to_string());
                }
            }
            KeyCode::Char('h') => {
                self.show_help = true;
            }
            KeyCode::Char('s' | 'S') => {
                self.show_save_dialog = true;
            }
            KeyCode::F(5) => {
                self.update_stats();
                self.add_log(LogLevel::Info, "Manual refresh triggered".to_string(), None);
            }
            _ => {}
        }
        match key.code {
            KeyCode::Right => {
                self.current_tab = self.current_tab.next();
                return Ok(());
            }
            KeyCode::Left => {
                self.current_tab = self.current_tab.prev();
                return Ok(());
            }
            KeyCode::Tab => {
                self.current_tab = if key.modifiers.contains(KeyModifiers::SHIFT) {
                    self.current_tab.prev()
                } else {
                    self.current_tab.next()
                };
                return Ok(());
            }
            _ => {}
        }
        match self.current_tab {
            DashboardTab::Repositories => self.handle_repo_tab_keys(key),
            DashboardTab::Environment => self.handle_env_tab_keys(key),
            DashboardTab::Logs => self.handle_logs_tab_keys(key),
            DashboardTab::Workflows => self.handle_workflow_tab_keys(key),
        }
        Ok(())
    }
    fn handle_repo_tab_keys(&mut self, key: KeyEvent) {
        let repo_count = self.repos.read().len();
        match key.code {
            KeyCode::Up if repo_count > 0 => {
                let current = self.repo_table_state.selected().unwrap_or(0);
                self.repo_table_state.select(Some((current + repo_count - 1) % repo_count));
            }
            KeyCode::Down if repo_count > 0 => {
                let current = self.repo_table_state.selected().unwrap_or(0);
                self.repo_table_state.select(Some((current + 1) % repo_count));
            }
            KeyCode::Char(' ') => self.toggle_selected_repo(),
            KeyCode::Enter => self.confirm_validate_selected(),
            KeyCode::Char('c') => self.start_clone_config(),
            KeyCode::Char('v') => self.confirm_validate_env_selected(),
            KeyCode::Char('r') => {
                self.refresh_repository_status();
                self.update_stats();
                self.add_log(LogLevel::Info, "Repository status refreshed".to_string(), None);
            }
            KeyCode::Char('b') => self.start_edit_base_path(),
            KeyCode::Char('B') => self.start_edit_repo_base_path(),
            KeyCode::Char('p') => self.start_edit_repo_path(),
            KeyCode::Char('a') => {
                self.input_mode = InputMode::AddingRepos;
                self.input_state.clear();
                self.add_log(
                    LogLevel::Info,
                    "Enter repository URLs (one per line, Enter to add, Esc to finish)".to_string(),
                    None,
                );
            }
            KeyCode::Delete | KeyCode::Char('d') if key.code == KeyCode::Delete || key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.delete_selected_repo();
            }
            _ => {}
        }
    }
    fn toggle_selected_repo(&mut self) {
        let Some(selected) = self.repo_table_state.selected() else {
            return;
        };
        let (msg, id) = {
            let mut repos = self.repos.write();
            let Some(repo) = repos.get_mut(selected) else {
                return;
            };
            repo.selected = !repo.selected;
            let msg = format!("{}elected repository: {}", if repo.selected { "S" } else { "Des" }, repo.name);
            (msg, repo.id)
        };
        self.update_stats();
        self.add_log(LogLevel::Info, msg, Some(id));
    }
    fn confirm_validate_selected(&mut self) {
        let Some(selected) = self.repo_table_state.selected() else {
            return;
        };
        let repos = self.repos.read();
        if let Some(repo) = repos.get(selected) {
            self.show_confirm_dialog = true;
            self.confirm_action = Some(format!("validate_{}", repo.id));
            self.confirm_dialog_expanded = false;
            self.confirm_dialog_scroll = 0;
        }
    }
    fn confirm_validate_env_selected(&mut self) {
        let Some(selected) = self.repo_table_state.selected() else {
            return;
        };
        let repos = self.repos.read();
        if let Some(repo) = repos.get(selected) {
            self.show_confirm_dialog = true;
            self.confirm_action = Some(format!("validate_env_{}", repo.id));
            self.confirm_dialog_expanded = true;
            self.confirm_dialog_scroll = 0;
        }
    }
    fn start_clone_config(&mut self) {
        if self.repos.read().is_empty() {
            self.add_log(LogLevel::Warning, "No repositories to clone. Add repositories first with 'a'".to_string(), None);
        } else {
            self.input_mode = InputMode::ConfiguringClone;
            self.clone_config.selected_repo_index = 0;
            self.clone_config.editing = false;
            self.add_log(
                LogLevel::Info,
                "Configure clone paths (↑/↓ to navigate, e to edit path, Enter to clone)".to_string(),
                None,
            );
        }
    }
    fn start_edit_base_path(&mut self) {
        self.input_state.buffer.clone_from(&self.clone_config.base_path);
        self.input_state.cursor = self.input_state.buffer.len();
        self.input_state.placeholder = "Enter global base path for repositories".to_string();
        self.input_mode = InputMode::EditingBasePath;
        self.add_log(LogLevel::Info, "Edit global base path (Enter to save, Esc to cancel)".to_string(), None);
    }
    fn start_edit_repo_base_path(&mut self) {
        let Some(selected) = self.repo_table_state.selected() else {
            return;
        };
        let repos = self.repos.read();
        let Some(repo) = repos.get(selected) else {
            return;
        };
        self.editing_repo_id = Some(repo.id);
        self.input_state.buffer.clone_from(&repo.base_path);
        self.input_state.cursor = self.input_state.buffer.len();
        self.input_state.placeholder = "Enter base path for this repository".to_string();
        drop(repos);
        self.input_mode = InputMode::EditingRepoBasePath;
        self.add_log(LogLevel::Info, "Edit repo base path (Enter to save, Esc to cancel)".to_string(), None);
    }
    fn start_edit_repo_path(&mut self) {
        let Some(selected) = self.repo_table_state.selected() else {
            return;
        };
        let repos = self.repos.read();
        let Some(repo) = repos.get(selected) else {
            return;
        };
        self.editing_repo_id = Some(repo.id);
        self.input_state.buffer = repo.path.to_string_lossy().to_string();
        self.input_state.cursor = self.input_state.buffer.len();
        self.input_state.placeholder = "Enter new path for repository".to_string();
        drop(repos);
        self.input_mode = InputMode::EditingRepoPath;
        self.add_log(LogLevel::Info, "Edit repository path (Enter to save, Esc to cancel)".to_string(), None);
    }
    fn delete_selected_repo(&mut self) {
        let Some(selected) = self.repo_table_state.selected() else {
            return;
        };
        let mut repos = self.repos.write();
        if selected >= repos.len() {
            return;
        }
        let removed = repos.remove(selected);
        let new_len = repos.len();
        drop(repos);
        self.add_log(LogLevel::Info, format!("Removed repository: {}", removed.name), Some(removed.id));
        self.update_stats();
        self.record_operation();
        self.repo_table_state.select((new_len > 0).then(|| selected.min(new_len - 1)));
    }
    fn handle_input_mode_keys(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Esc => {
                if !self.input_state.buffer.trim().is_empty() {
                    let url = self.input_state.buffer.trim().to_string();
                    self.add_repositories_from_urls(vec![url]);
                }
                self.input_mode = InputMode::Normal;
                self.input_state.clear();
                self.add_log(LogLevel::Info, "Finished adding repositories".to_string(), None);
            }
            KeyCode::Enter => {
                if !self.input_state.buffer.trim().is_empty() {
                    let url = self.input_state.buffer.trim().to_string();
                    self.input_state.buffer.clear();
                    self.input_state.cursor = 0;
                    self.add_repositories_from_urls(vec![url]);
                }
            }
            KeyCode::Char(c) if key.modifiers.contains(KeyModifiers::CONTROL) && c == 'v' => {}
            _ => {
                self.input_state.handle_edit_key(key);
            }
        }
        Ok(())
    }
    fn handle_clone_config_keys(&mut self, key: KeyEvent) -> Result<()> {
        let repos_to_clone: Vec<(usize, String, PathBuf)> = {
            let repos = self.repos.read();
            let selected: Vec<_> = repos
                .iter()
                .filter(|r| r.selected)
                .map(|r| (r.id, r.name.clone(), r.path.clone()))
                .collect();
            if selected.is_empty() {
                repos.iter().map(|r| (r.id, r.name.clone(), r.path.clone())).collect()
            } else {
                selected
            }
        };
        let repo_count = repos_to_clone.len();
        if self.clone_config.editing {
            match key.code {
                KeyCode::Esc => {
                    self.clone_config.editing = false;
                    self.add_log(LogLevel::Info, "Path editing cancelled".to_string(), None);
                }
                KeyCode::Enter => {
                    if let Some((repo_id, ..)) = repos_to_clone.get(self.clone_config.selected_repo_index) {
                        let new_path = self.input_state.buffer.trim().to_string();
                        if !new_path.is_empty() {
                            let mut repos = self.repos.write();
                            if let Some(repo) = repos.iter_mut().find(|r| r.id == *repo_id) {
                                repo.path = platform::normalize_path(&new_path);
                                drop(repos);
                                self.add_log(LogLevel::Info, "Path updated for repo".to_string(), None);
                            }
                        }
                    }
                    self.clone_config.editing = false;
                    self.input_state.clear();
                }
                _ => {
                    self.input_state.handle_edit_key(key);
                }
            }
            return Ok(());
        }
        match key.code {
            KeyCode::Esc => {
                self.input_mode = InputMode::Normal;
                self.clone_config.selected_repo_index = 0;
                self.add_log(LogLevel::Info, "Clone configuration cancelled".to_string(), None);
            }
            KeyCode::Up => {
                if repo_count > 0 && self.clone_config.selected_repo_index > 0 {
                    self.clone_config.selected_repo_index -= 1;
                }
            }
            KeyCode::Down => {
                if repo_count > 0 && self.clone_config.selected_repo_index < repo_count - 1 {
                    self.clone_config.selected_repo_index += 1;
                }
            }
            KeyCode::Char('e') | KeyCode::Tab => {
                if let Some((repo_id, ..)) = repos_to_clone.get(self.clone_config.selected_repo_index) {
                    let repos = self.repos.read();
                    if let Some(repo) = repos.iter().find(|r| r.id == *repo_id) {
                        self.input_state.buffer = repo.path.to_string_lossy().to_string();
                        self.input_state.cursor = self.input_state.buffer.len();
                        self.clone_config.editing = true;
                    }
                }
            }
            KeyCode::Enter => {
                let conflicts = self.check_clone_path_conflicts();
                if !conflicts.is_empty() {
                    let conflict_messages: Vec<String> = conflicts
                        .iter()
                        .map(|(name, path, reason)| format!("• {}: {} ({})", name, path.display(), reason))
                        .collect();
                    let error_msg = format!("Cannot clone - directory conflicts detected:\n{}", conflict_messages.join("\n"));
                    self.show_error_popup = true;
                    self.error_message.clone_from(&error_msg);
                    self.add_log(LogLevel::Error, "Clone path conflicts detected".to_string(), None);
                    return Ok(());
                }
                self.input_mode = InputMode::Normal;
                self.clone_config.selected_repo_index = 0;
                self.show_confirm_dialog = true;
                self.confirm_action = Some("clone_selected".to_string());
                self.confirm_dialog_expanded = true;
            }
            _ => {}
        }
        Ok(())
    }
    fn handle_env_vars_edit_keys(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Esc => {
                self.input_mode = InputMode::Normal;
                self.input_state.clear();
                self.add_log(LogLevel::Info, "Cancelled .env editing".to_string(), None);
            }
            KeyCode::F(2) => {
                self.save_env_vars_to_file();
                self.input_mode = InputMode::Normal;
                self.input_state.clear();
            }
            KeyCode::Enter => self.input_state.insert_newline(),
            _ => {
                self.input_state.handle_multiline_key(key);
            }
        }
        Ok(())
    }
    fn handle_required_files_edit_keys(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Esc => {
                self.input_mode = InputMode::Normal;
                self.input_state.clear();
                self.add_log(LogLevel::Info, "Cancelled required files editing".to_string(), None);
            }
            KeyCode::F(2) | KeyCode::Enter if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.save_required_files();
                self.input_mode = InputMode::Normal;
                self.input_state.clear();
            }
            KeyCode::Enter => self.input_state.insert_newline(),
            _ => {
                self.input_state.handle_edit_key(key);
            }
        }
        Ok(())
    }
    fn save_required_files(&mut self) {
        let Some(selected) = self.repo_table_state.selected() else {
            self.add_log(LogLevel::Error, "No repository selected".to_string(), None);
            return;
        };
        let required_files: Vec<String> = self
            .input_state
            .buffer
            .lines()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect();
        let Some((repo_id, repo_name)) = ({
            let mut repos = self.repos.write();
            repos.get_mut(selected).map(|repo| {
                repo.required_files.clone_from(&required_files);
                repo.last_updated = Local::now();
                (repo.id, repo.name.clone())
            })
        }) else {
            self.add_log(LogLevel::Error, "Repository not found".to_string(), None);
            return;
        };
        self.add_log(
            LogLevel::Success,
            format!("Updated required files for {}: {} file(s)", repo_name, required_files.len()),
            Some(repo_id),
        );
        self.validate_repository_environment(repo_id);
        self.save_config();
        self.record_operation();
    }
    fn handle_repo_path_edit_keys(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Esc => {
                self.input_mode = InputMode::Normal;
                self.editing_repo_id = None;
                self.input_state.clear();
                self.add_log(LogLevel::Info, "Path editing cancelled".to_string(), None);
            }
            KeyCode::Enter => {
                self.save_repo_path();
                self.input_mode = InputMode::Normal;
                self.editing_repo_id = None;
                self.input_state.clear();
            }
            _ => {
                self.input_state.handle_edit_key(key);
            }
        }
        Ok(())
    }
    fn handle_base_path_edit_keys(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Esc => {
                self.input_mode = InputMode::Normal;
                self.input_state.clear();
                self.add_log(LogLevel::Info, "Base path editing cancelled".to_string(), None);
            }
            KeyCode::Enter => {
                let new_path = self.input_state.buffer.trim().to_string();
                if new_path.is_empty() {
                    self.add_log(LogLevel::Warning, "Base path cannot be empty, keeping current".to_string(), None);
                } else {
                    let old_path = self.clone_config.base_path.clone();
                    self.clone_config.base_path.clone_from(&new_path);
                    self.add_log(LogLevel::Success, format!("Base path updated: {} → {}", old_path, new_path), None);
                    self.refresh_repository_status();
                    self.save_config();
                }
                self.input_mode = InputMode::Normal;
                self.input_state.clear();
            }
            _ => {
                self.input_state.handle_edit_key(key);
            }
        }
        Ok(())
    }
    fn handle_repo_base_path_edit_keys(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Esc => {
                self.input_mode = InputMode::Normal;
                self.editing_repo_id = None;
                self.input_state.clear();
                self.add_log(LogLevel::Info, "Repo base path editing cancelled".to_string(), None);
            }
            KeyCode::Enter => {
                self.save_repo_base_path();
                self.input_mode = InputMode::Normal;
                self.editing_repo_id = None;
                self.input_state.clear();
            }
            _ => {
                self.input_state.handle_edit_key(key);
            }
        }
        Ok(())
    }
    fn save_repo_base_path(&mut self) {
        let Some(repo_id) = self.editing_repo_id else {
            self.add_log(LogLevel::Error, "No repository being edited".to_string(), None);
            return;
        };
        let new_base_path = self.input_state.buffer.trim().to_string();
        if new_base_path.is_empty() {
            self.add_log(LogLevel::Error, "Base path cannot be empty".to_string(), None);
            return;
        }
        let mut repos = self.repos.write();
        if let Some(repo) = repos.iter_mut().find(|r| r.id == repo_id) {
            let old_base_path = repo.base_path.clone();
            repo.base_path.clone_from(&new_base_path);
            let new_full_path = platform::normalize_path(&format!(
                "{}/{}/{}",
                if new_base_path.is_empty() { "." } else { &new_base_path },
                if self.clone_config.dir_name.is_empty() {
                    "projects"
                } else {
                    &self.clone_config.dir_name
                },
                &repo.name
            ));
            let old_path = repo.path.clone();
            repo.path.clone_from(&new_full_path);
            repo.last_updated = Local::now();
            let repo_name = repo.name.clone();
            drop(repos);
            self.add_log(
                LogLevel::Success,
                format!(
                    "[{}] Base path: {} → {} | Full path: {} → {}",
                    repo_name,
                    old_base_path,
                    new_base_path,
                    old_path.display(),
                    new_full_path.display()
                ),
                Some(repo_id),
            );
            self.refresh_repository_status();
            self.save_config();
        } else {
            drop(repos);
            self.add_log(LogLevel::Error, "Repository not found".to_string(), None);
        }
    }
    fn save_repo_path(&mut self) {
        let Some(repo_id) = self.editing_repo_id else {
            self.add_log(LogLevel::Error, "No repository being edited".to_string(), None);
            return;
        };
        let new_path = self.input_state.buffer.trim().to_string();
        if new_path.is_empty() {
            self.add_log(LogLevel::Error, "Path cannot be empty".to_string(), None);
            return;
        }
        let normalized_path = platform::normalize_path(&new_path);
        let mut repos = self.repos.write();
        if let Some(repo) = repos.iter_mut().find(|r| r.id == repo_id) {
            let old_path = repo.path.clone();
            repo.path.clone_from(&normalized_path);
            repo.last_updated = Local::now();
            let repo_name = repo.name.clone();
            drop(repos);
            if normalized_path.join(".git").exists() {
                self.add_log(
                    LogLevel::Success,
                    format!("Updated path for '{}': {} (repo detected)", repo_name, normalized_path.display()),
                    Some(repo_id),
                );
            } else {
                self.add_log(
                    LogLevel::Info,
                    format!("Updated path for '{}': {} → {}", repo_name, old_path.display(), normalized_path.display()),
                    Some(repo_id),
                );
            }
            self.refresh_repository_status();
            self.save_config();
            self.record_operation();
        } else {
            drop(repos);
            self.add_log(LogLevel::Error, format!("Repository {} not found", repo_id), None);
        }
    }
    const MAX_CONSECUTIVE_REFRESHES: usize = 20;
    fn refresh_repository_status(&mut self) {
        if self.refresh_paused {
            return;
        }
        self.consecutive_refresh_count += 1;
        if self.consecutive_refresh_count >= Self::MAX_CONSECUTIVE_REFRESHES {
            self.refresh_paused = true;
            self.add_log(
                LogLevel::Warning,
                format!(
                    "Refresh paused after {} consecutive retries. Perform any operation to re-enable.",
                    Self::MAX_CONSECUTIVE_REFRESHES
                ),
                None,
            );
            return;
        }
        let mut repos = self.repos.write();
        let mut updated_count = 0;
        let mut discovered_count = 0;
        let base_path = self.clone_config.base_path.clone();
        let dir_name = self.clone_config.dir_name.clone();
        for repo in repos.iter_mut() {
            let status_was_ready = matches!(repo.status, RepoStatus::Ready);
            let path_exists = repo.path.exists();
            let git_exists = repo.path.join(".git").exists();
            if git_exists && !status_was_ready {
                repo.status = RepoStatus::Ready;
                repo.last_updated = Local::now();
                updated_count += 1;
            } else if !git_exists && status_was_ready {
                repo.status = RepoStatus::Pending;
                repo.last_updated = Local::now();
                updated_count += 1;
            } else if !path_exists && matches!(repo.status, RepoStatus::Pending) {
                if let Some((found_path, is_cloned)) = discovery::discover_repo_by_name(&repo.name, &base_path, &dir_name) {
                    repo.path = found_path;
                    repo.status = if is_cloned { RepoStatus::Ready } else { RepoStatus::Pending };
                    repo.last_updated = Local::now();
                    discovered_count += 1;
                }
            } else if git_exists && !status_was_ready && matches!(repo.status, RepoStatus::Pending) {
                repo.status = RepoStatus::Ready;
                repo.last_updated = Local::now();
                updated_count += 1;
            }
            repo.missing_files = repo
                .required_files
                .iter()
                .filter(|f| !repo.path.join(f).exists())
                .cloned()
                .collect();
        }
        drop(repos);
        if updated_count > 0 {
            self.add_log(LogLevel::Info, format!("Detected {} repository status change(s)", updated_count), None);
        }
        if discovered_count > 0 {
            self.add_log(LogLevel::Success, format!("Auto-discovered {} repo(s) at new location(s)", discovered_count), None);
        }
    }
    fn handle_docker_compose_edit_keys(&mut self, key: KeyEvent) -> Result<()> {
        if self.docker_compose_state.editing_env {
            return self.handle_env_file_edit_keys(key);
        }
        let (service_count, visible_height) = {
            let repos = self.repos.read();
            let count = self
                .repo_table_state
                .selected()
                .and_then(|idx| repos.get(idx))
                .map_or(0, |r| r.docker_compose_status.services.len());
            (count, 20_usize)
        };
        match key.code {
            KeyCode::Esc => {
                self.input_mode = InputMode::Normal;
                self.docker_compose_state.reset();
                self.input_state.clear();
                self.add_log(LogLevel::Info, "Closed docker-compose view".to_string(), None);
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if service_count > 0 && self.docker_compose_state.selected_service > 0 {
                    self.docker_compose_state.selected_service -= 1;
                    if self.docker_compose_state.selected_service < self.docker_compose_state.scroll_offset {
                        self.docker_compose_state.scroll_offset = self.docker_compose_state.selected_service;
                    }
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if service_count > 0 && self.docker_compose_state.selected_service < service_count - 1 {
                    self.docker_compose_state.selected_service += 1;
                    let max_visible = self.docker_compose_state.scroll_offset + visible_height;
                    if self.docker_compose_state.selected_service >= max_visible {
                        self.docker_compose_state.scroll_offset =
                            self.docker_compose_state.selected_service.saturating_sub(visible_height - 1);
                    }
                }
            }
            KeyCode::Left | KeyCode::Char('h') => {
                if service_count > 0 {
                    self.docker_compose_state
                        .expanded_services
                        .remove(&self.docker_compose_state.selected_service);
                }
            }
            KeyCode::Right | KeyCode::Char('l') => {
                if service_count > 0 {
                    self.docker_compose_state
                        .expanded_services
                        .insert(self.docker_compose_state.selected_service);
                }
            }
            KeyCode::PageUp => {
                let page_size = visible_height.saturating_sub(2);
                if self.docker_compose_state.selected_service > page_size {
                    self.docker_compose_state.selected_service -= page_size;
                } else {
                    self.docker_compose_state.selected_service = 0;
                }
                if self.docker_compose_state.scroll_offset > self.docker_compose_state.selected_service {
                    self.docker_compose_state.scroll_offset = self.docker_compose_state.selected_service;
                }
            }
            KeyCode::PageDown => {
                let page_size = visible_height.saturating_sub(2);
                if self.docker_compose_state.selected_service + page_size < service_count {
                    self.docker_compose_state.selected_service += page_size;
                } else if service_count > 0 {
                    self.docker_compose_state.selected_service = service_count - 1;
                }
                let max_visible = self.docker_compose_state.scroll_offset + visible_height;
                if self.docker_compose_state.selected_service >= max_visible {
                    self.docker_compose_state.scroll_offset = self.docker_compose_state.selected_service.saturating_sub(visible_height - 1);
                }
            }
            KeyCode::Home => {
                self.docker_compose_state.selected_service = 0;
                self.docker_compose_state.scroll_offset = 0;
            }
            KeyCode::End => {
                if service_count > 0 {
                    self.docker_compose_state.selected_service = service_count - 1;
                    if self.docker_compose_state.selected_service >= visible_height {
                        self.docker_compose_state.scroll_offset =
                            self.docker_compose_state.selected_service.saturating_sub(visible_height - 1);
                    }
                }
            }
            KeyCode::Enter | KeyCode::Char(' ') => {
                if service_count > 0 {
                    self.docker_compose_state
                        .toggle_expanded(self.docker_compose_state.selected_service);
                }
            }
            KeyCode::Char('e') => {
                self.edit_service_env_file();
            }
            KeyCode::Char('m') => {
                self.migrate_selected_service_env();
            }
            KeyCode::Char('i') => {
                self.inline_selected_service_env();
            }
            _ => {}
        }
        Ok(())
    }
    fn migrate_selected_service_env(&mut self) {
        let Some(selected_repo) = self.repo_table_state.selected() else {
            return;
        };
        let (repo_path, compose_path, service_name, has_environment) = {
            let repos = self.repos.read();
            let Some(repo) = repos.get(selected_repo) else {
                return;
            };
            let compose_path = docker_compose::find_compose_file(&repo.path);
            let service_idx = self.docker_compose_state.selected_service;
            let service = repo.docker_compose_status.services.get(service_idx);
            match (compose_path, service) {
                (Some(path), Some(svc)) => (repo.path.clone(), path, svc.name.clone(), svc.has_environment),
                _ => return,
            }
        };
        if !has_environment {
            self.add_log(LogLevel::Info, format!("Service '{}' has no environment section to migrate", service_name), None);
            return;
        }
        let env_folder = self.docker_compose_state.env_folder.clone();
        match docker_compose::migrate_env_to_file(&compose_path, &repo_path, &service_name, &env_folder) {
            Ok((env_file_path, relative_path)) => {
                self.add_log(LogLevel::Success, format!("Moved '{}' env vars to {}", service_name, relative_path), None);
                self.add_log(LogLevel::Info, format!("Created: {}", env_file_path.display()), None);
                let repos = self.repos.read();
                if let Some(repo) = repos.get(selected_repo) {
                    let repo_id = repo.id;
                    drop(repos);
                    self.validate_repository_environment(repo_id);
                }
            }
            Err(e) => {
                self.add_log(LogLevel::Error, format!("Failed to migrate service: {}", e), None);
            }
        }
    }
    fn inline_selected_service_env(&mut self) {
        let Some(selected_repo) = self.repo_table_state.selected() else {
            return;
        };
        let (repo_path, compose_path, service_name, has_env_file) = {
            let repos = self.repos.read();
            let Some(repo) = repos.get(selected_repo) else {
                return;
            };
            let compose_path = docker_compose::find_compose_file(&repo.path);
            let service_idx = self.docker_compose_state.selected_service;
            let service = repo.docker_compose_status.services.get(service_idx);
            match (compose_path, service) {
                (Some(path), Some(svc)) => (repo.path.clone(), path, svc.name.clone(), svc.has_env_file),
                _ => return,
            }
        };
        if !has_env_file {
            self.add_log(LogLevel::Info, format!("Service '{}' has no env_file reference to inline", service_name), None);
            return;
        }
        match docker_compose::restore_env_from_file(&compose_path, &repo_path, &service_name) {
            Ok(var_count) => {
                self.add_log(
                    LogLevel::Success,
                    format!("Inlined '{}' environment with ${{KEY:-value}} format ({} vars)", service_name, var_count),
                    None,
                );
                let repos = self.repos.read();
                if let Some(repo) = repos.get(selected_repo) {
                    let repo_id = repo.id;
                    drop(repos);
                    self.validate_repository_environment(repo_id);
                }
            }
            Err(e) => {
                self.add_log(LogLevel::Error, format!("Failed to inline service env: {}", e), None);
            }
        }
    }
    fn edit_service_env_file(&mut self) {
        let Some(selected_repo) = self.repo_table_state.selected() else {
            return;
        };
        let (repo_path, service_name, env_files, has_env_file) = {
            let repos = self.repos.read();
            let Some(repo) = repos.get(selected_repo) else {
                return;
            };
            let service_idx = self.docker_compose_state.selected_service;
            let service = repo.docker_compose_status.services.get(service_idx);
            match service {
                Some(svc) => (repo.path.clone(), svc.name.clone(), svc.env_file.clone(), svc.has_env_file),
                _ => return,
            }
        };
        let env_path = if has_env_file && !env_files.is_empty() {
            repo_path.join(&env_files[0])
        } else {
            let env_folder = &self.docker_compose_state.env_folder;
            let env_filename = format!(".env.{}", service_name);
            repo_path.join(env_folder).join(&env_filename)
        };
        if !env_path.exists() {
            self.add_log(
                LogLevel::Info,
                format!("No .env file found for service '{}'. Use 'm' to move environment to .env file first.", service_name),
                None,
            );
            return;
        }
        let content = match std::fs::read_to_string(&env_path) {
            Ok(c) => c,
            Err(e) => {
                self.add_log(LogLevel::Error, format!("Failed to read {}: {}", env_path.display(), e), None);
                return;
            }
        };
        let display_path = env_path
            .file_name()
            .map_or_else(|| env_path.to_string_lossy().to_string(), |n| n.to_string_lossy().to_string());
        self.docker_compose_state.editing_env = true;
        self.docker_compose_state.editing_path = Some(env_path);
        self.input_state.buffer = content;
        self.input_state.cursor = self.input_state.buffer.len();
        self.input_state.placeholder = format!("Edit {} for service '{}' (KEY=value per line)", display_path, service_name);
    }
    fn handle_env_file_edit_keys(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Esc => {
                self.docker_compose_state.editing_env = false;
                self.docker_compose_state.editing_path = None;
                self.input_state.clear();
                self.add_log(LogLevel::Info, "Cancelled .env file editing".to_string(), None);
            }
            KeyCode::Enter if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.save_env_file_edits();
            }
            KeyCode::Enter => self.input_state.insert_newline(),
            _ => {
                self.input_state.handle_multiline_key(key);
            }
        }
        Ok(())
    }
    fn save_env_file_edits(&mut self) {
        let Some(env_path) = self.docker_compose_state.editing_path.take() else {
            self.add_log(LogLevel::Error, "No file path set for saving".to_string(), None);
            return;
        };
        let content = &self.input_state.buffer;
        match std::fs::write(&env_path, content) {
            Ok(_) => {
                let filename = env_path
                    .file_name()
                    .map_or_else(|| env_path.to_string_lossy().to_string(), |n| n.to_string_lossy().to_string());
                self.add_log(LogLevel::Success, format!("Saved {}", filename), None);
                self.docker_compose_state.editing_env = false;
                self.input_state.clear();
                if let Some(selected_repo) = self.repo_table_state.selected() {
                    let repos = self.repos.read();
                    if let Some(repo) = repos.get(selected_repo) {
                        let repo_id = repo.id;
                        drop(repos);
                        self.validate_repository_environment(repo_id);
                    }
                }
            }
            Err(e) => {
                self.add_log(LogLevel::Error, format!("Failed to save {}: {}", env_path.display(), e), None);
                self.docker_compose_state.editing_path = Some(env_path);
            }
        }
    }
    fn check_clone_path_conflicts(&self) -> Vec<(String, PathBuf, String)> {
        let repos = self.repos.read();
        let mut conflicts = Vec::new();
        for repo in repos.iter() {
            if !repo.selected && repos.len() > 1 {
                continue; // Skip unselected repos when multiple exist
            }
            let path = &repo.path;
            if path.exists() {
                let git_dir = path.join(".git");
                if git_dir.exists() {
                    continue;
                }
                if let Ok(entries) = std::fs::read_dir(path) {
                    let has_content = entries.count() > 0;
                    if has_content {
                        conflicts.push((
                            repo.name.clone(),
                            path.clone(),
                            "Directory exists with content but is not a git repository".to_string(),
                        ));
                    }
                } else {
                    conflicts.push((repo.name.clone(), path.clone(), "Cannot read directory".to_string()));
                }
            }
        }
        conflicts
    }
    fn handle_env_tab_keys(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('e') => {
                if let Some(selected) = self.repo_table_state.selected() {
                    let repos = self.repos.read();
                    if let Some(repo) = repos.get(selected) {
                        if !repo.path.exists() {
                            drop(repos);
                            self.add_log(LogLevel::Warning, "Repository not cloned yet. Clone first to edit .env".to_string(), None);
                            return;
                        }
                        let env_path = repo.path.join(".env");
                        let sample_path = expresso_kit::file_patterns::find_env_sample_file(&repo.path);
                        let initial_content = if env_path.exists() {
                            std::fs::read_to_string(&env_path).unwrap_or_default()
                        } else if let Some(ref sample) = sample_path {
                            std::fs::read_to_string(sample).unwrap_or_default()
                        } else {
                            String::new()
                        };
                        drop(repos);
                        self.input_mode = InputMode::EditingEnvVars;
                        self.input_state.buffer = initial_content;
                        self.input_state.cursor = self.input_state.buffer.len();
                        self.input_state.placeholder = "Enter environment variables (KEY=value format, one per line)".to_string();
                        self.add_log(
                            LogLevel::Info,
                            "Editing .env file. Paste or type variables, Enter to save, Esc to cancel".to_string(),
                            None,
                        );
                    }
                }
            }
            KeyCode::Char('f') => {
                if let Some(selected) = self.repo_table_state.selected() {
                    let repos = self.repos.read();
                    if let Some(repo) = repos.get(selected) {
                        let initial_content = repo.required_files.join("\n");
                        drop(repos);
                        self.input_mode = InputMode::EditingRequiredFiles;
                        self.input_state.buffer = initial_content;
                        self.input_state.cursor = self.input_state.buffer.len();
                        self.input_state.placeholder = "Enter required files (1 per line, e.g., .env, Dockerfile)".to_string();
                        self.add_log(
                            LogLevel::Info,
                            "Editing required files. One file per line, Enter to save, Esc to cancel".to_string(),
                            None,
                        );
                    }
                }
            }
            KeyCode::Char('d') => {
                if let Some(selected) = self.repo_table_state.selected() {
                    let repos = self.repos.read();
                    if let Some(repo) = repos.get(selected) {
                        let repo_path = repo.path.clone();
                        let repo_name = repo.name.clone();
                        let docker_status = repo.docker_compose_status.clone();
                        drop(repos);
                        if docker_status.sample_exists && !docker_status.compose_exists {
                            match docker_compose::copy_compose_sample(&repo_path) {
                                Ok(copied_path) => {
                                    self.add_log(LogLevel::Success, format!("Copied docker-compose sample to: {:?}", copied_path), None);
                                    let repos = self.repos.read();
                                    if let Some(repo) = repos.iter().find(|r| r.name == repo_name) {
                                        let repo_id = repo.id;
                                        drop(repos);
                                        self.validate_repository_environment(repo_id);
                                    }
                                }
                                Err(e) => {
                                    self.add_log(LogLevel::Error, format!("Failed to copy docker-compose sample: {}", e), None);
                                }
                            }
                        } else if docker_status.compose_exists {
                            self.docker_compose_state.reset();
                            self.input_mode = InputMode::EditingDockerCompose;
                            self.add_log(
                                LogLevel::Info,
                                "Docker-compose editor: ↑↓ select, Enter expand, m migrate, e edit env, Esc close".to_string(),
                                None,
                            );
                        } else {
                            self.add_log(
                                LogLevel::Warning,
                                "No docker-compose sample or compose file found in this repository".to_string(),
                                None,
                            );
                        }
                    }
                }
            }
            KeyCode::Char('c') => {
                if let Some(selected) = self.repo_table_state.selected() {
                    self.copy_env_sample_to_env(selected);
                }
            }
            KeyCode::Char('v') => {
                if let Some(selected) = self.repo_table_state.selected() {
                    let repos = self.repos.read();
                    if let Some(repo) = repos.get(selected) {
                        let repo_id = repo.id;
                        drop(repos);
                        self.validate_repository_environment(repo_id);
                    }
                }
            }
            KeyCode::Up => {
                let repo_count = self.repos.read().len();
                if repo_count > 0 {
                    let current = self.repo_table_state.selected().unwrap_or(0);
                    let new = if current == 0 { repo_count - 1 } else { current - 1 };
                    self.repo_table_state.select(Some(new));
                }
            }
            KeyCode::Down => {
                let repo_count = self.repos.read().len();
                if repo_count > 0 {
                    let current = self.repo_table_state.selected().unwrap_or(0);
                    let new = if current >= repo_count - 1 { 0 } else { current + 1 };
                    self.repo_table_state.select(Some(new));
                }
            }
            _ => {}
        }
    }
    fn handle_logs_tab_keys(&mut self, key: KeyEvent) {
        if let KeyCode::Char('c') = key.code {
            self.logs.lock().clear();
            self.add_log(LogLevel::Info, "Logs cleared".to_string(), None);
        }
    }
    fn handle_workflow_tab_keys(&mut self, key: KeyEvent) {
        let repo_count = self.repos.read().len();
        if self.input_mode == InputMode::EditingWorkflow {
            match key.code {
                KeyCode::Esc => {
                    self.workflow_state.editing = false;
                    self.input_mode = InputMode::SelectingWorkflow;
                    self.add_log(LogLevel::Info, "Exited workflow editor".to_string(), None);
                }
                KeyCode::Char('s') => {
                    self.workflow_state.workflow_content = self.input_state.buffer.clone();
                    self.save_workflow();
                }
                KeyCode::Enter => {
                    self.input_state.insert_char('\n');
                }
                _ => {
                    self.input_state.handle_multiline_key(key);
                }
            }
            return;
        }
        if self.input_mode == InputMode::SelectingWorkflow {
            match key.code {
                KeyCode::Esc => {
                    self.input_mode = InputMode::Normal;
                    self.workflow_state.visible = false;
                    self.add_log(LogLevel::Info, "Workflow selection cancelled".to_string(), None);
                }
                KeyCode::Up => {
                    self.workflow_state.select_prev();
                    self.regenerate_workflow_preview();
                }
                KeyCode::Down => {
                    self.workflow_state.select_next();
                    self.regenerate_workflow_preview();
                }
                KeyCode::Char('e') => {
                    self.input_state.buffer = self.workflow_state.workflow_content.clone();
                    self.input_state.cursor = 0;
                    self.input_state.placeholder = format!("{}-ci.yml", self.workflow_state.target_name);
                    self.workflow_state.editing = true;
                    self.input_mode = InputMode::EditingWorkflow;
                    self.add_log(LogLevel::Info, "Entered workflow editor".to_string(), None);
                }
                KeyCode::Char('s') => {
                    self.save_workflow();
                }
                _ => {}
            }
            return;
        }
        match key.code {
            KeyCode::Up if repo_count > 0 => {
                let current = self.repo_table_state.selected().unwrap_or(0);
                if current > 0 {
                    self.repo_table_state.select(Some(current - 1));
                }
            }
            KeyCode::Down if repo_count > 0 => {
                let current = self.repo_table_state.selected().unwrap_or(0);
                let max = repo_count.saturating_sub(1);
                if current < max {
                    self.repo_table_state.select(Some(current + 1));
                }
            }
            KeyCode::Char('w') | KeyCode::Enter => {
                self.open_workflow_popup();
            }
            _ => {}
        }
    }
    fn open_workflow_popup(&mut self) {
        let selected_idx = self.repo_table_state.selected().unwrap_or(0);
        let repo_info = {
            let repos = self.repos.read();
            repos.get(selected_idx).map(|r| (r.path.clone(), r.name.clone()))
        };
        if let Some((path, name)) = repo_info {
            let detected = detect_project_types(&path);
            let suggested_template = if detected.rust {
                0 // Rust
            } else if detected.node {
                1 // Node.js
            } else if detected.python {
                2 // Python
            } else if detected.go {
                3 // Go
            } else if detected.docker {
                4 // Docker
            } else if detected.docker_compose {
                5 // Docker Compose
            } else {
                6 // Custom
            };
            let template = WorkflowTemplate::ALL[suggested_template];
            let workflow_content = self.generate_workflow_for_template(template, &detected, &path);
            self.workflow_state = WorkflowPopupState {
                visible: true,
                selected_template: suggested_template,
                detected: Some(detected),
                target_path: Some(path),
                target_name: name.clone(),
                workflow_content,
                cursor_line: 0,
                scroll_offset: 0,
                editing: false,
            };
            self.input_mode = InputMode::SelectingWorkflow;
            self.add_log(LogLevel::Info, format!("Workflow generator opened for '{}'", name), None);
        } else {
            self.add_log(LogLevel::Warning, "No repository selected. Add a repository first.".to_string(), None);
        }
    }
    fn generate_workflow_for_template(&self, template: WorkflowTemplate, detected: &DetectedProjects, path: &std::path::Path) -> String {
        let mut project = detected.clone();
        project.rust = template == WorkflowTemplate::Rust;
        project.node = template == WorkflowTemplate::NodeJs;
        project.python = template == WorkflowTemplate::Python;
        project.go = template == WorkflowTemplate::Go;
        project.docker = template == WorkflowTemplate::Docker;
        project.docker_compose = template == WorkflowTemplate::DockerCompose;
        if template == WorkflowTemplate::Custom {
            return "name: CI
on:
  push:
    branches: [main]
  pull_request:
    branches: [main]
jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      # Add your steps here
"
            .to_string();
        }
        expresso_kit::generate_workflow(&path.to_string_lossy(), project.docker_compose, &project)
    }
    fn regenerate_workflow_preview(&mut self) {
        if let Some(detected) = &self.workflow_state.detected.clone() {
            let template = self.workflow_state.selected_template();
            if let Some(path) = &self.workflow_state.target_path.clone() {
                self.workflow_state.workflow_content = self.generate_workflow_for_template(template, detected, path);
            }
        }
    }
    fn save_workflow(&mut self) {
        if let Some(path) = &self.workflow_state.target_path.clone() {
            let workflow_dir = path.join(".github").join("workflows");
            if let Err(e) = std::fs::create_dir_all(&workflow_dir) {
                self.add_log(LogLevel::Error, format!("Failed to create workflow directory: {}", e), None);
                return;
            }
            let filename = if let Some(detected) = &self.workflow_state.detected {
                detected.workflow_filename()
            } else {
                let template = self.workflow_state.selected_template();
                match template {
                    WorkflowTemplate::Rust => "rust-ci.yml".to_string(),
                    WorkflowTemplate::NodeJs => "node-ci.yml".to_string(),
                    WorkflowTemplate::Python => "python-ci.yml".to_string(),
                    WorkflowTemplate::Go => "go-ci.yml".to_string(),
                    WorkflowTemplate::Docker => "docker-build.yml".to_string(),
                    WorkflowTemplate::DockerCompose => "docker-compose-ci.yml".to_string(),
                    WorkflowTemplate::Custom => "ci.yml".to_string(),
                }
            };
            let workflow_path = workflow_dir.join(&filename);
            match std::fs::write(&workflow_path, &self.workflow_state.workflow_content) {
                Ok(_) => {
                    self.add_log(LogLevel::Info, format!("Workflow saved to {}", workflow_path.display()), None);
                    self.input_mode = InputMode::Normal;
                    self.workflow_state.visible = false;
                    self.workflow_state.editing = false;
                }
                Err(e) => {
                    self.add_log(LogLevel::Error, format!("Failed to save workflow: {}", e), None);
                }
            }
        }
    }
    fn render_dashboard(&mut self, frame: &mut Frame) {
        let size = frame.area();
        let main_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(5), // Top panel (Header + Stats side by side)
                Constraint::Length(2), // Tabs
                Constraint::Min(1),    // Main content
                Constraint::Length(2), // Footer (minimal)
            ])
            .split(size);
        let top_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(30), Constraint::Percentage(50)])
            .split(main_chunks[0]);
        self.render_header(frame, top_chunks[0]);
        StatsPanel { stats: &self.dashboard_stats }.render(top_chunks[1], frame.buffer_mut());
        self.render_tabs(frame, main_chunks[1]);
        match self.current_tab {
            DashboardTab::Repositories => self.render_repositories_tab(frame, main_chunks[2]),
            DashboardTab::Environment => self.render_environment_tab(frame, main_chunks[2]),
            DashboardTab::Logs => self.render_logs_tab(frame, main_chunks[2]),
            DashboardTab::Workflows => self.render_workflows_tab(frame, main_chunks[2]),
        }
        self.render_footer(frame, main_chunks[3]);
        if self.input_mode == InputMode::AddingRepos {
            self.render_input_popup(frame);
        }
        if self.input_mode == InputMode::ConfiguringClone {
            self.render_clone_config_popup(frame);
        }
        if self.input_mode == InputMode::EditingEnvVars {
            self.render_env_vars_popup(frame);
        }
        if self.input_mode == InputMode::EditingRequiredFiles {
            self.render_required_files_popup(frame);
        }
        if self.input_mode == InputMode::EditingDockerCompose {
            self.render_docker_compose_popup(frame);
        }
        if self.input_mode == InputMode::EditingRepoPath {
            self.render_repo_path_popup(frame);
        }
        if self.show_confirm_dialog {
            self.render_confirm_dialog(frame);
        }
        if self.show_save_dialog {
            self.render_save_dialog(frame);
        }
        if self.show_error_popup {
            ErrorPopup { message: &self.error_message }.render(size, frame.buffer_mut());
        }
        if self.show_help {
            HelpPopup.render(size, frame.buffer_mut());
        }
        if self.input_mode == InputMode::EditingBasePath {
            self.render_base_path_popup(frame);
        }
        if self.input_mode == InputMode::SelectingWorkflow {
            self.render_workflow_popup(frame);
        }
        if self.input_mode == InputMode::EditingWorkflow {
            self.render_workflow_editor(frame);
        }
        self.render_toasts(frame);
    }
    fn render_toasts(&self, frame: &mut Frame) {
        if self.toasts.is_empty() {
            return;
        }
        let size = frame.area();
        let toast_width = 60u16.min(size.width.saturating_sub(4));
        let toast_height = 3u16;
        let spacing = 1u16;
        let mut y_offset = size.height.saturating_sub(2);
        for toast in self.toasts.iter().rev().take(5) {
            if y_offset < toast_height + spacing {
                break;
            }
            y_offset = y_offset.saturating_sub(toast_height + spacing);
            let toast_area = Rect::new(size.width.saturating_sub(toast_width + 2), y_offset, toast_width, toast_height);
            Clear.render(toast_area, frame.buffer_mut());
            let border_color = toast.level.color();
            let remaining = toast.remaining_secs();
            let title = format!(" {} ({:.0}s) ", toast.level.icon(), remaining);
            let block = Block::default()
                .borders(Borders::ALL)
                .title(title)
                .title_alignment(Alignment::Left)
                .border_style(Style::default().fg(border_color));
            let inner = block.inner(toast_area);
            block.render(toast_area, frame.buffer_mut());
            let max_len = inner.width as usize;
            let msg = if toast.message.len() > max_len {
                format!("{}...", &toast.message[..max_len.saturating_sub(3)])
            } else {
                toast.message.clone()
            };
            Paragraph::new(msg)
                .style(Style::default().fg(Color::White))
                .render(inner, frame.buffer_mut());
        }
    }
    fn render_base_path_popup(&self, frame: &mut Frame) {
        let popup_area = centered_popup(frame.area(), 60, 30, 50, 8, 80, 12);
        Clear.render(popup_area, frame.buffer_mut());
        let block = Block::default()
            .borders(Borders::ALL)
            .title(" Edit Base Path ")
            .title_alignment(Alignment::Center)
            .border_style(Style::default().fg(Color::Yellow));
        let inner = block.inner(popup_area);
        block.render(popup_area, frame.buffer_mut());
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(2), // Instructions
                Constraint::Length(3), // Input field
                Constraint::Min(1),    // Help text
            ])
            .split(inner);
        let instructions = Paragraph::new("Enter the base path for cloning repositories:").style(Style::default().fg(Color::Gray));
        instructions.render(chunks[0], frame.buffer_mut());
        let input_block = Block::default()
            .borders(Borders::ALL)
            .title(" Path ")
            .border_style(Style::default().fg(Color::Cyan));
        let input_inner = input_block.inner(chunks[1]);
        input_block.render(chunks[1], frame.buffer_mut());
        let buffer = &self.input_state.buffer;
        let cursor_pos = self.input_state.cursor.min(buffer.len());
        let display_text = if buffer.is_empty() {
            Span::styled(&self.clone_config.base_path, Style::default().fg(Color::DarkGray))
        } else {
            Span::styled(buffer.as_str(), Style::default().fg(Color::White))
        };
        Paragraph::new(Line::from(display_text)).render(input_inner, frame.buffer_mut());
        let cursor_x = input_inner.x + cursor_pos as u16;
        let cursor_y = input_inner.y;
        frame.set_cursor_position((cursor_x.min(input_inner.x + input_inner.width - 1), cursor_y));
        let help = Paragraph::new("Enter: Save | Esc: Cancel")
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Center);
        help.render(chunks[2], frame.buffer_mut());
    }
    fn render_input_popup(&self, frame: &mut Frame) {
        let popup_area = centered_popup(frame.area(), 70, 40, 50, 10, 90, 20);
        Clear.render(popup_area, frame.buffer_mut());
        let block = Block::default()
            .borders(Borders::ALL)
            .title(" Add Repository ")
            .title_alignment(Alignment::Center)
            .border_style(Style::default().fg(Color::Green));
        let inner = block.inner(popup_area);
        block.render(popup_area, frame.buffer_mut());
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(2), // Instructions
                Constraint::Length(3), // Input field
                Constraint::Min(1),    // Already added repos
                Constraint::Length(1), // Help line
            ])
            .split(inner);
        let instructions =
            Paragraph::new(vec![Line::from("Enter repository URL (supports multi-line paste):")]).style(Style::default().fg(Color::Gray));
        instructions.render(chunks[0], frame.buffer_mut());
        let input_block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan));
        let input_inner = input_block.inner(chunks[1]);
        input_block.render(chunks[1], frame.buffer_mut());
        let display_text = if self.input_state.buffer.is_empty() {
            Span::styled(&self.input_state.placeholder, Style::default().fg(Color::DarkGray))
        } else {
            Span::styled(&self.input_state.buffer, Style::default().fg(Color::White))
        };
        Paragraph::new(Line::from(display_text)).render(input_inner, frame.buffer_mut());
        let cursor_x = input_inner.x + self.input_state.cursor as u16;
        let cursor_y = input_inner.y;
        frame.set_cursor_position((cursor_x.min(input_inner.x + input_inner.width - 1), cursor_y));
        if !self.input_state.lines.is_empty() {
            let added_text = self
                .input_state
                .lines
                .iter()
                .take(3)
                .map(|url| Line::from(Span::styled(format!("  🗂️ {}", url), Style::default().fg(Color::Green))))
                .collect::<Vec<_>>();
            let added_count = self.input_state.lines.len();
            let mut lines = added_text;
            if added_count > 3 {
                lines.push(Line::from(Span::styled(
                    format!("  ... and {} more", added_count - 3),
                    Style::default().fg(Color::DarkGray),
                )));
            }
            Paragraph::new(lines).render(chunks[2], frame.buffer_mut());
        }
        let help = Paragraph::new("Enter: Add URL | Esc: Finish | Paste: Multi-line supported")
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Center);
        help.render(chunks[3], frame.buffer_mut());
    }
    fn render_clone_config_popup(&self, frame: &mut Frame) {
        let popup_area = centered_popup(frame.area(), 75, 70, 60, 15, 100, 30);
        Clear.render(popup_area, frame.buffer_mut());
        let block = Block::default()
            .borders(Borders::ALL)
            .title(" Clone Configuration - Edit Paths ")
            .title_alignment(Alignment::Center)
            .border_style(Style::default().fg(Color::Yellow));
        let inner = block.inner(popup_area);
        block.render(popup_area, frame.buffer_mut());
        let repos_to_clone: Vec<(String, PathBuf)> = {
            let repos = self.repos.read();
            let selected: Vec<_> = repos
                .iter()
                .filter(|r| r.selected)
                .map(|r| (r.name.clone(), r.path.clone()))
                .collect();
            if selected.is_empty() {
                repos.iter().map(|r| (r.name.clone(), r.path.clone())).collect()
            } else {
                selected
            }
        };
        let repo_count = repos_to_clone.len();
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(2), // Instructions
                Constraint::Min(5),    // Repository list with paths
                Constraint::Length(1), // Help line
            ])
            .split(inner);
        let instructions = if self.clone_config.editing {
            Paragraph::new("Editing path - type to modify, Enter to save, Esc to cancel").style(Style::default().fg(Color::Cyan))
        } else {
            Paragraph::new("Navigate with ↑/↓, press 'e' or Tab to edit path, Enter to clone").style(Style::default().fg(Color::Gray))
        };
        instructions.render(chunks[0], frame.buffer_mut());
        let list_block = Block::default()
            .borders(Borders::ALL)
            .title(format!(" Repositories ({}) ", repo_count))
            .border_style(Style::default().fg(Color::DarkGray));
        let list_inner = list_block.inner(chunks[1]);
        list_block.render(chunks[1], frame.buffer_mut());
        let mut lines: Vec<Line> = Vec::new();
        for (idx, (repo_name, repo_path)) in repos_to_clone.iter().enumerate() {
            let is_selected = idx == self.clone_config.selected_repo_index;
            let prefix = if is_selected { "▶ " } else { "  " };
            let name_style = if is_selected {
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };
            lines.push(Line::from(vec![Span::styled(prefix, name_style), Span::styled(repo_name.clone(), name_style)]));
            let path_str = repo_path.to_string_lossy();
            if is_selected && self.clone_config.editing {
                let buffer = &self.input_state.buffer;
                let cursor_pos = self.input_state.cursor.min(buffer.len());
                let (before, after) = buffer.split_at(cursor_pos);
                lines.push(Line::from(vec![
                    Span::styled("    Path: ", Style::default().fg(Color::DarkGray)),
                    Span::styled(before, Style::default().fg(Color::Cyan)),
                    Span::styled("│", Style::default().fg(Color::White).add_modifier(Modifier::SLOW_BLINK)),
                    Span::styled(after, Style::default().fg(Color::Cyan)),
                ]));
            } else {
                let path_style = if is_selected {
                    Style::default().fg(Color::Green)
                } else {
                    Style::default().fg(Color::DarkGray)
                };
                lines.push(Line::from(vec![
                    Span::styled("    Path: ", Style::default().fg(Color::DarkGray)),
                    Span::styled(path_str.into_owned(), path_style),
                ]));
            }
            if idx < repos_to_clone.len() - 1 {
                lines.push(Line::from(""));
            }
        }
        let visible_height = list_inner.height as usize;
        let lines_per_repo = 3; // name + path + spacing
        let selected_line = self.clone_config.selected_repo_index * lines_per_repo;
        let scroll_offset = if selected_line >= visible_height {
            selected_line.saturating_sub(visible_height / 2)
        } else {
            0
        };
        let list_widget = Paragraph::new(lines).scroll((scroll_offset as u16, 0));
        list_widget.render(list_inner, frame.buffer_mut());
        let help = if self.clone_config.editing {
            Paragraph::new("Type to edit | Enter: Save | Esc: Cancel")
        } else {
            Paragraph::new("↑/↓: Navigate | e/Tab: Edit path | Enter: Clone | Esc: Cancel")
        };
        help.style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Center)
            .render(chunks[2], frame.buffer_mut());
    }
    fn render_env_vars_popup(&self, frame: &mut Frame) {
        let popup_area = centered_popup(frame.area(), 85, 80, 60, 15, 120, 40);
        Clear.render(popup_area, frame.buffer_mut());
        let repo_name = {
            let repos = self.repos.read();
            self.repo_table_state
                .selected()
                .and_then(|idx| repos.get(idx))
                .map_or_else(|| "Unknown".to_string(), |r| r.name.clone())
        };
        let block = Block::default()
            .borders(Borders::ALL)
            .title(format!(" Edit .env - {} ", repo_name))
            .title_alignment(Alignment::Center)
            .border_style(Style::default().fg(Color::Green));
        let inner = block.inner(popup_area);
        block.render(popup_area, frame.buffer_mut());
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(2), // Instructions
                Constraint::Min(5),    // Text area
                Constraint::Length(1), // Help line
            ])
            .split(inner);
        let instructions = Paragraph::new(vec![Line::from("Enter environment variables (KEY=value format, one per line):")])
            .style(Style::default().fg(Color::Gray));
        instructions.render(chunks[0], frame.buffer_mut());
        let text_block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan));
        let text_inner = text_block.inner(chunks[1]);
        text_block.render(chunks[1], frame.buffer_mut());
        let buffer = &self.input_state.buffer;
        let cursor_pos = self.input_state.cursor;
        let before_cursor = &buffer[..cursor_pos.min(buffer.len())];
        let lines_before: Vec<&str> = before_cursor.split('\n').collect();
        let cursor_line = lines_before.len().saturating_sub(1);
        let cursor_col = lines_before.last().map_or(0, |l| l.len());
        let visible_height = text_inner.height.saturating_sub(1) as usize;
        let scroll_y = if cursor_line >= visible_height {
            cursor_line.saturating_sub(visible_height / 2)
        } else {
            0
        };
        let display_text = if self.input_state.buffer.is_empty() {
            Text::styled(&self.input_state.placeholder, Style::default().fg(Color::DarkGray))
        } else {
            let lines: Vec<Line> = self
                .input_state
                .buffer
                .lines()
                .enumerate()
                .map(|(line_idx, line)| {
                    let line_num = format!("{:3} ", line_idx + 1);
                    let mut spans = vec![Span::styled(line_num, Style::default().fg(Color::DarkGray))];
                    if let Some(eq_pos) = line.find('=') {
                        let key = &line[..eq_pos];
                        let value = &line[eq_pos + 1..];
                        if key.trim_start().starts_with('#') {
                            spans.push(Span::styled(line, Style::default().fg(Color::DarkGray)));
                        } else {
                            spans.push(Span::styled(key, Style::default().fg(Color::Yellow)));
                            spans.push(Span::styled("=", Style::default().fg(Color::White)));
                            spans.push(Span::styled(value, Style::default().fg(Color::Green)));
                        }
                    } else if line.starts_with('#') {
                        spans.push(Span::styled(line, Style::default().fg(Color::DarkGray)));
                    } else {
                        spans.push(Span::styled(line, Style::default().fg(Color::White)));
                    }
                    Line::from(spans)
                })
                .collect();
            Text::from(lines)
        };
        Paragraph::new(display_text)
            .scroll((scroll_y as u16, 0))
            .render(text_inner, frame.buffer_mut());
        let help = Paragraph::new("F2: Save | Enter: New line | Esc: Cancel | ↑↓←→: Navigate | Paste supported")
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Center);
        help.render(chunks[2], frame.buffer_mut());
        let cursor_x = text_inner.x + 4 + cursor_col as u16; // 4 for line number
        let cursor_y = text_inner.y + (cursor_line - scroll_y) as u16;
        if cursor_y < text_inner.y + text_inner.height && cursor_x < text_inner.x + text_inner.width {
            frame.set_cursor_position((cursor_x, cursor_y));
        }
    }
    fn render_required_files_popup(&self, frame: &mut Frame) {
        let popup_area = centered_popup(frame.area(), 60, 60, 40, 10, 70, 25);
        Clear.render(popup_area, frame.buffer_mut());
        let repo_name = {
            let repos = self.repos.read();
            self.repo_table_state
                .selected()
                .and_then(|idx| repos.get(idx))
                .map_or_else(|| "Unknown".to_string(), |r| r.name.clone())
        };
        let block = Block::default()
            .borders(Borders::ALL)
            .title(format!(" Required Files - {} ", repo_name))
            .title_alignment(Alignment::Center)
            .border_style(Style::default().fg(Color::Magenta));
        let inner = block.inner(popup_area);
        block.render(popup_area, frame.buffer_mut());
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(2), // Instructions
                Constraint::Min(3),    // Text area
                Constraint::Length(1), // Help line
            ])
            .split(inner);
        let instructions =
            Paragraph::new("Enter required files (one per line, e.g. .env, docker-compose.yml):").style(Style::default().fg(Color::Gray));
        instructions.render(chunks[0], frame.buffer_mut());
        let text_block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan));
        let text_inner = text_block.inner(chunks[1]);
        text_block.render(chunks[1], frame.buffer_mut());
        let display_text = if self.input_state.buffer.is_empty() {
            Text::styled(&self.input_state.placeholder, Style::default().fg(Color::DarkGray))
        } else {
            let lines: Vec<Line> = self
                .input_state
                .buffer
                .lines()
                .map(|line| {
                    let trimmed = line.trim();
                    if trimmed.is_empty() {
                        Line::styled(line, Style::default().fg(Color::DarkGray))
                    } else {
                        Line::styled(line, Style::default().fg(Color::Cyan))
                    }
                })
                .collect();
            Text::from(lines)
        };
        Paragraph::new(display_text)
            .wrap(Wrap { trim: false })
            .render(text_inner, frame.buffer_mut());
        let help = Paragraph::new("F2/Ctrl+Enter: Save | Enter: New line | Esc: Cancel")
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Center);
        help.render(chunks[2], frame.buffer_mut());
        let buffer = &self.input_state.buffer;
        let cursor_pos = self.input_state.cursor;
        let before_cursor = &buffer[..cursor_pos.min(buffer.len())];
        let lines_before: Vec<&str> = before_cursor.split('\n').collect();
        let cursor_line = lines_before.len().saturating_sub(1);
        let cursor_col = lines_before.last().map_or(0, |l| l.len());
        let cursor_x = text_inner.x + cursor_col as u16;
        let cursor_y = text_inner.y + cursor_line as u16;
        if cursor_y < text_inner.y + text_inner.height && cursor_x < text_inner.x + text_inner.width {
            frame.set_cursor_position((cursor_x, cursor_y));
        }
    }
    fn render_repo_path_popup(&self, frame: &mut Frame) {
        let popup_area = centered_popup(frame.area(), 70, 30, 50, 8, 90, 12);
        Clear.render(popup_area, frame.buffer_mut());
        let repo_info = {
            let repos = self.repos.read();
            self.editing_repo_id.and_then(|id| repos.iter().find(|r| r.id == id)).map(|r| {
                let is_cloned = r.path.join(".git").exists();
                (r.name.clone(), r.path.clone(), is_cloned)
            })
        };
        let (repo_name, current_path, is_cloned) = repo_info.unwrap_or_else(|| ("Unknown".to_string(), PathBuf::new(), false));
        let title = format!(" Edit Path - {} ", repo_name);
        let block = Block::default()
            .borders(Borders::ALL)
            .title(title)
            .title_alignment(Alignment::Center)
            .border_style(Style::default().fg(Color::Yellow));
        let inner = block.inner(popup_area);
        block.render(popup_area, frame.buffer_mut());
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(2), // Current path info
                Constraint::Length(3), // Input field
                Constraint::Min(1),    // Help line
            ])
            .split(inner);
        let status_icon = if is_cloned { "✓ Cloned" } else { "○ Not cloned" };
        let info_text = format!("Current: {} ({})", current_path.display(), status_icon);
        let info = Paragraph::new(info_text).style(Style::default().fg(Color::Gray));
        info.render(chunks[0], frame.buffer_mut());
        let input_block = Block::default()
            .borders(Borders::ALL)
            .title(" New Path ")
            .border_style(Style::default().fg(Color::Cyan));
        let input_inner = input_block.inner(chunks[1]);
        input_block.render(chunks[1], frame.buffer_mut());
        let display_text = if self.input_state.buffer.is_empty() {
            Span::styled(&self.input_state.placeholder, Style::default().fg(Color::DarkGray))
        } else {
            Span::styled(&self.input_state.buffer, Style::default().fg(Color::White))
        };
        Paragraph::new(Line::from(display_text)).render(input_inner, frame.buffer_mut());
        let help = Paragraph::new("Enter: Save | Esc: Cancel | Use absolute or relative path")
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Center);
        help.render(chunks[2], frame.buffer_mut());
        let cursor_x = input_inner.x + self.input_state.cursor.min(input_inner.width as usize - 1) as u16;
        let cursor_y = input_inner.y;
        frame.set_cursor_position((cursor_x, cursor_y));
    }
    fn render_docker_compose_popup(&self, frame: &mut Frame) {
        if self.docker_compose_state.editing_env {
            self.render_env_file_editor(frame);
            return;
        }
        let popup_area = centered_popup(frame.area(), 90, 85, 70, 20, 120, 45);
        Clear.render(popup_area, frame.buffer_mut());
        let (repo_name, docker_status, env_status) = {
            let repos = self.repos.read();
            self.repo_table_state.selected().and_then(|idx| repos.get(idx)).map_or_else(
                || ("Unknown".to_string(), DockerComposeStatus::default(), EnvStatus::default()),
                |r| (r.name.clone(), r.docker_compose_status.clone(), r.env_status.clone()),
            )
        };
        let block = Block::default()
            .borders(Borders::ALL)
            .title(format!(" 🐳 Docker Compose - {} ", repo_name))
            .title_alignment(Alignment::Center)
            .border_style(Style::default().fg(Color::Blue));
        let inner = block.inner(popup_area);
        block.render(popup_area, frame.buffer_mut());
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(2), // Status summary
                Constraint::Min(5),    // Services list
                Constraint::Length(3), // Help line
            ])
            .split(inner);
        let sample_icon = if docker_status.sample_exists { "✅" } else { "❌" };
        let compose_icon = if docker_status.compose_exists { "✅" } else { "❌" };
        let env_file_icon = if env_status.sample_file_exists { "✅" } else { "⚠️" };
        let status_line = Line::from(vec![
            Span::styled("Sample: ", Style::default().fg(Color::Gray)),
            Span::raw(sample_icon),
            Span::raw("  "),
            Span::styled("Compose: ", Style::default().fg(Color::Gray)),
            Span::raw(compose_icon),
            Span::raw("  "),
            Span::styled(".env: ", Style::default().fg(Color::Gray)),
            Span::raw(env_file_icon),
            Span::raw("  "),
            Span::styled("Services: ", Style::default().fg(Color::Gray)),
            Span::styled(format!("{}", docker_status.services.len()), Style::default().fg(Color::Cyan)),
        ]);
        Paragraph::new(status_line)
            .alignment(Alignment::Center)
            .render(chunks[0], frame.buffer_mut());
        let services_block = Block::default()
            .borders(Borders::ALL)
            .title(" Services ")
            .border_style(Style::default().fg(Color::Cyan));
        let services_inner = services_block.inner(chunks[1]);
        services_block.render(chunks[1], frame.buffer_mut());
        if docker_status.services.is_empty() {
            let empty_msg = if docker_status.sample_exists || docker_status.compose_exists {
                "No services found in docker-compose file"
            } else {
                "No docker-compose file found in this repository"
            };
            Paragraph::new(empty_msg)
                .style(Style::default().fg(Color::DarkGray))
                .alignment(Alignment::Center)
                .render(services_inner, frame.buffer_mut());
        } else {
            let mut service_lines: Vec<Line> = Vec::new();
            let selected_idx = self.docker_compose_state.selected_service;
            let mut selected_line_start = 0_usize; // Track where the selected service line starts
            for (idx, service) in docker_status.services.iter().enumerate() {
                let is_selected = idx == selected_idx;
                let is_expanded = self.docker_compose_state.is_expanded(idx);
                if is_selected {
                    selected_line_start = service_lines.len();
                }
                let select_indicator = if is_selected { "▶ " } else { "  " };
                let expand_arrow = if is_expanded { "▼ " } else { "▷ " };
                let env_indicator = if service.has_environment { "⚙️" } else { "  " };
                let file_indicator = if service.has_env_file { "📄" } else { "  " };
                let style = if is_selected {
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::White)
                };
                let service_line = Line::from(vec![
                    Span::styled(select_indicator, Style::default().fg(Color::Cyan)),
                    Span::styled(expand_arrow, Style::default().fg(Color::DarkGray)),
                    Span::styled(&service.name, style),
                    Span::raw("  "),
                    Span::raw(env_indicator),
                    Span::styled(
                        if service.has_environment {
                            format!(" {} vars", service.environment.len())
                        } else {
                            String::new()
                        },
                        Style::default().fg(Color::Cyan),
                    ),
                    Span::raw("  "),
                    Span::raw(file_indicator),
                    Span::styled(
                        if service.has_env_file {
                            format!(" {}", service.env_file.join(", "))
                        } else {
                            String::new()
                        },
                        Style::default().fg(Color::Green),
                    ),
                ]);
                service_lines.push(service_line);
                if is_expanded {
                    if service.has_environment && !service.environment.is_empty() {
                        service_lines.push(Line::from(vec![
                            Span::raw("      "),
                            Span::styled("── Environment Variables ──", Style::default().fg(Color::DarkGray)),
                        ]));
                        for (key, value) in &service.environment {
                            let status_icon = if env_status.missing_vars.contains(key) {
                                Span::styled("❌ ", Style::default().fg(Color::Red))
                            } else if env_status.empty_vars.contains(key) {
                                Span::styled("⚠️ ", Style::default().fg(Color::Yellow))
                            } else {
                                Span::styled("✓ ", Style::default().fg(Color::Green))
                            };
                            let var_line = Line::from(vec![
                                Span::raw("        "),
                                status_icon,
                                Span::styled(key, Style::default().fg(Color::Cyan)),
                                Span::styled("=", Style::default().fg(Color::DarkGray)),
                                Span::styled(value, Style::default().fg(Color::White)),
                            ]);
                            service_lines.push(var_line);
                        }
                    }
                    if service.has_env_file && !service.env_file.is_empty() {
                        service_lines.push(Line::from(vec![
                            Span::raw("      "),
                            Span::styled("── env_file References ──", Style::default().fg(Color::DarkGray)),
                        ]));
                        for file in &service.env_file {
                            service_lines.push(Line::from(vec![
                                Span::raw("        "),
                                Span::styled("📁 ", Style::default().fg(Color::Green)),
                                Span::styled(file, Style::default().fg(Color::White)),
                            ]));
                        }
                    }
                    if let Some(missing) = docker_status.missing_env_vars.get(&service.name).filter(|m| !m.is_empty()) {
                        service_lines.push(Line::from(vec![
                            Span::raw("      "),
                            Span::styled(format!("⚠️  {} var(s) missing in .env", missing.len()), Style::default().fg(Color::Yellow)),
                        ]));
                    }
                    service_lines.push(Line::from("")); // Spacing
                }
            }
            let visible_height = services_inner.height as usize;
            let total_lines = service_lines.len();
            let scroll_offset = if selected_line_start >= visible_height {
                selected_line_start.saturating_sub(visible_height / 3)
            } else {
                0
            }
            .min(total_lines.saturating_sub(visible_height));
            Paragraph::new(service_lines)
                .scroll((scroll_offset as u16, 0))
                .render(services_inner, frame.buffer_mut());
        }
        let selected_service = docker_status.services.get(self.docker_compose_state.selected_service);
        let has_env = selected_service.is_some_and(|s| s.has_environment);
        let has_env_file = selected_service.is_some_and(|s| s.has_env_file);
        let mut help_parts = vec!["↑↓/jk: Select", "←→/hl: Collapse/Expand"];
        if has_env && !has_env_file {
            help_parts.push("m: Move to .env file");
        }
        if has_env_file {
            help_parts.push("i: Inline env");
            help_parts.push("e: Edit .env");
        }
        help_parts.push("Esc: Close");
        let help_text = help_parts.join(" | ");
        let help = Paragraph::new(help_text)
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: true });
        help.render(chunks[2], frame.buffer_mut());
    }
    fn render_env_file_editor(&self, frame: &mut Frame) {
        let popup_area = centered_popup(frame.area(), 90, 90, 70, 20, 130, 50);
        Clear.render(popup_area, frame.buffer_mut());
        let title = if !self.input_state.placeholder.is_empty() {
            format!(" ✏️  {} ", self.input_state.placeholder)
        } else {
            " ✏️  Edit .env File ".to_string()
        };
        let block = Block::default()
            .borders(Borders::ALL)
            .title(title)
            .title_alignment(Alignment::Center)
            .border_style(Style::default().fg(Color::Green));
        let inner = block.inner(popup_area);
        block.render(popup_area, frame.buffer_mut());
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(3),    // Editor content
                Constraint::Length(2), // Help line
            ])
            .split(inner);
        let content = &self.input_state.buffer;
        let cursor_pos = self.input_state.cursor;
        let before_cursor = &content[..cursor_pos];
        let cursor_line = before_cursor.matches('\n').count();
        let cursor_col = before_cursor.rfind('\n').map_or(cursor_pos, |p| cursor_pos - p - 1);
        let lines: Vec<Line> = content
            .lines()
            .enumerate()
            .map(|(line_idx, line)| {
                let line_num = format!("{:3} ", line_idx + 1);
                let mut spans = vec![Span::styled(line_num, Style::default().fg(Color::DarkGray))];
                if let Some(eq_pos) = line.find('=') {
                    let key = &line[..eq_pos];
                    let value = &line[eq_pos + 1..];
                    if key.trim_start().starts_with('#') {
                        spans.push(Span::styled(line, Style::default().fg(Color::DarkGray)));
                    } else {
                        spans.push(Span::styled(key, Style::default().fg(Color::Cyan)));
                        spans.push(Span::styled("=", Style::default().fg(Color::Yellow)));
                        spans.push(Span::styled(value, Style::default().fg(Color::White)));
                    }
                } else if line.trim_start().starts_with('#') {
                    spans.push(Span::styled(line, Style::default().fg(Color::DarkGray)));
                } else {
                    spans.push(Span::styled(line, Style::default().fg(Color::White)));
                }
                Line::from(spans)
            })
            .collect();
        let visible_height = chunks[0].height.saturating_sub(1) as usize;
        let scroll_y = if cursor_line >= visible_height {
            cursor_line.saturating_sub(visible_height / 2)
        } else {
            0
        };
        let editor_block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray));
        let editor_inner = editor_block.inner(chunks[0]);
        editor_block.render(chunks[0], frame.buffer_mut());
        Paragraph::new(lines)
            .scroll((scroll_y as u16, 0))
            .render(editor_inner, frame.buffer_mut());
        let cursor_x = editor_inner.x + 4 + cursor_col as u16; // 4 for line number
        let cursor_y = editor_inner.y + (cursor_line - scroll_y) as u16;
        if cursor_x < editor_inner.x + editor_inner.width && cursor_y < editor_inner.y + editor_inner.height {
            frame.set_cursor_position((cursor_x, cursor_y));
        }
        let help_text = "Ctrl+Enter: Save | Esc: Cancel | Arrow keys: Navigate";
        let help = Paragraph::new(help_text)
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Center);
        help.render(chunks[1], frame.buffer_mut());
    }
    fn render_header(&self, frame: &mut Frame, area: Rect) {
        let title = Line::from(vec![
            Span::styled(" EXPRESSO", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::styled("-", Style::default().fg(Color::White)),
            Span::styled("KIT", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::styled(" | ", Style::default().fg(Color::DarkGray)),
            Span::styled("Project Validator Manager", Style::default().fg(Color::White)),
        ]);
        let subtitle = Line::from(vec![
            Span::styled(" Status: ", Style::default().fg(Color::Gray)),
            Span::styled("Online", Style::default().fg(Color::Green)),
            Span::raw(" | "),
            Span::styled("Last Update: ", Style::default().fg(Color::Gray)),
            Span::styled(self.last_update.format("%H:%M:%S").to_string(), Style::default().fg(Color::Blue)),
        ]);
        let block = Block::default()
            .borders(Borders::RIGHT)
            .border_style(Style::default().fg(Color::DarkGray));
        let inner = block.inner(area);
        block.render(area, frame.buffer_mut());
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Length(1)])
            .split(inner);
        Paragraph::new(title)
            .alignment(Alignment::Left)
            .render(chunks[0], frame.buffer_mut());
        Paragraph::new(subtitle)
            .alignment(Alignment::Left)
            .render(chunks[1], frame.buffer_mut());
    }
    fn render_tabs(&self, frame: &mut Frame, area: Rect) {
        let tabs = Tabs::new(vec!["📁 Repositories", "🧰 Environment", "⚙️ Workflows", "📓 Logs"])
            .block(Block::default().borders(Borders::BOTTOM))
            .select(match self.current_tab {
                DashboardTab::Repositories => 0,
                DashboardTab::Environment => 1,
                DashboardTab::Workflows => 2,
                DashboardTab::Logs => 3,
            })
            .style(Style::default().fg(Color::White))
            .highlight_style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
            .divider(" | ");
        tabs.render(area, frame.buffer_mut());
    }
    fn render_repositories_tab(&mut self, frame: &mut Frame, area: Rect) {
        let repos = self.repos.read();
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
            .split(area);
        if repos.is_empty() {
            let empty_block = Block::default()
                .borders(Borders::ALL)
                .title(" Repositories ")
                .title_alignment(Alignment::Center)
                .border_style(Style::default().fg(Color::Cyan));
            let inner = empty_block.inner(chunks[0]);
            empty_block.render(chunks[0], frame.buffer_mut());
            let content_height = 6;
            let start_y = inner.y + inner.height.saturating_sub(content_height) / 2;
            let empty_message = vec![
                Line::from(Span::styled(
                    "📁 No repositories added yet",
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
                )),
                Line::from(""),
                Line::from(Span::styled("Press 'a' to add git repository URLs", Style::default().fg(Color::Green))),
                Line::from(""),
                Line::from(Span::styled("You can paste single or multiple URLs", Style::default().fg(Color::DarkGray))),
                Line::from(Span::styled("(one per line)", Style::default().fg(Color::DarkGray))),
            ];
            let centered_area = Rect::new(inner.x, start_y, inner.width, content_height);
            Paragraph::new(empty_message)
                .alignment(Alignment::Center)
                .render(centered_area, frame.buffer_mut());
        } else {
            RepoTable { repos: &repos, state: &mut self.repo_table_state }.render(chunks[0], frame.buffer_mut());
        }
        let right_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(12), Constraint::Min(1)])
            .split(chunks[1]);
        ProgressGauges { repos: &repos }.render(right_chunks[0], frame.buffer_mut());
        let actions = Block::default()
            .borders(Borders::ALL)
            .title(" Quick Actions ")
            .title_alignment(Alignment::Center)
            .border_style(Style::default().fg(Color::Green));
        let inner = actions.inner(right_chunks[1]);
        actions.render(right_chunks[1], frame.buffer_mut());
        let action_text = if repos.is_empty() {
            vec![
                Line::from(Span::styled("a", Style::default().fg(Color::Cyan))).alignment(Alignment::Center),
                Line::from(" Add repositories").alignment(Alignment::Center),
                Line::from(""),
                Line::from("Tip: You can paste").alignment(Alignment::Center),
                Line::from("multiple URLs at once!").alignment(Alignment::Center),
            ]
        } else {
            vec![
                Line::from(vec![
                    Span::styled("a", Style::default().fg(Color::Cyan)),
                    Span::raw(" Add repositories"),
                ])
                .alignment(Alignment::Center),
                Line::from(vec![
                    Span::styled("SPACE", Style::default().fg(Color::Cyan)),
                    Span::raw(" Toggle selection"),
                ])
                .alignment(Alignment::Center),
                Line::from(vec![
                    Span::styled("ENTER", Style::default().fg(Color::Yellow)),
                    Span::raw(" Validate selected"),
                ])
                .alignment(Alignment::Center),
                Line::from(vec![
                    Span::styled("c", Style::default().fg(Color::Cyan)),
                    Span::raw(" Clone selected"),
                ])
                .alignment(Alignment::Center),
                Line::from(vec![
                    Span::styled("v", Style::default().fg(Color::Cyan)),
                    Span::raw(" Validate env only"),
                ])
                .alignment(Alignment::Center),
                Line::from(vec![
                    Span::styled("Del", Style::default().fg(Color::Cyan)),
                    Span::raw(" Remove selected"),
                ])
                .alignment(Alignment::Center),
            ]
        };
        Paragraph::new(action_text)
            .alignment(Alignment::Center)
            .render(inner, frame.buffer_mut());
    }
    fn render_environment_tab(&self, frame: &mut Frame, area: Rect) {
        let repos = self.repos.read();
        if repos.is_empty() {
            let block = Block::default()
                .borders(Borders::ALL)
                .title(" Environment Validation ")
                .title_alignment(Alignment::Center)
                .border_style(Style::default().fg(Color::Cyan));
            let inner = block.inner(area);
            block.render(area, frame.buffer_mut());
            let content_height = 12;
            let start_y = inner.y + inner.height.saturating_sub(content_height) / 2;
            let centered_area = Rect::new(inner.x, start_y, inner.width, content_height);
            let empty_message = vec![
                Line::from(Span::styled(
                    "🔠 No repositories to validate",
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
                )),
                Line::from(""),
                Line::from(Span::styled(
                    "Add repositories first (press 'a' in Repositories tab)",
                    Style::default().fg(Color::Gray),
                )),
                Line::from(""),
                Line::from(Span::styled("Environment validation compares:", Style::default().fg(Color::Cyan))),
                Line::from(Span::styled(".env.sample (template) ↔ .env (actual)", Style::default().fg(Color::DarkGray))),
                Line::from(""),
                Line::from(Span::styled("❌ Missing = key not in .env", Style::default().fg(Color::Red))),
                Line::from(Span::styled("⚠️  Empty = key exists but has no value", Style::default().fg(Color::Yellow))),
            ];
            Paragraph::new(empty_message)
                .alignment(Alignment::Center)
                .render(centered_area, frame.buffer_mut());
            return;
        }
        let selected_repo = self.repo_table_state.selected().and_then(|idx| repos.get(idx));
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)])
            .split(area);
        let env_block = Block::default()
            .borders(Borders::ALL)
            .title(" Environment Validation ")
            .title_alignment(Alignment::Center)
            .border_style(Style::default().fg(Color::Cyan));
        env_block.clone().render(chunks[0], frame.buffer_mut());
        let env_inner = env_block.inner(chunks[0]);
        if let Some(repo) = selected_repo {
            let (sample_status, sample_desc) = if repo.env_status.sample_file_exists {
                (
                    Span::styled("✅ Found", Style::default().fg(Color::Green)),
                    "Reference file for environment variables",
                )
            } else {
                (
                    Span::styled("❌ Not found", Style::default().fg(Color::Red)),
                    "No reference file. Create .env.sample to enable validation",
                )
            };
            let (missing_count, missing_style, missing_desc) = {
                let count = repo.env_status.missing_vars.len();
                if count == 0 {
                    (count, Style::default().fg(Color::Green), "All required vars defined")
                } else {
                    (count, Style::default().fg(Color::Red), "Vars in .env.sample but not in .env")
                }
            };
            let (empty_count, empty_style, empty_desc) = {
                let count = repo.env_status.empty_vars.len();
                if count == 0 {
                    (count, Style::default().fg(Color::Green), "All vars have values")
                } else {
                    (count, Style::default().fg(Color::Yellow), "Vars defined but empty (no value)")
                }
            };
            let required_missing = repo.missing_files.len();
            let required_total = repo.required_files.len();
            let (req_style, req_desc) = if required_missing == 0 {
                (Style::default().fg(Color::Green), "All required files present")
            } else {
                (Style::default().fg(Color::Red), "Some required files are missing")
            };
            let has_docker_compose = repo.docker_compose_status.compose_exists || repo.docker_compose_status.sample_exists;
            let dc_sample_status = if repo.docker_compose_status.sample_exists {
                Span::styled("✅", Style::default().fg(Color::Green))
            } else {
                Span::styled("—", Style::default().fg(Color::DarkGray))
            };
            let dc_compose_status = if repo.docker_compose_status.compose_exists {
                Span::styled("✅", Style::default().fg(Color::Green))
            } else if repo.docker_compose_status.sample_exists {
                Span::styled("❌", Style::default().fg(Color::Red))
            } else {
                Span::styled("—", Style::default().fg(Color::DarkGray))
            };
            let dc_services_count = repo.docker_compose_status.services.len();
            let dc_with_env = repo.docker_compose_status.services.iter().filter(|s| s.has_environment).count();
            let mut env_lines = vec![
                Line::from(vec![
                    Span::raw("Repository: "),
                    Span::styled(&repo.name, Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                ])
                .alignment(Alignment::Center),
                Line::from(""),
                Line::from(Span::styled("=== .env Validation ===", Style::default().fg(Color::Cyan))).alignment(Alignment::Center),
                Line::from(vec![Span::raw(".env.sample: "), sample_status]).alignment(Alignment::Center),
                Line::from(Span::styled(sample_desc, Style::default().fg(Color::DarkGray))).alignment(Alignment::Center),
                Line::from(""),
                Line::from(vec![
                    Span::raw("Missing: "),
                    Span::styled(format!("{}", missing_count), missing_style),
                    Span::raw(" | Empty: "),
                    Span::styled(format!("{}", empty_count), empty_style),
                ])
                .alignment(Alignment::Center),
                Line::from(Span::styled(
                    if missing_count > 0 { missing_desc } else { empty_desc },
                    Style::default().fg(Color::DarkGray),
                ))
                .alignment(Alignment::Center),
                Line::from(""),
                Line::from(Span::styled("=== Required Files ===", Style::default().fg(Color::Yellow))).alignment(Alignment::Center),
                Line::from(vec![
                    Span::raw("Files: "),
                    Span::styled(format!("{}/{}", required_total - required_missing, required_total), req_style),
                    if required_missing > 0 {
                        Span::styled(format!(" ({} missing)", required_missing), Style::default().fg(Color::Red))
                    } else {
                        Span::raw("")
                    },
                ])
                .alignment(Alignment::Center),
                Line::from(Span::styled(req_desc, Style::default().fg(Color::DarkGray))).alignment(Alignment::Center),
                Line::from(""),
                Line::from(Span::styled("=== Docker Compose ===", Style::default().fg(Color::Blue))).alignment(Alignment::Center),
            ];
            if has_docker_compose {
                env_lines.push(
                    Line::from(vec![
                        Span::raw("Sample: "),
                        dc_sample_status,
                        Span::raw(" | Compose: "),
                        dc_compose_status,
                    ])
                    .alignment(Alignment::Center),
                );
                if dc_services_count > 0 {
                    env_lines.push(
                        Line::from(vec![
                            Span::raw("Services: "),
                            Span::styled(format!("{}", dc_services_count), Style::default().fg(Color::Cyan)),
                            Span::raw(" ("),
                            Span::styled(format!("{}", dc_with_env), Style::default().fg(Color::Yellow)),
                            Span::raw(" with env)"),
                        ])
                        .alignment(Alignment::Center),
                    );
                }
                env_lines.push(Line::from(""));
                env_lines.push(
                    Line::from(Span::styled("Press 'd' for docker-compose details", Style::default().fg(Color::DarkGray)))
                        .alignment(Alignment::Center),
                );
            } else {
                env_lines.push(
                    Line::from(Span::styled("No docker-compose file found", Style::default().fg(Color::DarkGray)))
                        .alignment(Alignment::Center),
                );
                env_lines.push(
                    Line::from(Span::styled("Add docker-compose.yml to enable", Style::default().fg(Color::DarkGray)))
                        .alignment(Alignment::Center),
                );
            }
            Paragraph::new(env_lines)
                .alignment(Alignment::Center)
                .render(env_inner, frame.buffer_mut());
        } else {
            let content_height = 4;
            let start_y = env_inner.y + env_inner.height.saturating_sub(content_height) / 2;
            let centered_area = Rect::new(env_inner.x, start_y, env_inner.width, content_height);
            let help_message = vec![
                Line::from(Span::styled("Select a repository to validate", Style::default().fg(Color::Gray))),
                Line::from(""),
                Line::from(Span::styled("Use ↑↓ in Repositories tab", Style::default().fg(Color::DarkGray))),
            ];
            Paragraph::new(help_message)
                .alignment(Alignment::Center)
                .render(centered_area, frame.buffer_mut());
        }
        let details_block = Block::default()
            .borders(Borders::ALL)
            .title(" Variable Details ")
            .title_alignment(Alignment::Center)
            .border_style(Style::default().fg(Color::Magenta));
        details_block.clone().render(chunks[1], frame.buffer_mut());
        let details_inner = details_block.inner(chunks[1]);
        if let Some(repo) = selected_repo {
            let mut detail_lines: Vec<Line> = Vec::new();
            if repo.env_status.missing_vars.is_empty() && repo.env_status.empty_vars.is_empty() {
                if repo.env_status.sample_file_exists {
                    detail_lines.push(Line::from(Span::styled(
                        "✅ All variables are present and have values!",
                        Style::default().fg(Color::Green),
                    )));
                } else {
                    detail_lines.push(Line::from(Span::styled("No .env.sample file found", Style::default().fg(Color::Yellow))));
                    detail_lines.push(Line::from(""));
                    detail_lines.push(Line::from(Span::styled(
                        "Add a .env.sample file with required",
                        Style::default().fg(Color::DarkGray),
                    )));
                    detail_lines.push(Line::from(Span::styled(
                        "environment variable keys to enable",
                        Style::default().fg(Color::DarkGray),
                    )));
                    detail_lines.push(Line::from(Span::styled("validation.", Style::default().fg(Color::DarkGray))));
                }
            } else {
                if !repo.env_status.missing_vars.is_empty() {
                    detail_lines.push(Line::from(Span::styled(
                        "❌ Missing variables (ERROR):",
                        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                    )));
                    for var in &repo.env_status.missing_vars {
                        detail_lines.push(Line::from(Span::styled(format!("   {}", var), Style::default().fg(Color::Red))));
                    }
                    detail_lines.push(Line::from(""));
                }
                if !repo.env_status.empty_vars.is_empty() {
                    detail_lines.push(Line::from(Span::styled(
                        "⚠️ Empty variables (WARNING):",
                        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
                    )));
                    for var in &repo.env_status.empty_vars {
                        detail_lines.push(Line::from(Span::styled(format!("   {}", var), Style::default().fg(Color::Yellow))));
                    }
                }
            }
            if repo.docker_compose_status.sample_exists && !repo.docker_compose_status.compose_exists {
                detail_lines.push(Line::from(""));
                detail_lines.push(Line::from(Span::styled(
                    "🐳 Docker Compose:",
                    Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD),
                )));
                detail_lines.push(Line::from(Span::styled("   ❌ docker-compose.yml missing", Style::default().fg(Color::Red))));
                detail_lines.push(Line::from(Span::styled("   Press 'd' to copy from sample", Style::default().fg(Color::DarkGray))));
            } else if !repo.docker_compose_status.missing_env_vars.is_empty() {
                detail_lines.push(Line::from(""));
                detail_lines.push(Line::from(Span::styled(
                    "🐳 Docker Compose Issues:",
                    Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD),
                )));
                for (service, vars) in &repo.docker_compose_status.missing_env_vars {
                    if !vars.is_empty() {
                        detail_lines.push(Line::from(Span::styled(
                            format!("   Service '{}': {} missing vars", service, vars.len()),
                            Style::default().fg(Color::Yellow),
                        )));
                    }
                }
            }
            Paragraph::new(detail_lines)
                .wrap(Wrap { trim: true })
                .render(details_inner, frame.buffer_mut());
        } else {
            Paragraph::new("No repository selected")
                .alignment(Alignment::Center)
                .style(Style::default().fg(Color::Gray))
                .render(details_inner, frame.buffer_mut());
        }
    }
    fn render_logs_tab(&self, frame: &mut Frame, area: Rect) {
        let logs = self.logs.lock();
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(1)])
            .split(area);
        let header = Line::from(vec![
            Span::styled("Application Logs", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::raw(" | "),
            Span::styled("c - Clear logs", Style::default().fg(Color::Gray)),
        ]);
        Paragraph::new(header)
            .alignment(Alignment::Center)
            .render(chunks[0], frame.buffer_mut());
        let logs_vec: Vec<LogEntry> = logs.iter().cloned().collect();
        LogViewer { logs: &logs_vec }.render(chunks[1], frame.buffer_mut());
    }
    fn render_workflows_tab(&self, frame: &mut Frame, area: Rect) {
        let repos = self.repos.read();
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(area);
        let left_block = Block::default()
            .borders(Borders::ALL)
            .title(" Select Repository ")
            .title_alignment(Alignment::Center)
            .border_style(Style::default().fg(Color::Cyan));
        let inner_left = left_block.inner(chunks[0]);
        left_block.render(chunks[0], frame.buffer_mut());
        if repos.is_empty() {
            let empty_message = Paragraph::new(vec![
                Line::from(Span::styled("No repositories added", Style::default().fg(Color::Yellow))),
                Line::from(""),
                Line::from(Span::styled("Add repositories in the Repositories tab", Style::default().fg(Color::Gray))),
            ])
            .alignment(Alignment::Center);
            empty_message.render(inner_left, frame.buffer_mut());
        } else {
            let items: Vec<ListItem> = repos
                .iter()
                .enumerate()
                .map(|(idx, repo)| {
                    let detected = workflow_templates::detect_project_types(&repo.path);
                    let detected_str = detected.summary();
                    let style = if Some(idx) == self.repo_table_state.selected() {
                        Style::default().bg(Color::Blue).fg(Color::White).add_modifier(Modifier::BOLD)
                    } else {
                        Style::default()
                    };
                    ListItem::new(vec![
                        Line::from(Span::styled(&repo.name, style)),
                        Line::from(Span::styled(format!("  📦 {detected_str}"), Style::default().fg(Color::DarkGray))),
                    ])
                })
                .collect();
            let list = List::new(items).highlight_style(Style::default().bg(Color::Blue).fg(Color::White));
            list.render(inner_left, frame.buffer_mut());
        }
        let right_block = Block::default()
            .borders(Borders::ALL)
            .title(" Workflow Templates ")
            .title_alignment(Alignment::Center)
            .border_style(Style::default().fg(Color::Magenta));
        let inner_right = right_block.inner(chunks[1]);
        right_block.render(chunks[1], frame.buffer_mut());
        let templates_info = vec![
            Line::from(Span::styled(
                "Available Templates:",
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(vec![
                Span::styled("🦀 Rust    ", Style::default().fg(Color::Red)),
                Span::styled("- Build, test, clippy, fmt", Style::default().fg(Color::Gray)),
            ]),
            Line::from(vec![
                Span::styled("📦 Node.js ", Style::default().fg(Color::Green)),
                Span::styled("- Build, lint, test (18/20/22)", Style::default().fg(Color::Gray)),
            ]),
            Line::from(vec![
                Span::styled("🐍 Python  ", Style::default().fg(Color::Yellow)),
                Span::styled("- Lint, test (3.10/3.11/3.12)", Style::default().fg(Color::Gray)),
            ]),
            Line::from(vec![
                Span::styled("🐻‍❄️ Go      ", Style::default().fg(Color::Cyan)),
                Span::styled("- Build, test, lint", Style::default().fg(Color::Gray)),
            ]),
            Line::from(vec![
                Span::styled("🐳 Docker  ", Style::default().fg(Color::Blue)),
                Span::styled("- Build and test image", Style::default().fg(Color::Gray)),
            ]),
            Line::from(vec![
                Span::styled("🐋 Compose ", Style::default().fg(Color::Magenta)),
                Span::styled("- Validate and test services", Style::default().fg(Color::Gray)),
            ]),
            Line::from(""),
            Line::from(Span::styled(
                "Press 'w' or Enter to generate workflow",
                Style::default().fg(Color::Green).add_modifier(Modifier::ITALIC),
            )),
        ];
        Paragraph::new(templates_info)
            .wrap(Wrap { trim: false })
            .render(inner_right, frame.buffer_mut());
    }
    fn render_workflow_popup(&self, frame: &mut Frame) {
        let area = frame.area();
        let popup_width = area.width.saturating_sub(10).min(100);
        let popup_height = area.height.saturating_sub(6).min(40);
        let popup_x = (area.width.saturating_sub(popup_width)) / 2;
        let popup_y = (area.height.saturating_sub(popup_height)) / 2;
        let popup_area = Rect::new(popup_x, popup_y, popup_width, popup_height);
        Clear.render(popup_area, frame.buffer_mut());
        let popup_block = Block::default()
            .borders(Borders::ALL)
            .title(format!(" Generate Workflow - {} ", self.workflow_state.target_name))
            .title_alignment(Alignment::Center)
            .border_style(Style::default().fg(Color::Yellow));
        let inner = popup_block.inner(popup_area);
        popup_block.render(popup_area, frame.buffer_mut());
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
            .split(inner);
        let templates_block = Block::default()
            .borders(Borders::RIGHT)
            .title(" Templates ")
            .title_alignment(Alignment::Center);
        let templates_inner = templates_block.inner(chunks[0]);
        templates_block.render(chunks[0], frame.buffer_mut());
        let template_items: Vec<ListItem> = WorkflowTemplate::ALL
            .iter()
            .enumerate()
            .map(|(idx, template)| {
                let is_selected = idx == self.workflow_state.selected_template;
                let is_detected = match (template, &self.workflow_state.detected) {
                    (WorkflowTemplate::Rust, Some(d)) => d.rust,
                    (WorkflowTemplate::NodeJs, Some(d)) => d.node,
                    (WorkflowTemplate::Python, Some(d)) => d.python,
                    (WorkflowTemplate::Go, Some(d)) => d.go,
                    (WorkflowTemplate::Docker, Some(d)) => d.docker,
                    (WorkflowTemplate::DockerCompose, Some(d)) => d.docker_compose,
                    _ => false,
                };
                let icon = if is_detected { "✓ " } else { "  " };
                let style = if is_selected {
                    Style::default().bg(Color::Blue).fg(Color::White).add_modifier(Modifier::BOLD)
                } else if is_detected {
                    Style::default().fg(Color::Green)
                } else {
                    Style::default()
                };
                ListItem::new(Line::from(Span::styled(format!("{icon}{}", template.name()), style)))
            })
            .collect();
        List::new(template_items).render(templates_inner, frame.buffer_mut());
        let preview_block = Block::default()
            .title(
                if self.workflow_state.editing {
                    " Edit Workflow (Tab to exit edit) "
                } else {
                    " Preview (e to edit) "
                },
            )
            .title_alignment(Alignment::Center)
            .border_style(
                if self.workflow_state.editing {
                    Style::default().fg(Color::Yellow)
                } else {
                    Style::default()
                },
            );
        let preview_inner = preview_block.inner(chunks[1]);
        preview_block.render(chunks[1], frame.buffer_mut());
        let lines: Vec<Line> = self
            .workflow_state
            .workflow_content
            .lines()
            .skip(self.workflow_state.scroll_offset)
            .take(preview_inner.height as usize)
            .enumerate()
            .map(|(idx, line)| {
                let line_num = idx + self.workflow_state.scroll_offset + 1;
                let is_current = idx + self.workflow_state.scroll_offset == self.workflow_state.cursor_line;
                let line_style = if is_current && self.workflow_state.editing {
                    Style::default().bg(Color::DarkGray)
                } else {
                    Style::default()
                };
                Line::from(vec![
                    Span::styled(format!("{line_num:3} │ "), Style::default().fg(Color::DarkGray)),
                    Span::styled(line, line_style),
                ])
            })
            .collect();
        Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .render(preview_inner, frame.buffer_mut());
        let footer_area = Rect::new(popup_x, popup_y + popup_height - 1, popup_width, 1);
        let footer_text = "↑↓: Select template • e: Edit • s: Save • Esc: Cancel";
        Paragraph::new(Span::styled(footer_text, Style::default().fg(Color::DarkGray)))
            .alignment(Alignment::Center)
            .render(footer_area, frame.buffer_mut());
    }
    fn render_workflow_editor(&self, frame: &mut Frame) {
        let popup_area = centered_popup(frame.area(), 90, 90, 70, 20, 130, 50);
        Clear.render(popup_area, frame.buffer_mut());
        let title = format!(
            " ✏️  Edit Workflow - {} ",
            if !self.input_state.placeholder.is_empty() {
                &self.input_state.placeholder
            } else {
                &self.workflow_state.target_name
            }
        );
        let block = Block::default()
            .borders(Borders::ALL)
            .title(title)
            .title_alignment(Alignment::Center)
            .border_style(Style::default().fg(Color::Yellow));
        let inner = block.inner(popup_area);
        block.render(popup_area, frame.buffer_mut());
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(3),    // Editor content
                Constraint::Length(2), // Help line
            ])
            .split(inner);
        let content = &self.input_state.buffer;
        let cursor_pos = self.input_state.cursor;
        let before_cursor = &content[..cursor_pos.min(content.len())];
        let cursor_line = before_cursor.matches('\n').count();
        let cursor_col = before_cursor.rfind('\n').map_or(cursor_pos, |p| cursor_pos - p - 1);
        let lines: Vec<Line> = content
            .lines()
            .enumerate()
            .map(|(line_idx, line)| {
                let line_num = format!("{:3} ", line_idx + 1);
                let mut spans = vec![Span::styled(line_num, Style::default().fg(Color::DarkGray))];
                let trimmed = line.trim_start();
                if trimmed.starts_with('#') {
                    spans.push(Span::styled(line, Style::default().fg(Color::DarkGray)));
                } else if let Some(rest) = trimmed.strip_prefix("- ") {
                    let indent = line.len() - trimmed.len();
                    spans.push(Span::styled(&line[..indent], Style::default()));
                    spans.push(Span::styled("- ", Style::default().fg(Color::Cyan)));
                    spans.push(Span::styled(rest, Style::default().fg(Color::White)));
                } else if let Some(colon_pos) = trimmed.find(':') {
                    let indent = line.len() - trimmed.len();
                    let key = &trimmed[..colon_pos];
                    let rest = &trimmed[colon_pos..];
                    spans.push(Span::styled(&line[..indent], Style::default()));
                    let key_color = match key {
                        "name" | "on" | "jobs" | "steps" | "runs-on" | "uses" | "with" | "env" | "if" | "needs" => Color::Magenta,
                        "push" | "pull_request" | "workflow_dispatch" | "schedule" => Color::Blue,
                        "branches" | "paths" | "tags" => Color::Cyan,
                        _ => Color::Yellow,
                    };
                    spans.push(Span::styled(key, Style::default().fg(key_color)));
                    if rest.len() > 1 {
                        spans.push(Span::styled(":", Style::default().fg(Color::White)));
                        let value = &rest[1..];
                        if value.contains("${{") {
                            spans.push(Span::styled(value, Style::default().fg(Color::Green)));
                        } else {
                            spans.push(Span::styled(value, Style::default().fg(Color::White)));
                        }
                    } else {
                        spans.push(Span::styled(rest, Style::default().fg(Color::White)));
                    }
                } else {
                    spans.push(Span::styled(line, Style::default().fg(Color::White)));
                }
                Line::from(spans)
            })
            .collect();
        let visible_height = chunks[0].height.saturating_sub(3) as usize;
        let scroll_y = if cursor_line >= visible_height {
            cursor_line.saturating_sub(visible_height / 2)
        } else {
            0
        };
        let editor_block = Block::default()
            .borders(Borders::ALL)
            .title(" YAML ")
            .border_style(Style::default().fg(Color::DarkGray));
        let editor_inner = editor_block.inner(chunks[0]);
        editor_block.render(chunks[0], frame.buffer_mut());
        Paragraph::new(lines)
            .scroll((scroll_y as u16, 0))
            .render(editor_inner, frame.buffer_mut());
        let cursor_x = editor_inner.x + 4 + cursor_col as u16; // 4 for line number
        let cursor_y = editor_inner.y + (cursor_line.saturating_sub(scroll_y)) as u16;
        if cursor_x < editor_inner.x + editor_inner.width && cursor_y < editor_inner.y + editor_inner.height {
            frame.set_cursor_position((cursor_x, cursor_y));
        }
        let help_text = "s: Save | Esc: Back | ↑↓←→: Navigate | Enter: New line";
        let help = Paragraph::new(help_text)
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Center);
        help.render(chunks[1], frame.buffer_mut());
    }
    fn render_footer(&self, frame: &mut Frame, area: Rect) {
        let help_text = match self.current_tab {
            DashboardTab::Repositories => {
                "↑↓: Navigate • a: Add • p: Edit path • SPACE: Select • c: Clone • r: Refresh • Del: Remove • s: Save • q: Quit"
            }
            DashboardTab::Environment => {
                "↑↓: Navigate • e: Edit .env • f: Required files • d: Docker Compose • c: Copy sample • v: Validate • ←→: Tabs"
            }
            DashboardTab::Logs => "←→: Tabs • c: Clear logs • h: Help • s: Save • q: Quit",
            DashboardTab::Workflows => "↑↓: Select • w: Generate • Enter: Edit • Esc: Cancel • ←→: Tabs",
        };
        let footer = Paragraph::new(help_text)
            .style(Style::default().fg(Color::Gray))
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: true });
        footer.render(area, frame.buffer_mut());
    }
    fn render_confirm_dialog(&self, frame: &mut Frame) {
        let title = match self.confirm_action.as_deref() {
            Some("quit") => "Confirm Quit",
            Some("clone_selected") => "Confirm Clone Operation",
            Some(action) if action.starts_with("validate_env_") => "Confirm Environment Validation",
            Some(action) if action.starts_with("validate_") => "Confirm Full Validation",
            _ => "Confirm Action",
        };
        let (summary, details) = match self.confirm_action.as_deref() {
            Some("quit") => (
                "Are you sure you want to quit Expresso-Kit?".to_string(),
                "Any unsaved changes will be lost.".to_string(),
            ),
            Some("clone_selected") => {
                let repos = self.repos.read();
                let selected: Vec<String> = repos.iter().filter(|r| r.selected).map(|r| r.name.clone()).collect();
                let summary = "Clone selected repositories?".to_string();
                let details = if selected.is_empty() {
                    "No repositories selected for cloning.".to_string()
                } else {
                    format!("Repositories to clone:\n{}", selected.join("\n"))
                };
                (summary, details)
            }
            Some(action) if action.starts_with("validate_env_") => {
                let repo_id_str = action.strip_prefix("validate_env_").unwrap_or("0");
                let repo_id: usize = repo_id_str.parse().unwrap_or(0);
                let repos = self.repos.read();
                repos.iter().find(|r| r.id == repo_id).map_or_else(
                    || (format!("Validate environment for repository #{}?", repo_id), "Repository not found.".to_string()),
                    |repo| {
                        let summary = format!("Validate environment for '{}'?", repo.name);
                        let details = format!(
                            "📁 Path: {}\n\n🔍 Environment Validation Checks:\n────────────────────────────────\n• Compare .env.sample ↔ \
                             .env\n• Detect missing variables\n• Detect empty variables\n\nCurrent Status:\n• Sample file: {}\n• Missing \
                             vars: {}\n• Empty vars: {}",
                            repo.path.display(),
                            if repo.env_status.sample_file_exists { "✅ Found" } else { "❌ Not found" },
                            repo.env_status.missing_vars.len(),
                            repo.env_status.empty_vars.len()
                        );
                        (summary, details)
                    },
                )
            }
            Some(action) if action.starts_with("validate_") => {
                let repo_id_str = action.strip_prefix("validate_").unwrap_or("0");
                let repo_id: usize = repo_id_str.parse().unwrap_or(0);
                let repos = self.repos.read();
                repos.iter().find(|r| r.id == repo_id).map_or_else(
                    || (format!("Validate repository #{}?", repo_id), "Repository not found.".to_string()),
                    |repo| {
                        let summary = format!("Run full validation for '{}'?", repo.name);
                        let details = format!(
                            "📁 Path: {}\n\n🔍 Validation Checks to Perform:\n────────────────────────────────\n✓ Environment \
                             validation\n• Compare .env.sample ↔ .env\n• Detect missing/empty variables\n\n✓ Required files check\n• \
                             Verify: {}\n\n✓ Docker Compose validation\n• Parse docker-compose.yml\n• Validate service configurations\n• \
                             Cross-check env var references\n\nCurrent Status:\n• Cloned: {}\n• Errors: {} | Warnings: {}",
                            repo.path.display(),
                            if repo.required_files.is_empty() {
                                ".env".to_string()
                            } else {
                                repo.required_files.join(", ")
                            },
                            if repo.is_cloned() { "✅ Yes" } else { "❌ No" },
                            repo.errors.len(),
                            repo.warnings.len()
                        );
                        (summary, details)
                    },
                )
            }
            _ => (
                "Are you sure you want to proceed?".to_string(),
                "Review the details below before confirming.".to_string(),
            ),
        };
        let actions = &[
            ("ENTER", "Confirm"),
            ("ESC", "Cancel"),
            ("←→", if self.confirm_dialog_expanded { "Collapse" } else { "Expand" }),
        ];
        ConfirmDialog {
            title,
            summary: &summary,
            details: &details,
            expanded: self.confirm_dialog_expanded,
            scroll_offset: self.confirm_dialog_scroll,
            actions,
        }
        .render(frame.area(), frame.buffer_mut());
    }
    fn render_save_dialog(&self, frame: &mut Frame) {
        let repos = self.repos.read();
        let total_repos = repos.len();
        let selected_repos = repos.iter().filter(|r| r.selected).count();
        let cloned_repos = repos.iter().filter(|r| r.is_cloned()).count();
        let mut unique_dirs: std::collections::BTreeMap<String, Vec<String>> = std::collections::BTreeMap::new();
        for repo in repos.iter() {
            let dir_path = repo
                .path
                .parent()
                .map_or_else(|| ".".to_string(), |p| p.to_string_lossy().to_string());
            unique_dirs.entry(dir_path).or_default().push(repo.name.clone());
        }
        let selected_details: Vec<String> = repos
            .iter()
            .filter(|r| r.selected)
            .map(|r| format!("  • {} → {}", r.name, r.path.display()))
            .collect();
        drop(repos);
        let paths_summary = unique_dirs
            .iter()
            .map(|(dir, repo_names)| format!("  📁 {} ({} repos)", dir, repo_names.len()))
            .collect::<Vec<_>>()
            .join("\n");
        let title = "💾 Save Configuration";
        let summary = "Save current state to .expresso-kit.toml";
        let selected_section = if selected_details.is_empty() {
            String::new()
        } else {
            format!("\n\n🔹 Selected repositories:\n{}", selected_details.join("\n"))
        };
        let details = format!(
            "📊 Repositories: {} total, {} selected, {} cloned\n\n📍 Tracked directories:\n{}{}\n\nPress ENTER or Y to save, ESC or N to \
             cancel",
            total_repos, selected_repos, cloned_repos, paths_summary, selected_section
        );
        let actions = &[("ENTER/Y", "Save"), ("ESC/N", "Cancel")];
        ConfirmDialog {
            title,
            summary,
            details: &details,
            expanded: true,
            scroll_offset: 0,
            actions,
        }
        .render(frame.area(), frame.buffer_mut());
    }
    pub fn start_background_tasks(&mut self) {}
    pub async fn shutdown(&mut self) {
        self.should_quit.store(true, Ordering::SeqCst);
        self.save_config();
        tokio::time::sleep(Duration::from_millis(100)).await;
        for task in self.bg_tasks.drain(..) {
            task.abort();
        }
    }
    pub async fn run(&mut self) -> Result<()> {
        if let Err(e) = signals::setup_signal_handler(self.should_quit.clone()) {
            self.add_log(LogLevel::Warning, format!("Could not setup signal handler: {}", e), None);
        }
        self.start_background_tasks();
        enable_raw_mode()?;
        let mut stdout = std::io::stdout();
        execute!(stdout, EnterAlternateScreen, TermClear(ClearType::All), MoveTo(0, 0), EnableMouseCapture)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;
        let result = self.main_loop(&mut terminal);
        self.shutdown().await;
        disable_raw_mode()?;
        execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture)?;
        terminal.show_cursor()?;
        result
    }
    fn main_loop<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> Result<()> {
        terminal.draw(|frame| self.render_dashboard(frame))?;
        let mut clone_progress_rx = self.clone_progress_rx.lock().take();
        loop {
            if self.should_quit.load(Ordering::SeqCst) {
                break;
            }
            if let Some(ref mut rx) = clone_progress_rx {
                while let Ok((repo_id, progress)) = rx.try_recv() {
                    self.handle_clone_progress(repo_id, progress);
                }
            }
            self.toasts.retain(|toast| !toast.is_expired());
            if event::poll(Duration::from_millis(100))?
                && let Event::Key(key) = event::read()?
            {
                self.handle_key_event(key)?;
            }
            const STATUS_REFRESH_INTERVAL: Duration = Duration::from_secs(15);
            if self.last_status_check.elapsed() >= STATUS_REFRESH_INTERVAL {
                self.refresh_repository_statuses();
            }
            self.update_stats();
            terminal.draw(|frame| self.render_dashboard(frame))?;
        }
        Ok(())
    }
}
#[tokio::main]
async fn main() -> Result<()> {
    let cli_args = Cli::parse();
    if should_run_tui(&cli_args) {
        let mut app = App::new()?;
        app.add_log(LogLevel::Success, "Dashboard initialized successfully".to_string(), None);
        app.run().await
    } else {
        let exit_code = cli_run(cli_args);
        if exit_code == std::process::ExitCode::SUCCESS {
            Ok(())
        } else {
            std::process::exit(1);
        }
    }
}
