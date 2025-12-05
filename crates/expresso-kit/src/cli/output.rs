//! CLI output formatting
//!
//! Text, JSON, and GitHub Actions annotation output

use std::path::Path;

use serde::Serialize;

use super::commands::OutputFormat;
use crate::validation;

// =============================================================================
// Validation Result
// =============================================================================

/// Result from a validation check
#[derive(Debug, Serialize)]
pub struct ValidationResult {
    pub passed: bool,
    pub check_name: String,
    pub path: String,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
    pub info: Vec<String>,
}

impl ValidationResult {
    /// Create a new validation result
    pub fn new(check_name: &str, path: &Path) -> Self {
        Self {
            passed: true,
            check_name: check_name.to_string(),
            path: path.display().to_string(),
            errors: Vec::new(),
            warnings: Vec::new(),
            info: Vec::new(),
        }
    }

    /// Create from a repository validation
    pub fn from_validation(check_name: &str, path: &Path, v: &validation::RepositoryValidation) -> Self {
        Self {
            passed: !v.has_errors(),
            check_name: check_name.to_string(),
            path: path.display().to_string(),
            errors: v.errors.clone(),
            warnings: v.warnings.clone(),
            info: v.info.clone(),
        }
    }

    /// Add an error
    pub fn add_error(&mut self, msg: impl Into<String>) {
        self.passed = false;
        self.errors.push(msg.into());
    }

    /// Add a warning
    pub fn add_warning(&mut self, msg: impl Into<String>) {
        self.warnings.push(msg.into());
    }

    /// Add an info message
    pub fn add_info(&mut self, msg: impl Into<String>) {
        self.info.push(msg.into());
    }

    /// Apply strict mode (warnings become failures)
    pub fn apply_strict(&mut self, strict: bool) {
        if strict && !self.warnings.is_empty() {
            self.passed = false;
        }
    }
}

// =============================================================================
// Output Functions
// =============================================================================

/// Output a validation result in the specified format
pub fn output_result(result: &ValidationResult, format: OutputFormat) {
    match format {
        OutputFormat::Text => output_text(result),
        OutputFormat::Json => {
            if let Ok(json) = serde_json::to_string_pretty(result) {
                println!("{json}");
            }
        }
        OutputFormat::Github => output_github(result),
    }
}

/// Output in human-readable text format
pub fn output_text(result: &ValidationResult) {
    let status = if result.passed { "PASSED ✓" } else { "FAILED ✗" };

    println!("{}: {} ({})", result.check_name, status, result.path);

    for err in &result.errors {
        println!("  ✗ {err}");
    }

    for warn in &result.warnings {
        println!("  ⚠ {warn}");
    }

    for info in &result.info {
        println!("  ℹ {info}");
    }
}

/// Output in GitHub Actions annotation format
pub fn output_github(result: &ValidationResult) {
    for err in &result.errors {
        println!("::error title={}::{err}", result.check_name);
    }

    for warn in &result.warnings {
        println!("::warning title={}::{warn}", result.check_name);
    }

    for info in &result.info {
        println!("::notice title={}::{info}", result.check_name);
    }
}
