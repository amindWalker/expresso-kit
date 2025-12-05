//! Validation module
//!
//! Comprehensive validation for git repositories including:
//! - Environment file validation (.env vs .env.sample)
//! - Required files validation
//! - Docker-compose validation
//! - Cross-reference validation

pub mod env;
pub mod files;

use std::{
    collections::{HashMap, HashSet},
    path::Path,
};

pub use env::{EnvValidationResult, parse_env_file, parse_sample_file, validate_repository_env};
pub use files::{DEFAULT_REQUIRED_FILES, RequiredFilesResult, validate_required_files};

use crate::docker_compose;

// =============================================================================
// Repository Validation
// =============================================================================

/// Complete validation result for a repository
#[derive(Debug, Clone, Default)]
pub struct RepositoryValidation {
    /// Environment file validation results
    pub env: EnvValidationResult,
    /// Required files validation results
    pub files: RequiredFilesResult,
    /// Docker-compose validation results
    pub compose: docker_compose::DockerComposeValidation,
    /// Missing env vars per docker-compose service
    pub compose_missing_env: HashMap<String, Vec<String>>,
    /// Cross-referenced vars missing from .env
    pub cross_ref_missing: Vec<String>,
    /// Error messages
    pub errors: Vec<String>,
    /// Warning messages
    pub warnings: Vec<String>,
    /// Informational messages
    pub info: Vec<String>,
}

impl RepositoryValidation {
    /// Check if there are any errors
    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }

    /// Check if there are any warnings
    pub fn has_warnings(&self) -> bool {
        !self.warnings.is_empty()
    }
}

/// Options for validation
#[derive(Debug, Clone, Default)]
pub struct ValidationOptions {
    /// Validate environment files
    pub env: bool,
    /// Validate docker-compose files
    pub compose: bool,
    /// Validate required files
    pub files: bool,
    /// Cross-validate env vars with compose references
    pub cross_validate: bool,
    /// Compare sample files with actual files
    pub compare_compose: bool,
}

impl ValidationOptions {
    /// Enable all validation options
    pub fn all() -> Self {
        Self {
            env: true,
            compose: true,
            files: true,
            cross_validate: false,
            compare_compose: false,
        }
    }

    /// Enable cross-validation
    pub fn with_cross_validate(mut self) -> Self {
        self.cross_validate = true;
        self
    }

    /// Enable compose comparison
    pub fn with_compare(mut self) -> Self {
        self.compare_compose = true;
        self
    }
}

/// Validate a repository with specific options
pub fn validate_repository_with_options(path: &Path, required_files_list: &[String], opts: &ValidationOptions) -> RepositoryValidation {
    let mut result = RepositoryValidation::default();

    // Environment validation
    if opts.env {
        result.env = validate_repository_env(path);

        if !result.env.sample_exists {
            result.warnings.push("No .env.sample or .env.example found".to_string());
        } else {
            result.info.push(format!("Sample: {} vars", result.env.sample_vars.len()));
        }

        if !result.env.env_exists {
            result.errors.push(".env file missing".to_string());
        } else {
            result.info.push(format!(".env: {} vars", result.env.env_vars.len()));
        }

        for var in &result.env.missing_vars {
            result.errors.push(format!("Missing: {}", var));
        }
        for var in &result.env.empty_vars {
            result.warnings.push(format!("Empty: {}", var));
        }
    }

    // Required files validation
    if opts.files {
        result.files = validate_required_files(path, required_files_list);

        for f in &result.files.present {
            result.info.push(format!("✓ {}", f));
        }
        for f in &result.files.missing {
            result.errors.push(format!("✗ Missing: {}", f));
        }
    }

    // Docker-compose validation
    if opts.compose {
        result.compose = docker_compose::validate_docker_compose(path);

        if result.compose.sample_exists {
            result
                .info
                .push(format!("Compose sample: {}", result.compose.sample_path.as_deref().unwrap_or("unknown")));
        }

        if result.compose.compose_exists {
            result.info.push("Found docker-compose.yml".to_string());
        }

        if result.compose.sample_exists && !result.compose.compose_exists {
            result
                .errors
                .push("docker-compose.sample exists but no docker-compose.yml".to_string());
        }

        for f in &result.compose.missing_env_files {
            result.warnings.push(format!("env_file missing: {}", f));
        }

        result.info.push(format!("{} services", result.compose.services.len()));

        // Compare compose files if requested
        if opts.compare_compose
            && result.compose.sample_exists
            && result.compose.compose_exists
            && let (Some(sample), Some(actual)) = (docker_compose::find_compose_sample_file(path), docker_compose::find_compose_file(path))
            && let Ok(comparisons) = docker_compose::compare_compose_services(&sample, &actual)
        {
            for comp in comparisons {
                for var in &comp.missing_vars {
                    result.errors.push(format!("'{}': missing '{}'", comp.service_name, var));
                }
                for var in &comp.extra_vars {
                    result.warnings.push(format!("'{}': extra '{}'", comp.service_name, var));
                }
            }
        }

        // Check for missing env vars in services
        for service in &result.compose.services {
            let missing: Vec<String> = service
                .environment
                .iter()
                .map(|(k, _)| k)
                .filter(|var| result.env.missing_vars.contains(*var) || result.env.empty_vars.contains(*var))
                .cloned()
                .collect();

            if !missing.is_empty() {
                result.compose_missing_env.insert(service.name.clone(), missing);
            }
        }
    }

    // Cross-validation
    if opts.cross_validate
        && let Some(compose_file) = docker_compose::find_compose_file(path)
        && let Ok(services) = docker_compose::parse_compose_file(&compose_file)
    {
        let env_vars: HashSet<String> = result.env.env_vars.keys().cloned().collect();
        let referenced: HashSet<String> = services
            .iter()
            .flat_map(|s| {
                s.environment
                    .iter()
                    .flat_map(|(_, v)| docker_compose::extract_env_references(v))
            })
            .collect();

        result.cross_ref_missing = referenced.iter().filter(|v| !env_vars.contains(v.as_str())).cloned().collect();

        if result.cross_ref_missing.is_empty() {
            result.info.push(format!("All {} referenced vars found", referenced.len()));
        } else {
            for var in &result.cross_ref_missing {
                result.warnings.push(format!("${{{}}} not in .env", var));
            }
        }
    }

    result
}

/// Validate a repository with default options
pub fn validate_repository(path: &Path, required_files_list: &[String]) -> RepositoryValidation {
    validate_repository_with_options(path, required_files_list, &ValidationOptions::all())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validation_options_should_pass() {
        let opts = ValidationOptions::all().with_cross_validate().with_compare();
        assert!(opts.env);
        assert!(opts.compose);
        assert!(opts.files);
        assert!(opts.cross_validate);
        assert!(opts.compare_compose);
    }
}
