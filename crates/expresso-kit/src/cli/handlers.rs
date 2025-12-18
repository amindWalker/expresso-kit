//! CLI command handlers
//!
//! Implementation of each CLI subcommand

use std::{
    fs,
    path::{Path, PathBuf},
    process::ExitCode,
};

use serde::Serialize;

use super::{
    commands::{Cli, Commands, OutputFormat},
    output::{ValidationResult, output_github, output_result, output_text},
};
use crate::{discovery, docker_compose, git_ops, platform, validation, workflow_templates};

// =============================================================================
// Main Entry Point
// =============================================================================

/// Run the CLI with parsed arguments
pub fn run(cli: Cli) -> ExitCode {
    match cli.command {
        None if cli.tui => ExitCode::SUCCESS, // TUI mode handled in main
        Some(Commands::Validate {
            path,
            format,
            required_files,
            strict,
            env_only,
            compose_only,
            files_only,
            compare,
            cross_validate,
        }) => run_validate(
            &path,
            format,
            &required_files,
            strict,
            env_only,
            compose_only,
            files_only,
            compare,
            cross_validate,
        ),
        Some(Commands::InitWorkflow {
            path,
            output,
            paths,
            compose,
            required_files: _,
            dry_run,
            auto_detect,
        }) => run_init_workflow(&path, output.as_deref(), &paths, compose, dry_run, auto_detect),
        Some(Commands::CheckDeps { format, strict }) => run_check_deps(format, strict),
        Some(Commands::ListServices { path, format, with_env }) => run_list_services(&path, format, with_env),
        Some(Commands::Clone { url, dest, no_validate, format }) => run_clone(&url, dest.as_deref(), !no_validate, format),
        Some(Commands::Discover {
            path,
            max_depth,
            format,
            compose_only,
            env_only,
            validate,
            required_files,
            strict,
            fail_fast,
        }) => run_discover(
            &path,
            max_depth,
            format,
            compose_only,
            env_only,
            validate,
            &required_files,
            strict,
            fail_fast,
        ),
        None => ExitCode::SUCCESS, // Will run TUI
    }
}

/// Check if TUI mode should be run
pub fn should_run_tui(cli: &Cli) -> bool {
    cli.command.is_none() || cli.tui
}

// =============================================================================
// Command Handlers
// =============================================================================

#[allow(clippy::fn_params_excessive_bools, clippy::too_many_arguments)]
fn run_validate(
    path: &Path,
    format: OutputFormat,
    required_files: &str,
    strict: bool,
    env_only: bool,
    compose_only: bool,
    files_only: bool,
    compare: bool,
    cross_validate: bool,
) -> ExitCode {
    if !path.exists() {
        let mut result = ValidationResult::new("validate", path);
        result.add_error(format!("Path does not exist: {}", path.display()));
        output_result(&result, format);
        return ExitCode::FAILURE;
    }

    let validate_all = !env_only && !compose_only && !files_only;

    let opts = validation::ValidationOptions {
        env: validate_all || env_only,
        compose: validate_all || compose_only,
        files: validate_all || files_only,
        cross_validate,
        compare_compose: compare,
    };

    let files: Vec<String> = required_files
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    let v = validation::validate_repository_with_options(path, &files, &opts);
    let mut result = ValidationResult::from_validation("validate", path, &v);
    result.apply_strict(strict);

    output_result(&result, format);

    if result.passed { ExitCode::SUCCESS } else { ExitCode::FAILURE }
}

fn run_init_workflow(project_path: &Path, output: Option<&Path>, paths: &str, compose: bool, dry_run: bool, auto_detect: bool) -> ExitCode {
    let project_path = if project_path.is_absolute() {
        project_path.to_path_buf()
    } else {
        std::env::current_dir().unwrap_or_default().join(project_path)
    };

    if !project_path.exists() {
        eprintln!("Error: Project path does not exist: {}", project_path.display());
        return ExitCode::FAILURE;
    }

    let detected = if auto_detect {
        workflow_templates::detect_project_types(&project_path)
    } else {
        workflow_templates::DetectedProjects::default()
    };

    let filename = output.map_or_else(|| detected.workflow_filename(), |p| p.to_string_lossy().to_string());

    let output_path = project_path.join(".github/workflows").join(&filename);
    let workflow = workflow_templates::generate_workflow(paths, compose, &detected);

    if dry_run {
        println!("# Would write to: {}", output_path.display());
        println!("{workflow}");
        return ExitCode::SUCCESS;
    }

    if let Some(parent) = output_path.parent()
        && let Err(e) = fs::create_dir_all(parent)
    {
        eprintln!("Error creating directory {}: {e}", parent.display());
        return ExitCode::FAILURE;
    }

    match fs::write(&output_path, &workflow) {
        Ok(()) => {
            println!("✓ Generated workflow: {}", output_path.display());
            if auto_detect {
                println!("  Detected: {}", detected.summary());
            }
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("Error writing workflow: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run_check_deps(format: OutputFormat, strict: bool) -> ExitCode {
    let mut result = ValidationResult::new("check-deps", Path::new("."));

    let deps = platform::check_dependencies();

    if deps.git_available {
        result.add_info("git: available");
    } else {
        result.add_error("git: not found");
    }

    if deps.in_git_repo {
        result.add_info("In git repository: yes");
    } else {
        result.add_warning("In git repository: no");
    }

    result.apply_strict(strict);
    output_result(&result, format);

    if result.passed { ExitCode::SUCCESS } else { ExitCode::FAILURE }
}

fn run_list_services(path: &Path, format: OutputFormat, with_env: bool) -> ExitCode {
    let validation = docker_compose::validate_docker_compose(path);

    let services: Vec<_> = validation.services.iter().filter(|s| !with_env || s.has_environment).collect();

    match format {
        OutputFormat::Text => {
            println!("Docker-Compose Services ({})", path.display());

            if services.is_empty() {
                println!("No services found{}", if with_env { " with env vars" } else { "" });
            } else {
                for svc in &services {
                    println!("• {} ({} env vars)", svc.name, svc.environment.len());
                }
            }

            println!("\nTotal: {} service(s)", services.len());
        }
        OutputFormat::Json => {
            #[derive(Serialize)]
            struct ServiceInfo {
                name: String,
                env_count: usize,
            }

            let info: Vec<_> = services
                .iter()
                .map(|s| ServiceInfo { name: s.name.clone(), env_count: s.environment.len() })
                .collect();

            if let Ok(json) = serde_json::to_string_pretty(&info) {
                println!("{json}");
            }
        }
        OutputFormat::Github => {
            for svc in &services {
                println!("::notice::Service '{}' has {} env vars", svc.name, svc.environment.len());
            }
        }
    }

    ExitCode::SUCCESS
}

fn run_clone(url: &str, dest: Option<&Path>, validate: bool, format: OutputFormat) -> ExitCode {
    let mut result = ValidationResult::new("clone", Path::new(url));

    if !platform::check_git_available() {
        result.add_error("git is not available");
        output_result(&result, format);
        return ExitCode::FAILURE;
    }

    let dest_path = dest.map_or_else(|| PathBuf::from(git_ops::extract_repo_name(url)), PathBuf::from);

    if dest_path.exists() {
        result.add_error(format!("Destination exists: {}", dest_path.display()));
        output_result(&result, format);
        return ExitCode::FAILURE;
    }

    result.add_info(format!("Cloning {} to {}", url, dest_path.display()));

    let mut cmd = platform::new_git_command();
    let args = platform::git_args(&["clone", "--progress", url, &dest_path.to_string_lossy()]);
    cmd.args(&args);

    match cmd.output() {
        Ok(output) if output.status.success() => {
            result.add_info("Clone successful");

            if validate {
                let env_result = validation::validate_repository_env(&dest_path);
                if env_result.is_valid() {
                    result.add_info("Environment validation: passed");
                } else {
                    for var in &env_result.missing_vars {
                        result.add_warning(format!("Missing: {var}"));
                    }
                }
            }
        }
        Ok(output) => {
            result.add_error(format!("Clone failed: {}", String::from_utf8_lossy(&output.stderr)));
        }
        Err(e) => {
            result.add_error(format!("Failed to execute git: {e}"));
        }
    }

    output_result(&result, format);

    if result.passed { ExitCode::SUCCESS } else { ExitCode::FAILURE }
}

#[allow(clippy::fn_params_excessive_bools, clippy::too_many_arguments)]
fn run_discover(
    path: &Path,
    max_depth: usize,
    format: OutputFormat,
    compose_only: bool,
    env_only: bool,
    validate: bool,
    required_files: &str,
    strict: bool,
    fail_fast: bool,
) -> ExitCode {
    if !path.exists() {
        eprintln!("Path does not exist: {}", path.display());
        return ExitCode::FAILURE;
    }

    let mut options = discovery::DiscoveryOptions::new().with_max_depth(max_depth);

    if compose_only {
        options = options.compose_only();
    }

    if env_only {
        options = options.env_only();
    }

    let result = discovery::discover_projects(path, &options);

    if !validate {
        match format {
            OutputFormat::Json => {
                if let Ok(json) = serde_json::to_string_pretty(&result) {
                    println!("{json}");
                }
            }
            OutputFormat::Text => {
                println!("\n=== Project Discovery ===\n");
                println!("Root: {}", result.root.display());
                println!("Found: {} projects\n", result.projects.len());

                for p in &result.projects {
                    let status = match (p.has_compose, p.has_env) {
                        (true, true) => "✓ compose + .env",
                        (true, false) => "✓ compose",
                        (false, true) => "✓ .env",
                        (false, false) => "○ samples only",
                    };
                    println!("  📁 {} - {}", p.name, status);
                }
            }
            OutputFormat::Github => {
                for p in &result.projects {
                    println!("::notice::Found project '{}' at {}", p.name, p.path.display());
                }
            }
        }
        return ExitCode::SUCCESS;
    }

    // Validate discovered projects
    let files: Vec<String> = required_files.split(',').map(|s| s.trim().to_string()).collect();
    let mut all_passed = true;
    let mut results: Vec<ValidationResult> = Vec::new();

    for project in &result.projects {
        let mut vr = ValidationResult::new("validate", &project.path);

        let env_result = validation::validate_repository_env(&project.path);
        if env_result.sample_exists && !env_result.env_exists {
            vr.add_warning(format!("{}: has .env.sample but no .env", project.name));
        }

        for var in &env_result.missing_vars {
            vr.add_error(format!("{}: missing env var '{}'", project.name, var));
        }

        if project.has_compose || project.has_compose_sample {
            let compose = docker_compose::validate_docker_compose(&project.path);
            if compose.sample_exists && !compose.compose_exists {
                vr.add_warning(format!("{}: has compose.sample but no compose", project.name));
            }
        }

        let files_result = validation::validate_required_files(&project.path, &files);
        for missing in &files_result.missing {
            vr.add_warning(format!("{}: missing '{}'", project.name, missing));
        }

        vr.apply_strict(strict);

        if !vr.passed {
            all_passed = false;
        }

        results.push(vr);

        if fail_fast && !all_passed {
            break;
        }
    }

    match format {
        OutputFormat::Text => {
            println!("\n=== Validation Results ===\n");
            for r in &results {
                output_text(r);
                println!();
            }
            let passed = results.iter().filter(|r| r.passed).count();
            println!("Summary: {}/{} passed", passed, results.len());
        }
        OutputFormat::Json => {
            if let Ok(json) = serde_json::to_string_pretty(&results) {
                println!("{json}");
            }
        }
        OutputFormat::Github => {
            for r in &results {
                output_github(r);
            }
        }
    }

    if all_passed { ExitCode::SUCCESS } else { ExitCode::FAILURE }
}
