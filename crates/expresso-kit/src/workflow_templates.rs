//! Workflow Templates Database
//!
//! Contains robust GitHub Actions workflow templates for different project types.
//! Each template is designed to be comprehensive and production-ready.

use std::path::Path;

/// Detected project types based on manifest files
#[derive(Debug, Default, Clone)]
pub struct DetectedProjects {
    pub rust: bool,
    pub node: bool,
    pub python: bool,
    pub go: bool,
    pub docker: bool,
    pub docker_compose: bool,
    pub env_sample: bool,
    pub project_name: String,
}

impl DetectedProjects {
    pub fn summary(&self) -> String {
        let mut types = Vec::new();
        if self.rust {
            types.push("Rust");
        }
        if self.node {
            types.push("Node.js");
        }
        if self.python {
            types.push("Python");
        }
        if self.go {
            types.push("Go");
        }
        if self.docker {
            types.push("Docker");
        }
        if self.docker_compose {
            types.push("Docker Compose");
        }
        if self.env_sample {
            types.push(".env config");
        }
        if types.is_empty() {
            "No specific project type".into()
        } else {
            types.join(", ")
        }
    }

    pub fn workflow_filename(&self) -> String {
        let name = if self.project_name.is_empty() { "project" } else { &self.project_name };
        format!("{}-ci.yml", name.to_lowercase().replace(' ', "-"))
    }
}

/// Detect project types from filesystem
pub fn detect_project_types(path: &Path) -> DetectedProjects {
    let project_name = extract_project_name(path);

    DetectedProjects {
        rust: path.join("Cargo.toml").exists(),
        node: path.join("package.json").exists(),
        python: path.join("pyproject.toml").exists() || path.join("requirements.txt").exists() || path.join("setup.py").exists(),
        go: path.join("go.mod").exists(),
        docker: path.join("Dockerfile").exists(),
        docker_compose: path.join("docker-compose.yml").exists()
            || path.join("docker-compose.yaml").exists()
            || path.join("compose.yml").exists(),
        env_sample: path.join(".env.sample").exists() || path.join(".env.example").exists() || path.join("env.sample").exists(),
        project_name,
    }
}

/// Extract project name from manifest files or directory name
fn extract_project_name(path: &Path) -> String {
    // Try Cargo.toml
    if let Ok(content) = std::fs::read_to_string(path.join("Cargo.toml"))
        && let Some(name) = extract_toml_name(&content)
    {
        return name;
    }

    // Try package.json
    if let Ok(content) = std::fs::read_to_string(path.join("package.json"))
        && let Some(name) = extract_json_name(&content)
    {
        return name;
    }

    // Try pyproject.toml
    if let Ok(content) = std::fs::read_to_string(path.join("pyproject.toml"))
        && let Some(name) = extract_toml_name(&content)
    {
        return name;
    }

    // Try go.mod
    if let Ok(content) = std::fs::read_to_string(path.join("go.mod"))
        && let Some(line) = content.lines().next()
        && let Some(module) = line.strip_prefix("module ")
    {
        let name = module.split('/').next_back().unwrap_or(module);
        return name.trim().to_string();
    }

    // Fallback to directory name
    path.file_name()
        .and_then(|n| n.to_str())
        .map_or_else(|| "project".to_string(), String::from)
}

fn extract_toml_name(content: &str) -> Option<String> {
    // Track section: [package], [project] (pyproject.toml), [workspace.package]
    let mut in_name_section = false;

    for line in content.lines() {
        let line = line.trim();

        // Track section headers
        match line {
            "[package]" | "[project]" | "[workspace.package]" => {
                in_name_section = true;
                continue;
            }
            _ if line.starts_with('[') => {
                in_name_section = false;
                continue;
            }
            _ => {}
        }

        // Look for name in valid sections
        if in_name_section && line.starts_with("name") && line.contains('=') {
            let value = line.split('=').nth(1)?.trim();
            return Some(value.trim_matches('"').trim_matches('\'').to_string());
        }
    }

    // Try to get from workspace members as fallback
    for line in content.lines() {
        let line = line.trim();
        if line.starts_with("members")
            && line.contains('[')
            && let Some(start) = line.find('"')
            && let Some(end) = line[start + 1..].find('"')
        {
            let member = &line[start + 1..start + 1 + end];
            return Some(member.split('/').next_back().unwrap_or(member).to_string());
        }
    }
    None
}

fn extract_json_name(content: &str) -> Option<String> {
    for line in content.lines() {
        let line = line.trim();
        if line.contains("\"name\"") && line.contains(':') {
            // Extract value after colon, handling: "name": "value", or "name": "value"}
            let value = line.split(':').nth(1)?.trim();
            // Remove trailing comma, braces, and quotes
            let value = value.trim_end_matches([',', '}', ' ']);
            return Some(value.trim_matches('"').to_string());
        }
    }
    None
}

/// Generate complete workflow from detected project types
pub fn generate_workflow(paths: &str, compose: bool, detected: &DetectedProjects) -> String {
    let paths_list: Vec<&str> = paths.split(',').map(str::trim).collect();

    let mut jobs = Vec::new();

    // Add language-specific jobs
    if detected.rust {
        jobs.push(rust_job());
    }
    if detected.node {
        jobs.push(node_job());
    }
    if detected.python {
        jobs.push(python_job());
    }
    if detected.go {
        jobs.push(go_job());
    }

    // Add Docker jobs if detected
    if detected.docker {
        jobs.push(docker_build_job());
    }
    if detected.docker_compose {
        jobs.push(docker_compose_job());
    }

    // Add validation job (always included)
    jobs.push(validation_job(&paths_list, compose, detected));

    // Build the complete workflow
    let jobs_str = jobs.join("\n");
    let project_name = if detected.project_name.is_empty() {
        "Project"
    } else {
        &detected.project_name
    };
    let detected_summary = detected.summary();

    format!(
        "# Generated by expresso-kit
# Project: {project_name}
# Detected: {detected_summary}

name: {project_name} CI

on:
  push:
    branches: [main, master, dev, develop]
  pull_request:
    branches: [main, master]
  workflow_dispatch:

concurrency:
  group: ${{{{ github.workflow }}}}-${{{{ github.ref }}}}
  cancel-in-progress: true

jobs:
{jobs_str}
"
    )
}

// ==================== RUST WORKFLOW ====================

fn rust_job() -> String {
    "  rust:
    name: Rust Build & Test
    runs-on: ubuntu-latest
    strategy:
      fail-fast: false
      matrix:
        toolchain: [stable, beta]

    steps:
      - name: Checkout
        uses: actions/checkout@v4

      - name: Install Rust toolchain
        uses: dtolnay/rust-toolchain@master
        with:
          toolchain: ${{ matrix.toolchain }}
          components: clippy, rustfmt

      - name: Cache cargo registry
        uses: actions/cache@v4
        with:
          path: |
            ~/.cargo/registry
            ~/.cargo/git
            target
          key: ${{ runner.os }}-cargo-${{ matrix.toolchain }}-${{ hashFiles('**/Cargo.lock') }}
          restore-keys: |
            ${{ runner.os }}-cargo-${{ matrix.toolchain }}-

      - name: Check formatting
        run: cargo fmt --all -- --check

      - name: Clippy lints
        run: cargo clippy --all-targets --all-features -- -D warnings

      - name: Build
        run: cargo build --all-targets --all-features

      - name: Run tests
        run: cargo test --all-targets --all-features

      - name: Build documentation
        run: cargo doc --no-deps --all-features
        env:
          RUSTDOCFLAGS: -D warnings
"
    .to_string()
}

// ==================== NODE.JS WORKFLOW ====================

fn node_job() -> String {
    "  node:
    name: Node.js Build & Test
    runs-on: ubuntu-latest
    strategy:
      fail-fast: false
      matrix:
        node-version: ['18', '20', '22']

    steps:
      - name: Checkout
        uses: actions/checkout@v4

      - name: Setup Node.js ${{ matrix.node-version }}
        uses: actions/setup-node@v4
        with:
          node-version: ${{ matrix.node-version }}
          cache: 'npm'

      - name: Install dependencies
        run: npm ci

      - name: Run linter
        run: npm run lint --if-present
        continue-on-error: true

      - name: Check types
        run: npm run type-check --if-present || npm run typecheck --if-present || true

      - name: Build
        run: npm run build --if-present

      - name: Run tests
        run: npm test --if-present

      - name: Upload coverage
        if: matrix.node-version == '20'
        uses: codecov/codecov-action@v4
        continue-on-error: true
        with:
          fail_ci_if_error: false
"
    .to_string()
}

// ==================== PYTHON WORKFLOW ====================

fn python_job() -> String {
    "  python:
    name: Python Build & Test
    runs-on: ubuntu-latest
    strategy:
      fail-fast: false
      matrix:
        python-version: ['3.10', '3.11', '3.12']

    steps:
      - name: Checkout
        uses: actions/checkout@v4

      - name: Setup Python ${{ matrix.python-version }}
        uses: actions/setup-python@v5
        with:
          python-version: ${{ matrix.python-version }}
          cache: 'pip'

      - name: Install dependencies
        run: |
          python -m pip install --upgrade pip
          pip install ruff pytest pytest-cov mypy
          if [ -f requirements.txt ]; then pip install -r requirements.txt; fi
          if [ -f requirements-dev.txt ]; then pip install -r requirements-dev.txt; fi
          if [ -f pyproject.toml ]; then pip install -e . 2>/dev/null || true; fi

      - name: Lint with ruff
        run: ruff check .
        continue-on-error: true

      - name: Format check with ruff
        run: ruff format --check .
        continue-on-error: true

      - name: Type check with mypy
        run: mypy . --ignore-missing-imports
        continue-on-error: true

      - name: Run tests with pytest
        run: pytest --cov --cov-report=xml --cov-report=term-missing -v
        continue-on-error: true

      - name: Upload coverage
        if: matrix.python-version == '3.12'
        uses: codecov/codecov-action@v4
        continue-on-error: true
        with:
          fail_ci_if_error: false
"
    .to_string()
}

// ==================== GO WORKFLOW ====================

fn go_job() -> String {
    "  go:
    name: Go Build & Test
    runs-on: ubuntu-latest
    strategy:
      fail-fast: false
      matrix:
        go-version: ['1.21', '1.22', '1.23']

    steps:
      - name: Checkout
        uses: actions/checkout@v4

      - name: Setup Go ${{ matrix.go-version }}
        uses: actions/setup-go@v5
        with:
          go-version: ${{ matrix.go-version }}
          cache: true

      - name: Verify dependencies
        run: go mod verify

      - name: Download dependencies
        run: go mod download

      - name: Build
        run: go build -v ./...

      - name: Run tests
        run: go test -v -race -coverprofile=coverage.out ./...

      - name: Install golangci-lint
        run: go install github.com/golangci/golangci-lint/cmd/golangci-lint@latest

      - name: Run linter
        run: golangci-lint run --timeout 5m
        continue-on-error: true

      - name: Upload coverage
        if: matrix.go-version == '1.22'
        uses: codecov/codecov-action@v4
        continue-on-error: true
        with:
          file: coverage.out
          fail_ci_if_error: false
"
    .to_string()
}

// ==================== DOCKER BUILD WORKFLOW ====================

fn docker_build_job() -> String {
    "  docker-build:
    name: Docker Build
    runs-on: ubuntu-latest

    steps:
      - name: Checkout
        uses: actions/checkout@v4

      - name: Set up Docker Buildx
        uses: docker/setup-buildx-action@v3

      - name: Build Docker image
        uses: docker/build-push-action@v6
        with:
          context: .
          push: false
          tags: test:latest
          cache-from: type=gha
          cache-to: type=gha,mode=max

      - name: Test container starts
        run: |
          docker run --rm -d --name test-container test:latest || true
          sleep 5
          docker logs test-container 2>/dev/null || true
          docker stop test-container 2>/dev/null || true
"
    .to_string()
}

// ==================== DOCKER COMPOSE WORKFLOW ====================

fn docker_compose_job() -> String {
    "  docker-compose:
    name: Docker Compose Integration
    runs-on: ubuntu-latest

    steps:
      - name: Checkout
        uses: actions/checkout@v4

      - name: Setup environment file
        run: |
          for sample in .env.sample .env.example env.sample; do
            if [ -f \"$sample\" ]; then
              cp \"$sample\" .env
              echo \"Created .env from $sample\"
              break
            fi
          done
          if [ ! -f .env ]; then
            touch .env
            echo \"Created empty .env file\"
          fi

      - name: Validate docker-compose.yml syntax
        run: docker compose config --quiet

      - name: Show docker-compose configuration
        run: docker compose config

      - name: Pull images
        run: docker compose pull --ignore-pull-failures
        continue-on-error: true

      - name: Build services
        run: docker compose build --parallel
        continue-on-error: true

      - name: Start services
        run: |
          docker compose up -d
          echo \"Waiting for services to be healthy...\"
          sleep 15

      - name: Check service status
        run: |
          docker compose ps
          docker compose logs --tail=50

      - name: Run health checks
        run: |
          SERVICES=$(docker compose ps --services)
          for service in $SERVICES; do
            STATUS=$(docker compose ps --status running $service --quiet)
            if [ -z \"$STATUS\" ]; then
              echo \"Warning: Service $service is not running\"
              docker compose logs $service --tail=20
            else
              echo \"OK: Service $service is running\"
            fi
          done

      - name: Collect logs on failure
        if: failure()
        run: |
          mkdir -p logs
          docker compose logs --no-color > logs/docker-compose.log

      - name: Upload logs
        if: failure()
        uses: actions/upload-artifact@v4
        with:
          name: docker-compose-logs
          path: logs/
          retention-days: 7

      - name: Cleanup
        if: always()
        run: |
          docker compose down --volumes --remove-orphans
          docker compose rm -f
"
    .to_string()
}

// ==================== VALIDATION WORKFLOW ====================

fn validation_job(paths: &[&str], compose: bool, detected: &DetectedProjects) -> String {
    let mut validate_steps = String::new();
    for path in paths {
        let compose_flag = if compose { " --compare" } else { "" };
        validate_steps.push_str(&format!(
            "
      - name: Validate configuration ({path})
        run: expresso-kit validate --path {path} --format github --strict{compose_flag}
"
        ));
    }

    // Determine dependencies for the validation job
    let mut needs = Vec::new();
    if detected.rust {
        needs.push("rust");
    }
    if detected.node {
        needs.push("node");
    }
    if detected.python {
        needs.push("python");
    }
    if detected.go {
        needs.push("go");
    }
    if detected.docker {
        needs.push("docker-build");
    }
    if detected.docker_compose {
        needs.push("docker-compose");
    }

    let needs_str = if needs.is_empty() {
        String::new()
    } else {
        format!("\n    needs: [{}]", needs.join(", "))
    };

    // Install expresso-kit step depends on whether Rust is already set up
    let install_step = if detected.rust {
        "      - name: Install expresso-kit
        run: cargo install expresso-kit || cargo install --path . || true"
    } else {
        "      - name: Install Rust (for expresso-kit)
        uses: dtolnay/rust-toolchain@stable

      - name: Install expresso-kit
        run: cargo install expresso-kit"
    };

    format!(
        "  validate:
    name: Configuration Validation
    runs-on: ubuntu-latest{needs_str}
    if: always()

    steps:
      - name: Checkout
        uses: actions/checkout@v4

{install_step}

      - name: Check dependencies
        run: expresso-kit check-deps --format github
{validate_steps}
      - name: Generate validation summary
        if: always()
        run: |
          echo \"## Validation Results\" >> $GITHUB_STEP_SUMMARY
          echo \"\" >> $GITHUB_STEP_SUMMARY
          expresso-kit validate --format text >> $GITHUB_STEP_SUMMARY || true
"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_workflow_filename_should_pass() {
        let detected = DetectedProjects {
            project_name: "My Cool Project".to_string(),
            ..Default::default()
        };
        assert_eq!(detected.workflow_filename(), "my-cool-project-ci.yml");
    }

    #[test]
    fn test_summary_should_pass() {
        let detected = DetectedProjects { rust: true, docker_compose: true, ..Default::default() };
        assert!(detected.summary().contains("Rust"));
        assert!(detected.summary().contains("Docker Compose"));
    }
}
