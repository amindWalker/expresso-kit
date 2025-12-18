//! Environment validation module
//!
//! Validates .env files against .env.sample templates.

use std::{collections::HashMap, fs, path::Path};

use crate::platform::file_patterns;

/// Result of environment file validation
#[derive(Debug, Default, Clone)]
pub struct EnvValidationResult {
    /// Whether a sample file exists
    pub sample_exists: bool,
    /// Whether the .env file exists
    pub env_exists: bool,
    /// Variables defined in sample but missing from .env
    pub missing_vars: Vec<String>,
    /// Variables that exist but have empty values
    pub empty_vars: Vec<String>,
    /// All variables from the sample file
    pub sample_vars: Vec<String>,
    /// All key-value pairs from .env
    pub env_vars: HashMap<String, String>,
}

impl EnvValidationResult {
    /// Check if the environment is valid (no missing or empty vars)
    pub const fn is_valid(&self) -> bool {
        self.missing_vars.is_empty() && self.empty_vars.is_empty()
    }

    /// Total number of issues found
    pub const fn issue_count(&self) -> usize {
        self.missing_vars.len() + self.empty_vars.len()
    }
}

/// Check if a line is a valid env variable definition
fn is_valid_env_line(line: &str) -> bool {
    let t = line.trim();
    !t.is_empty() && !t.starts_with('#')
}

/// Parse a single env line into key-value pair
fn parse_env_line(line: &str) -> Option<(String, String)> {
    let t = line.trim();
    t.find('=').and_then(|pos| {
        let key = t[..pos].trim();
        (!key.is_empty()).then(|| {
            (
                key.to_string(),
                t[pos + 1..].trim().trim_matches('"').trim_matches('\'').to_string(),
            )
        })
    })
}

/// Parse an env file into key-value pairs
pub fn parse_env_file(path: &Path) -> HashMap<String, String> {
    fs::read_to_string(path)
        .map(|c| c.lines().filter(|l| is_valid_env_line(l)).filter_map(parse_env_line).collect())
        .unwrap_or_default()
}

/// Parse a sample file to extract required variable names
pub fn parse_sample_file(path: &Path) -> Vec<String> {
    fs::read_to_string(path)
        .map(|c| {
            c.lines()
                .filter(|l| is_valid_env_line(l))
                .filter_map(|l| {
                    let t = l.trim();
                    t.find('=')
                        .map(|p| t[..p].trim())
                        .or_else(|| (!t.contains(' ')).then_some(t))
                        .filter(|k| !k.is_empty())
                        .map(String::from)
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Validate a repository's environment files
pub fn validate_repository_env(repo_path: &Path) -> EnvValidationResult {
    let env_path = repo_path.join(".env");
    let sample_path = file_patterns::find_env_sample_file(repo_path);

    let sample_exists = sample_path.is_some();
    let env_exists = env_path.exists();

    let sample_vars = sample_path.as_ref().map(|p| parse_sample_file(p)).unwrap_or_default();
    let env_vars = if env_exists { parse_env_file(&env_path) } else { HashMap::new() };

    // Partition into missing and empty
    let (missing_vars, empty_vars): (Vec<_>, Vec<_>) = sample_vars
        .iter()
        .filter_map(|key| {
            match env_vars.get(key) {
                None => Some((key.clone(), true)),                     // missing
                Some(v) if v.is_empty() => Some((key.clone(), false)), // empty
                _ => None,
            }
        })
        .partition(|(_, is_missing)| *is_missing);

    EnvValidationResult {
        sample_exists,
        env_exists,
        missing_vars: missing_vars.into_iter().map(|(k, _)| k).collect(),
        empty_vars: empty_vars.into_iter().map(|(k, _)| k).collect(),
        sample_vars,
        env_vars,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_valid_env_line_should_pass() {
        assert!(is_valid_env_line("FOO=bar"));
        assert!(is_valid_env_line("  FOO=bar  "));
        assert!(!is_valid_env_line("# comment"));
        assert!(!is_valid_env_line(""));
        assert!(!is_valid_env_line("   "));
    }

    #[test]
    fn test_parse_env_line_should_pass() {
        assert_eq!(parse_env_line("FOO=bar"), Some(("FOO".to_string(), "bar".to_string())));
        assert_eq!(parse_env_line("FOO=\"bar\""), Some(("FOO".to_string(), "bar".to_string())));
        assert_eq!(parse_env_line("FOO="), Some(("FOO".to_string(), String::new())));
        assert_eq!(parse_env_line("=bar"), None);
    }
}
