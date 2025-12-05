//! Docker Compose parsing and validation

use std::{collections::HashMap, fs, path::Path};

use crate::file_patterns::{find_compose_file, find_compose_sample_file};

// =============================================================================
// Service Configuration Types
// =============================================================================

/// Environment configuration for a docker-compose service
#[derive(Debug, Clone, Default)]
pub struct ServiceEnvConfig {
    pub name: String,
    pub environment: Vec<(String, String)>,
    pub env_file: Vec<String>,
    pub has_environment: bool,
    pub has_env_file: bool,
}

/// Validation results for docker-compose files
#[derive(Debug, Clone, Default)]
pub struct DockerComposeValidation {
    pub sample_exists: bool,
    pub compose_exists: bool,
    pub sample_path: Option<String>,
    pub services: Vec<ServiceEnvConfig>,
    pub all_env_vars: HashMap<String, Vec<String>>,
    pub referenced_env_vars: Vec<String>,
    pub missing_env_files: Vec<String>,
    pub valid_env_files: Vec<String>,
}

/// Comparison between sample and actual docker-compose services
#[derive(Debug, Clone, Default)]
pub struct ServiceComparison {
    pub service_name: String,
    pub sample_vars: Vec<(String, String)>,
    pub compose_vars: Vec<(String, String)>,
    pub missing_vars: Vec<String>,                     // In sample but not in compose
    pub extra_vars: Vec<String>,                       // In compose but not in sample
    pub different_vars: Vec<(String, String, String)>, // (var_name, sample_value, compose_value)
    pub sample_has_env_file: bool,
    pub compose_has_env_file: bool,
    pub compose_env_files: Vec<String>,
}

impl ServiceComparison {
    pub fn is_valid(&self) -> bool {
        self.missing_vars.is_empty() && self.different_vars.is_empty()
    }

    pub fn has_issues(&self) -> bool {
        !self.missing_vars.is_empty() || !self.different_vars.is_empty()
    }
}

// =============================================================================
// YAML Parsing Utilities
// =============================================================================

fn yaml_value_to_string(value: &serde_yml::Value) -> String {
    match value {
        serde_yml::Value::String(s) => s.clone(),
        serde_yml::Value::Number(n) => n.to_string(),
        serde_yml::Value::Bool(b) => b.to_string(),
        serde_yml::Value::Null => String::new(),
        _ => value.as_str().unwrap_or("").to_string(),
    }
}

pub(crate) fn parse_environment(env_value: &serde_yml::Value) -> Vec<(String, String)> {
    match env_value {
        serde_yml::Value::Mapping(map) => map
            .iter()
            .filter_map(|(key, value)| key.as_str().map(|k| (k.to_string(), yaml_value_to_string(value))))
            .collect(),
        serde_yml::Value::Sequence(seq) => seq
            .iter()
            .filter_map(|item| item.as_str())
            .map(|s| {
                s.find('=').map_or_else(
                    || (s.to_string(), String::new()),
                    |pos| (s[..pos].trim().to_string(), s[pos + 1..].trim().to_string()),
                )
            })
            .collect(),
        _ => Vec::new(),
    }
}

pub(crate) fn parse_env_file_value(env_file_value: &serde_yml::Value) -> Vec<String> {
    match env_file_value {
        serde_yml::Value::String(s) => vec![s.clone()],
        serde_yml::Value::Sequence(seq) => seq.iter().filter_map(|item| item.as_str().map(String::from)).collect(),
        _ => Vec::new(),
    }
}

// =============================================================================
// Compose File Parsing
// =============================================================================

/// Parse a docker-compose file and extract service configurations
pub fn parse_compose_file(path: &Path) -> Result<Vec<ServiceEnvConfig>, String> {
    let content = fs::read_to_string(path).map_err(|e| format!("Failed to read docker-compose file: {e}"))?;

    let yaml: serde_yml::Value = serde_yml::from_str(&content).map_err(|e| format!("Failed to parse docker-compose YAML: {e}"))?;

    let mut services = Vec::new();

    if let Some(services_map) = yaml.get("services").and_then(|s| s.as_mapping()) {
        for (service_name, service_config) in services_map {
            let name = service_name.as_str().unwrap_or("unknown").to_string();
            let mut config = ServiceEnvConfig { name, ..Default::default() };

            if let Some(mapping) = service_config.as_mapping() {
                if let Some(env_value) = mapping.get(serde_yml::Value::String("environment".to_string())) {
                    config.environment = parse_environment(env_value);
                    config.has_environment = true;
                }

                if let Some(env_file_value) = mapping.get(serde_yml::Value::String("env_file".to_string())) {
                    config.env_file = parse_env_file_value(env_file_value);
                    config.has_env_file = true;
                }
            }

            services.push(config);
        }
    }

    Ok(services)
}

// =============================================================================
// Validation
// =============================================================================

/// Validate docker-compose files in a repository
pub fn validate_docker_compose(repo_path: &Path) -> DockerComposeValidation {
    let sample_path = find_compose_sample_file(repo_path);
    let compose_path = find_compose_file(repo_path);

    let sample_exists = sample_path.is_some();
    let compose_exists = compose_path.is_some();

    let services = sample_path
        .as_ref()
        .or(compose_path.as_ref())
        .map(|path| parse_compose_file(path).unwrap_or_default())
        .unwrap_or_default();

    let mut all_env_vars: HashMap<String, Vec<String>> = HashMap::new();
    let mut referenced_env_vars: Vec<String> = Vec::new();
    let mut missing_env_files: Vec<String> = Vec::new();
    let mut valid_env_files: Vec<String> = Vec::new();

    for service in &services {
        for (var_name, var_value) in &service.environment {
            all_env_vars.entry(var_name.clone()).or_default().push(service.name.clone());

            for referenced in extract_env_references(var_value) {
                if !referenced_env_vars.contains(&referenced) {
                    referenced_env_vars.push(referenced);
                }
            }
        }

        for env_file_path in &service.env_file {
            let full_path = repo_path.join(env_file_path);
            if full_path.exists() {
                if !valid_env_files.contains(env_file_path) {
                    valid_env_files.push(env_file_path.clone());
                }
            } else if !missing_env_files.contains(env_file_path) {
                missing_env_files.push(env_file_path.clone());
            }
        }
    }

    DockerComposeValidation {
        sample_exists,
        compose_exists,
        sample_path: sample_path.map(|p| p.to_string_lossy().to_string()),
        services,
        all_env_vars,
        referenced_env_vars,
        missing_env_files,
        valid_env_files,
    }
}

// =============================================================================
// Environment Variable Reference Extraction
// =============================================================================

/// Extract environment variable references from a value string
/// Handles both ${VAR} and $VAR syntax
pub fn extract_env_references(value: &str) -> Vec<String> {
    let mut refs = Vec::new();
    let mut chars = value.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '$' {
            if chars.peek() == Some(&'{') {
                chars.next(); // consume '{'
                let mut var_name = String::new();
                let mut found_closing_brace = false;

                for ch in chars.by_ref() {
                    if ch == '}' {
                        found_closing_brace = true;
                        break;
                    }
                    if ch == ':' || ch == '-' {
                        break;
                    }
                    var_name.push(ch);
                }

                // Only consume remaining chars if we broke on ':' or '-' (not '}')
                if !found_closing_brace {
                    for ch in chars.by_ref() {
                        if ch == '}' {
                            break;
                        }
                    }
                }

                if !var_name.is_empty() {
                    refs.push(var_name);
                }
            } else {
                let mut var_name = String::new();
                while let Some(&ch) = chars.peek() {
                    if ch.is_alphanumeric() || ch == '_' {
                        var_name.push(ch);
                        chars.next();
                    } else {
                        break;
                    }
                }

                if !var_name.is_empty() {
                    refs.push(var_name);
                }
            }
        }
    }

    refs
}

// =============================================================================
// Service Comparison
// =============================================================================

fn vec_contains_key(vec: &[(String, String)], key: &str) -> bool {
    vec.iter().any(|(k, _)| k == key)
}

fn vec_get_value<'a>(vec: &'a [(String, String)], key: &str) -> Option<&'a str> {
    vec.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
}

fn normalize_env_value(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.starts_with("${") && trimmed.ends_with('}') {
        let inner = &trimmed[2..trimmed.len() - 1];
        if let Some(pos) = inner.find(":-") {
            return inner[..pos].to_string();
        }
        return inner.to_string();
    }
    trimmed.to_string()
}

/// Compare services between sample and actual docker-compose files
pub fn compare_compose_services(sample_path: &Path, compose_path: &Path) -> Result<Vec<ServiceComparison>, String> {
    let sample_services = parse_compose_file(sample_path)?;
    let compose_services = parse_compose_file(compose_path)?;

    let mut comparisons = Vec::new();

    let compose_map: HashMap<String, &ServiceEnvConfig> = compose_services.iter().map(|s| (s.name.clone(), s)).collect();

    for sample_service in &sample_services {
        let mut comparison = ServiceComparison {
            service_name: sample_service.name.clone(),
            sample_vars: sample_service.environment.clone(),
            compose_vars: Vec::new(),
            missing_vars: Vec::new(),
            extra_vars: Vec::new(),
            different_vars: Vec::new(),
            sample_has_env_file: sample_service.has_env_file,
            compose_has_env_file: false,
            compose_env_files: Vec::new(),
        };

        if let Some(compose_service) = compose_map.get(&sample_service.name) {
            comparison.compose_vars.clone_from(&compose_service.environment);
            comparison.compose_has_env_file = compose_service.has_env_file;
            comparison.compose_env_files.clone_from(&compose_service.env_file);

            // Find missing vars (in sample but not in compose)
            for (var_name, _) in &sample_service.environment {
                if !vec_contains_key(&compose_service.environment, var_name) {
                    comparison.missing_vars.push(var_name.clone());
                }
            }

            // Find extra vars (in compose but not in sample)
            for (var_name, _) in &compose_service.environment {
                if !vec_contains_key(&sample_service.environment, var_name) {
                    comparison.extra_vars.push(var_name.clone());
                }
            }

            // Find different values
            for (var_name, sample_value) in &sample_service.environment {
                if let Some(compose_value) = vec_get_value(&compose_service.environment, var_name) {
                    let normalized_sample = normalize_env_value(sample_value);
                    let normalized_compose = normalize_env_value(compose_value);

                    if normalized_sample != normalized_compose {
                        comparison
                            .different_vars
                            .push((var_name.clone(), sample_value.clone(), compose_value.to_string()));
                    }
                }
            }
        } else {
            // Service doesn't exist in compose - all vars are missing
            comparison.missing_vars = sample_service.environment.iter().map(|(k, _)| k.clone()).collect();
        }

        comparisons.push(comparison);
    }

    // Find extra services in compose that aren't in sample
    for compose_service in &compose_services {
        if !sample_services.iter().any(|s| s.name == compose_service.name) {
            comparisons.push(ServiceComparison {
                service_name: compose_service.name.clone(),
                sample_vars: Vec::new(),
                compose_vars: compose_service.environment.clone(),
                missing_vars: Vec::new(),
                extra_vars: compose_service.environment.iter().map(|(k, _)| k.clone()).collect(),
                different_vars: Vec::new(),
                sample_has_env_file: false,
                compose_has_env_file: compose_service.has_env_file,
                compose_env_files: compose_service.env_file.clone(),
            });
        }
    }

    Ok(comparisons)
}

/// Copy docker-compose sample to docker-compose.yml
pub fn copy_compose_sample(repo_path: &Path) -> Result<std::path::PathBuf, String> {
    let sample_path = find_compose_sample_file(repo_path).ok_or_else(|| "No docker-compose sample file found".to_string())?;

    let target_path = repo_path.join("docker-compose.yml");

    crate::file_patterns::copy_sample_to_target(&sample_path, &target_path, "docker-compose")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_env_references_should_pass() {
        assert_eq!(extract_env_references("${DB_HOST:-localhost}"), vec!["DB_HOST"]);
        assert_eq!(extract_env_references("$HOME/data"), vec!["HOME"]);
        assert_eq!(extract_env_references("${A}:${B}"), vec!["A", "B"]);
        assert!(extract_env_references("no variables").is_empty());
    }

    #[test]
    fn test_normalize_env_value_should_pass() {
        assert_eq!(normalize_env_value("${VAR:-default}"), "VAR");
        assert_eq!(normalize_env_value("${VAR}"), "VAR");
        assert_eq!(normalize_env_value("plain"), "plain");
    }
}
