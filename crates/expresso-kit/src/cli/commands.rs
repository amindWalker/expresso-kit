//! CLI command definitions
//!
//! Defines the command-line interface using clap

use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};

/// Expresso-Kit: TUI + CLI for validating git repositories with docker-compose
#[derive(Parser, Debug)]
#[command(
    name = "expresso-kit",
    author,
    version,
    about = "Project validator for git repos with docker-compose"
)]
pub struct Cli {
    /// Run in TUI mode (default if no subcommand provided)
    #[arg(short, long)]
    pub tui: bool,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

/// Available CLI commands
#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Validate repository configuration (.env, docker-compose, files, etc.)
    Validate {
        /// Path to the repository to validate
        #[arg(short, long, default_value = ".")]
        path: PathBuf,

        /// Output format (text, json, github)
        #[arg(short, long, value_enum, default_value = "text")]
        format: OutputFormat,

        /// Required files to check (comma-separated)
        #[arg(short, long, default_value = ".env")]
        required_files: String,

        /// Fail on warnings (exit code 1 if any issues found)
        #[arg(long)]
        strict: bool,

        /// Only validate environment files
        #[arg(long)]
        env_only: bool,

        /// Only validate docker-compose files
        #[arg(long)]
        compose_only: bool,

        /// Only validate required files
        #[arg(long)]
        files_only: bool,

        /// Compare sample files with actual files
        #[arg(long)]
        compare: bool,

        /// Cross-validate env vars with docker-compose references
        #[arg(long)]
        cross_validate: bool,
    },

    /// Generate GitHub Actions workflow
    InitWorkflow {
        /// Target project directory to generate workflow in
        #[arg(short = 'P', long, default_value = ".")]
        path: PathBuf,

        /// Output filename (relative to project's .github/workflows/)
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Repository paths to validate in workflow (comma-separated)
        #[arg(long, default_value = ".")]
        paths: String,

        /// Include compose validation
        #[arg(long)]
        compose: bool,

        /// Required files to check
        #[arg(long, default_value = ".env")]
        required_files: String,

        /// Print to stdout instead of writing file
        #[arg(long)]
        dry_run: bool,

        /// Auto-detect project type and add build/test steps
        #[arg(long)]
        auto_detect: bool,
    },

    /// Check system dependencies (git, etc.)
    CheckDeps {
        /// Output format
        #[arg(short, long, value_enum, default_value = "text")]
        format: OutputFormat,

        /// Fail if any dependency is missing
        #[arg(long)]
        strict: bool,
    },

    /// List docker-compose services
    ListServices {
        /// Path to the repository
        #[arg(short, long, default_value = ".")]
        path: PathBuf,

        /// Output format
        #[arg(short, long, value_enum, default_value = "text")]
        format: OutputFormat,

        /// Show only services with environment variables
        #[arg(long)]
        with_env: bool,
    },

    /// Clone a git repository and validate its configuration
    Clone {
        /// Repository URL to clone
        url: String,

        /// Destination path
        #[arg(short, long)]
        dest: Option<PathBuf>,

        /// Skip validation after cloning
        #[arg(long)]
        no_validate: bool,

        /// Output format
        #[arg(short, long, value_enum, default_value = "text")]
        format: OutputFormat,
    },

    /// Discover projects with docker-compose or .env
    Discover {
        /// Root path to search for projects
        #[arg(short, long, default_value = ".")]
        path: PathBuf,

        /// Maximum directory depth to search
        #[arg(long, default_value = "3")]
        max_depth: usize,

        /// Output format (text, json, github)
        #[arg(short, long, value_enum, default_value = "text")]
        format: OutputFormat,

        /// Only show projects with docker-compose files
        #[arg(long)]
        compose_only: bool,

        /// Only show projects with .env files
        #[arg(long)]
        env_only: bool,

        /// Validate all discovered projects
        #[arg(long)]
        validate: bool,

        /// Required files for validation (comma-separated)
        #[arg(short, long, default_value = ".env")]
        required_files: String,

        /// Fail on warnings when validating
        #[arg(long)]
        strict: bool,

        /// Stop at first failure when validating
        #[arg(long)]
        fail_fast: bool,
    },
}

/// Output format for CLI results
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum OutputFormat {
    /// Human-readable text output
    Text,
    /// JSON output for parsing
    Json,
    /// GitHub Actions annotation format
    Github,
}
