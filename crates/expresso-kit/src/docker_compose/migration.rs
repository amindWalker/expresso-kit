//! Docker Compose migration utilities
//!
//! Migrate environment variables between inline definitions and external files

use std::{collections::HashMap, fs, path::Path};

use super::compose::parse_env_file_value;

// =============================================================================
// Migration Utilities
// =============================================================================

/// Parse environment with support for default values
pub fn parse_environment_with_defaults(env_value: &serde_yml::Value) -> Vec<(String, String)> {
    let mut result = Vec::new();

    match env_value {
        serde_yml::Value::Mapping(map) => {
            for (key, value) in map {
                if let Some(key_str) = key.as_str() {
                    let val_str = match value {
                        serde_yml::Value::String(s) => s.clone(),
                        serde_yml::Value::Number(n) => n.to_string(),
                        serde_yml::Value::Bool(b) => b.to_string(),
                        serde_yml::Value::Null => String::new(),
                        _ => value.as_str().unwrap_or("").to_string(),
                    };
                    result.push((key_str.to_string(), val_str));
                }
            }
        }
        serde_yml::Value::Sequence(seq) => {
            for item in seq {
                if let Some(eq_pos) = item.as_str().and_then(|s| s.find('=').map(|pos| (s, pos))) {
                    let (s, pos) = eq_pos;
                    let key = s[..pos].to_string();
                    let value = s[pos + 1..].to_string();
                    result.push((key, value));
                }
            }
        }
        _ => {}
    }

    result
}

/// Extract the actual value and whether it has a default
fn extract_env_value(value: &str) -> (String, bool) {
    let trimmed = value.trim();

    if trimmed.starts_with("${") && trimmed.ends_with('}') {
        let inner = &trimmed[2..trimmed.len() - 1];
        if let Some(pos) = inner.find(":-") {
            let default_value = &inner[pos + 2..];
            return (default_value.to_string(), true);
        }
        return (String::new(), false);
    }

    (trimmed.to_string(), false)
}

/// Format a value as an environment default
fn format_as_env_default(key: &str, value: &str) -> String {
    if value.starts_with("${") && value.ends_with('}') {
        return value.to_string();
    }
    format!("${{{}:-{}}}", key, value)
}

// =============================================================================
// Migrate to External File
// =============================================================================

/// Migrate environment variables from docker-compose to an external .env file
///
/// Returns the path to the created env file and its relative path
pub fn migrate_env_to_file(
    compose_path: &Path,
    repo_path: &Path,
    service_name: &str,
    env_folder: &str,
) -> Result<(std::path::PathBuf, String), String> {
    let content = fs::read_to_string(compose_path).map_err(|e| format!("Failed to read docker-compose file: {e}"))?;

    let mut yaml: serde_yml::Value = serde_yml::from_str(&content).map_err(|e| format!("Failed to parse docker-compose YAML: {e}"))?;

    // Extract environment variables from the service
    let env_vars: Vec<(String, String)> = if let Some(services) = yaml.get("services") {
        if let Some(service) = services.get(service_name) {
            if let Some(mapping) = service.as_mapping() {
                if let Some(env_value) = mapping.get(serde_yml::Value::String("environment".to_string())) {
                    parse_environment_with_defaults(env_value)
                } else {
                    return Err(format!("Service '{}' has no environment section", service_name));
                }
            } else {
                return Err(format!("Invalid service configuration for '{}'", service_name));
            }
        } else {
            return Err(format!("Service '{}' not found", service_name));
        }
    } else {
        return Err("No services section found".to_string());
    };

    if env_vars.is_empty() {
        return Err("No environment variables to migrate".to_string());
    }

    // Create env directory if needed
    let env_dir = repo_path.join(env_folder);
    if !env_dir.exists() {
        fs::create_dir_all(&env_dir).map_err(|e| format!("Failed to create {}/ directory: {}", env_folder, e))?;
    }

    // Create env file
    let env_filename = format!(".env.{}", service_name);
    let env_file_path = env_dir.join(&env_filename);
    let relative_env_path = format!("{}/{}", env_folder, env_filename);

    let mut env_content = String::new();
    env_content.push_str(&format!("# Environment variables for service: {}\n", service_name));
    env_content.push_str("# Migrated from docker-compose.yml\n");
    env_content.push_str("# Original values with defaults are preserved as comments\n\n");

    for (key, original_value) in &env_vars {
        let (actual_value, has_default) = extract_env_value(original_value);
        if has_default {
            env_content.push_str(&format!("# Original: {}={}\n", key, original_value));
        }
        env_content.push_str(&format!("{}={}\n", key, actual_value));
    }

    fs::write(&env_file_path, &env_content).map_err(|e| format!("Failed to write {}: {}", relative_env_path, e))?;

    // Create backup of original values
    let backup_filename = format!(".env.{}.original", service_name);
    let backup_path = env_dir.join(&backup_filename);
    let original_content: String = env_vars
        .iter()
        .map(|(k, v)| format!("{}={}", k, v))
        .collect::<Vec<_>>()
        .join("\n");

    fs::write(&backup_path, &original_content).map_err(|e| format!("Failed to write backup file: {e}"))?;

    // Update docker-compose.yml
    if let Some(mapping) = yaml
        .get_mut("services")
        .and_then(|s| s.get_mut(service_name))
        .and_then(|s| s.as_mapping_mut())
    {
        mapping.remove(serde_yml::Value::String("environment".to_string()));
        let env_file_key = serde_yml::Value::String("env_file".to_string());
        mapping.insert(env_file_key, serde_yml::Value::String(relative_env_path.clone()));
    }

    let new_yaml = serde_yml::to_string(&yaml).map_err(|e| format!("Failed to serialize YAML: {e}"))?;

    fs::write(compose_path, &new_yaml).map_err(|e| format!("Failed to write docker-compose.yml: {e}"))?;

    Ok((env_file_path, relative_env_path))
}

// =============================================================================
// Restore from External File
// =============================================================================

/// Restore environment variables from external .env file back to docker-compose
///
/// Returns the number of variables restored
pub fn restore_env_from_file(compose_path: &Path, repo_path: &Path, service_name: &str) -> Result<usize, String> {
    let content = fs::read_to_string(compose_path).map_err(|e| format!("Failed to read docker-compose file: {e}"))?;

    let mut yaml: serde_yml::Value = serde_yml::from_str(&content).map_err(|e| format!("Failed to parse docker-compose YAML: {e}"))?;

    // Get env_file reference
    let env_file_ref = yaml
        .get("services")
        .and_then(|services| services.get(service_name))
        .and_then(|service| service.as_mapping())
        .and_then(|mapping| mapping.get(serde_yml::Value::String("env_file".to_string())))
        .and_then(|env_file_value| parse_env_file_value(env_file_value).first().cloned());

    let env_file_ref = env_file_ref.ok_or_else(|| "No env_file reference found".to_string())?;

    let env_folder = std::path::Path::new(&env_file_ref)
        .parent()
        .map_or_else(|| ".".to_string(), |p| p.to_string_lossy().to_string());

    let env_dir = repo_path.join(&env_folder);

    // Try to read from backup first, then from env file
    let backup_filename = format!(".env.{}.original", service_name);
    let backup_path = env_dir.join(&backup_filename);

    let env_vars: HashMap<String, String> = if backup_path.exists() {
        let backup_content = fs::read_to_string(&backup_path).map_err(|e| format!("Failed to read backup file: {e}"))?;

        backup_content
            .lines()
            .filter(|l| !l.trim().is_empty() && !l.trim().starts_with('#'))
            .filter_map(|l| l.find('=').map(|pos| (l[..pos].trim().to_string(), l[pos + 1..].to_string())))
            .collect()
    } else {
        let full_path = repo_path.join(&env_file_ref);
        if full_path.exists() {
            let env_content = fs::read_to_string(&full_path).map_err(|e| format!("Failed to read env file: {e}"))?;

            env_content
                .lines()
                .filter(|l| !l.trim().is_empty() && !l.trim().starts_with('#'))
                .filter_map(|l| {
                    l.find('=')
                        .map(|pos| (l[..pos].trim().to_string(), l[pos + 1..].trim().to_string()))
                })
                .collect()
        } else {
            return Err(format!("Env file not found: {}", env_file_ref));
        }
    };

    if env_vars.is_empty() {
        return Err("No environment variables to restore".to_string());
    }

    let var_count = env_vars.len();

    // Update docker-compose.yml
    if let Some(mapping) = yaml
        .get_mut("services")
        .and_then(|s| s.get_mut(service_name))
        .and_then(|s| s.as_mapping_mut())
    {
        mapping.remove(serde_yml::Value::String("env_file".to_string()));

        let env_key = serde_yml::Value::String("environment".to_string());
        let mut env_mapping = serde_yml::Mapping::new();

        for (key, value) in &env_vars {
            let formatted_value = format_as_env_default(key, value);
            env_mapping.insert(serde_yml::Value::String(key.clone()), serde_yml::Value::String(formatted_value));
        }

        mapping.insert(env_key, serde_yml::Value::Mapping(env_mapping));
    }

    // Clean up files
    let env_filename = format!(".env.{}", service_name);
    let env_file_path = env_dir.join(&env_filename);
    let _ = fs::remove_file(&env_file_path);
    let _ = fs::remove_file(&backup_path);

    let new_yaml = serde_yml::to_string(&yaml).map_err(|e| format!("Failed to serialize YAML: {e}"))?;

    fs::write(compose_path, &new_yaml).map_err(|e| format!("Failed to write docker-compose.yml: {e}"))?;

    Ok(var_count)
}

// =============================================================================
// Service Environment Update
// =============================================================================

/// Update a service's environment variables in docker-compose
pub fn update_service_environment(compose_path: &Path, service_name: &str, env_vars: &[(String, String)]) -> Result<String, String> {
    let content = fs::read_to_string(compose_path).map_err(|e| format!("Failed to read docker-compose file: {e}"))?;

    let mut yaml: serde_yml::Value = serde_yml::from_str(&content).map_err(|e| format!("Failed to parse docker-compose YAML: {e}"))?;

    if let Some(services) = yaml.get_mut("services") {
        if let Some(service) = services.get_mut(service_name) {
            if let Some(mapping) = service.as_mapping_mut() {
                let env_key = serde_yml::Value::String("environment".to_string());
                let mut env_mapping = serde_yml::Mapping::new();

                for (key, value) in env_vars {
                    env_mapping.insert(serde_yml::Value::String(key.clone()), serde_yml::Value::String(value.clone()));
                }

                mapping.insert(env_key, serde_yml::Value::Mapping(env_mapping));
            }
        } else {
            return Err(format!("Service '{}' not found", service_name));
        }
    } else {
        return Err("No services section found".to_string());
    }

    serde_yml::to_string(&yaml).map_err(|e| format!("Failed to serialize YAML: {e}"))
}
