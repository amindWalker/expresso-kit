//! Required files validation
//!
//! Checks for the presence of required files in a repository.

use std::path::Path;

/// Default files that should be present in a repo
pub const DEFAULT_REQUIRED_FILES: &[&str] = &[".env"];

/// Result of required files validation
#[derive(Debug, Default, Clone)]
pub struct RequiredFilesResult {
    /// List of files that were required
    pub required: Vec<String>,
    /// Files that are missing
    pub missing: Vec<String>,
    /// Files that are present
    pub present: Vec<String>,
}

impl RequiredFilesResult {
    /// Check if all required files are present
    pub const fn is_valid(&self) -> bool {
        self.missing.is_empty()
    }
}

/// Validate that required files exist in a repository
pub fn validate_required_files(repo_path: &Path, required_files: &[String]) -> RequiredFilesResult {
    let (present, missing): (Vec<_>, Vec<_>) = required_files.iter().cloned().partition(|f| repo_path.join(f).exists());

    RequiredFilesResult { required: required_files.to_vec(), missing, present }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    #[test]
    fn test_validate_nonexistent_path_should_fail() {
        let result = validate_required_files(&PathBuf::from("/nonexistent/path"), &[".env".to_string()]);
        assert!(!result.is_valid());
        assert_eq!(result.missing, vec![".env".to_string()]);
    }
}
