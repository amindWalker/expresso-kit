//! CLI module
//!
//! Command handlers and output formatting for CLI mode

mod commands;
mod handlers;
mod output;

pub use commands::{Cli, Commands, OutputFormat};
pub use handlers::{run, should_run_tui};
pub use output::{ValidationResult, output_result};
