#!/usr/bin/env bash

# Run nixpkgs CI checks locally with act
#
# Prerequisites:
#   - Docker installed and running
#   - act installed: brew install act (macOS) or see https://github.com/nektos/act
#
# Usage:
#   ./run-nix-ci.sh              # Run all Linux checks
#   ./run-nix-ci.sh --quick      # Run only flake check
#   ./run-nix-ci.sh --full       # Run all checks (requires macOS for darwin)
#   ./run-nix-ci.sh --list       # List available jobs

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m'

log_info() { echo -e "${GREEN}[INFO]${NC} $1"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }
log_step() { echo -e "${CYAN}[STEP]${NC} $1"; }

check_dependencies() {
    if ! command -v docker &>/dev/null; then
        log_error "Docker is not installed or not running"
        log_info "Install Docker: https://docs.docker.com/get-docker/"
        exit 1
    fi

    if ! docker info &>/dev/null; then
        log_error "Docker daemon is not running"
        log_info "Start Docker and try again"
        exit 1
    fi

    if ! command -v act &>/dev/null; then
        log_error "act is not installed"
        log_info "Install act:"
        log_info "  macOS: brew install act"
        log_info "  Linux: curl -s https://raw.githubusercontent.com/nektos/act/master/install.sh | sudo bash"
        log_info "  Other: https://github.com/nektos/act#installation"
        exit 1
    fi

    log_info "All dependencies satisfied"
}

list_jobs() {
    log_step "Available CI jobs in nix-ci.yml:"
    echo ""
    cd "$PROJECT_ROOT"
    act --list -W .github/workflows/nix-ci.yml 2>/dev/null || {
        # Fallback: parse YAML manually
        grep -E "^  [a-z].*:$" .github/workflows/nix-ci.yml | sed 's/://g' | sed 's/^  /  - /'
    }
}

run_quick_check() {
    log_step "Running quick flake check..."
    cd "$PROJECT_ROOT"

    act -j nix-flake-check \
        -W .github/workflows/nix-ci.yml \
        --container-architecture linux/amd64 \
        --env ACT=true \
        --bind \
        -P ubuntu-latest=catthehacker/ubuntu:act-latest
}

run_linux_checks() {
    log_step "Running all Linux checks..."
    cd "$PROJECT_ROOT"

    # Run flake check
    log_info "Running flake checks..."
    act -j nix-flake-check \
        -W .github/workflows/nix-ci.yml \
        --container-architecture linux/amd64 \
        --env ACT=true \
        --bind \
        -P ubuntu-latest=catthehacker/ubuntu:act-latest

    # Run Linux x86_64 build
    log_info "Running x86_64-linux build..."
    act -j nix-build-linux \
        -W .github/workflows/nix-ci.yml \
        --container-architecture linux/amd64 \
        --env ACT=true \
        --bind \
        -P ubuntu-latest=catthehacker/ubuntu:act-latest \
        --matrix arch:x86_64

    # Run package validation
    log_info "Running package validation..."
    act -j nix-package-validation \
        -W .github/workflows/nix-ci.yml \
        --container-architecture linux/amd64 \
        --env ACT=true \
        --bind \
        -P ubuntu-latest=catthehacker/ubuntu:act-latest

    # Run code quality
    log_info "Running code quality checks..."
    act -j nix-code-quality \
        -W .github/workflows/nix-ci.yml \
        --container-architecture linux/amd64 \
        --env ACT=true \
        --bind \
        -P ubuntu-latest=catthehacker/ubuntu:act-latest
}

run_single_job() {
    local job="$1"
    log_step "Running job: $job"
    cd "$PROJECT_ROOT"

    act -j "$job" \
        -W .github/workflows/nix-ci.yml \
        --container-architecture linux/amd64 \
        --env ACT=true \
        --bind \
        -P ubuntu-latest=catthehacker/ubuntu:act-latest
}

run_with_nix_directly() {
    log_step "Running checks with Nix directly (faster than act)..."
    cd "$PROJECT_ROOT"

    if ! command -v nix &>/dev/null; then
        log_error "Nix is not installed"
        log_info "Install Nix: curl --proto '=https' --tlsv1.2 -sSf -L https://install.determinate.systems/nix | sh -s -- install"
        exit 1
    fi

    log_info "Checking flake..."
    nix flake check --print-build-logs

    log_info "Building package..."
    nix build .#expresso-kit --print-build-logs

    log_info "Testing binary..."
    ./result/bin/expresso-kit --version
    ./result/bin/expresso-kit --help

    log_info "Validating package.nix..."
    nix-instantiate --parse packaging/nix/package.nix > /dev/null

    log_info "Checking meta attributes..."
    nix eval --json .#packages.x86_64-linux.expresso-kit.meta | jq .

    log_info "All Nix checks passed!"
}

print_usage() {
    cat << EOF
Usage: $0 [OPTIONS] [JOB_NAME]

Run nixpkgs CI checks locally using act or nix directly.

Options:
  (no args)       Run all Linux checks with act
  --quick         Run only flake check (fastest)
  --nix           Run checks directly with Nix (faster than act)
  --list          List all available jobs
  --full          Run all checks (Linux only with act)
  --help          Show this help message

Job Names (run specific job):
  nix-flake-check         Flake structure and evaluation
  nix-build-linux         Build on Linux (x86_64)
  nix-package-validation  Validate package.nix for nixpkgs
  nix-code-quality        Formatting and linting

Examples:
  $0                              # Run all Linux checks
  $0 --quick                      # Quick flake check only
  $0 --nix                        # Use Nix directly (fastest)
  $0 nix-package-validation       # Run specific job
  $0 --list                       # Show available jobs

Notes:
  - macOS (Darwin) builds cannot run in act, use actual GitHub Actions
  - ARM (aarch64) builds require proper ARM Docker setup
  - For fastest testing, use: $0 --nix

EOF
}

print_summary() {
    echo ""
    log_info "============================================"
    log_info "nixpkgs PR Readiness Summary"
    log_info "============================================"
    echo ""
    echo "If all checks passed, your package is ready for nixpkgs PR!"
    echo ""
    echo "Next steps:"
    echo "  1. Tag a release: git tag v\$(grep 'version = ' Cargo.toml | head -1 | cut -d'\"' -f2)"
    echo "  2. Calculate hash: ./packaging/build-nix.sh --hash"
    echo "  3. Update package.nix with the correct hash"
    echo "  4. Prepare PR: ./packaging/build-nix.sh --nixpkgs-pr"
    echo ""
}

main() {
    case "${1:-}" in
        --quick)
            check_dependencies
            run_quick_check
            ;;
        --nix)
            run_with_nix_directly
            print_summary
            ;;
        --list)
            list_jobs
            ;;
        --full)
            check_dependencies
            run_linux_checks
            print_summary
            ;;
        --help|-h)
            print_usage
            ;;
        nix-*)
            check_dependencies
            run_single_job "$1"
            ;;
        "")
            check_dependencies
            run_linux_checks
            print_summary
            ;;
        *)
            log_error "Unknown option: $1"
            print_usage
            exit 1
            ;;
    esac
}

main "$@"
