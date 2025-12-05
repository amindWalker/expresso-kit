#!/usr/bin/env bash

# Build and test Nix package for expresso-kit
#
# Usage:
#   ./build-nix.sh              # Build package
#   ./build-nix.sh --check      # Run all checks
#   ./build-nix.sh --shell      # Enter development shell
#   ./build-nix.sh --hash       # Calculate source hash for nixpkgs PR
#   ./build-nix.sh --update     # Update flake inputs
#
# For nixpkgs PR preparation:
#   ./build-nix.sh --nixpkgs-pr

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
NIX_DIR="$SCRIPT_DIR/nix"

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

check_nix() {
    if ! command -v nix &>/dev/null; then
        log_error "Nix is not installed"
        log_info "Install Nix: curl --proto '=https' --tlsv1.2 -sSf -L https://install.determinate.systems/nix | sh -s -- install"
        exit 1
    fi

    # Check if flakes are enabled
    if ! nix flake --help &>/dev/null 2>&1; then
        log_error "Nix flakes are not enabled"
        log_info "Add to ~/.config/nix/nix.conf:"
        log_info "  experimental-features = nix-command flakes"
        exit 1
    fi
}

build_package() {
    log_step "Building expresso-kit with Nix..."

    cd "$NIX_DIR"
    nix build .#expresso-kit --print-build-logs

    if [[ -L result ]]; then
        log_info "Build successful!"
        log_info "Binary: $(readlink -f result)/bin/expresso-kit"

        # Show version
        ./result/bin/expresso-kit --version || true
    fi
}

run_checks() {
    log_step "Running Nix flake checks..."

    cd "$NIX_DIR"
    nix flake check --print-build-logs

    log_info "All checks passed!"
}

enter_shell() {
    log_step "Entering development shell..."

    cd "$NIX_DIR"
    nix develop
}

calculate_hash() {
    log_step "Calculating source hash for nixpkgs..."

    local version
    version=$(grep -m1 'version = ' "$PROJECT_ROOT/Cargo.toml" | cut -d'"' -f2)

    log_info "Version: $version"
    log_info "Repository: https://github.com/amindWalker/expresso-kit"

    # Use nix-prefetch-github for accurate hash
    if command -v nix-prefetch-github &>/dev/null; then
        log_info "Calculating hash with nix-prefetch-github..."
        nix-prefetch-github amindWalker expresso-kit --rev "v${version}" 2>/dev/null || {
            log_warn "Tag v${version} not found, using HEAD"
            nix-prefetch-github amindWalker expresso-kit
        }
    else
        log_info "Calculating hash with nix-prefetch-url..."
        local url="https://github.com/amindWalker/expresso-kit/archive/v${version}.tar.gz"
        local hash
        hash=$(nix-prefetch-url --unpack "$url" 2>/dev/null) || {
            log_warn "Tag v${version} not found, trying main branch"
            url="https://github.com/amindWalker/expresso-kit/archive/refs/heads/main.tar.gz"
            hash=$(nix-prefetch-url --unpack "$url" 2>/dev/null)
        }

        # Convert to SRI format
        local sri_hash
        sri_hash=$(nix hash to-sri --type sha256 "$hash")

        log_info "SHA256 (SRI): $sri_hash"
    fi
}

update_flake() {
    log_step "Updating flake inputs..."

    cd "$NIX_DIR"
    nix flake update

    log_info "Flake inputs updated"
}

prepare_nixpkgs_pr() {
    log_step "Preparing package for nixpkgs PR..."

    local version
    version=$(grep -m1 'version = ' "$PROJECT_ROOT/Cargo.toml" | cut -d'"' -f2)

    local nixpkgs_dir="$SCRIPT_DIR/nixpkgs-pr"
    local pkg_dir="$nixpkgs_dir/pkgs/by-name/ex/expresso-kit"

    rm -rf "$nixpkgs_dir"
    mkdir -p "$pkg_dir"

    # Copy and update the package
    cp "$NIX_DIR/package.nix" "$pkg_dir/package.nix"

    log_info "Package prepared at: $pkg_dir"
    log_info ""
    log_info "Next steps for nixpkgs PR:"
    log_info "1. Fork https://github.com/NixOS/nixpkgs"
    log_info "2. Clone your fork locally"
    log_info "3. Create a new branch: git checkout -b expresso-kit-init"
    log_info "4. Copy the package:"
    log_info "   cp -r $pkg_dir path/to/nixpkgs/pkgs/by-name/ex/expresso-kit"
    log_info "5. Update the hash in package.nix (run: ./build-nix.sh --hash)"
    log_info "6. Test the build: nix-build -A expresso-kit"
    log_info "7. Run nixpkgs checks: nix-build -A nixosTests.expresso-kit (if applicable)"
    log_info "8. Commit with message: expresso-kit: init at ${version}"
    log_info "9. Push and create PR"
    log_info ""
    log_info "PR checklist:"
    log_info "  [ ] Package builds on x86_64-linux"
    log_info "  [ ] Package builds on aarch64-linux"
    log_info "  [ ] Package builds on x86_64-darwin"
    log_info "  [ ] Package builds on aarch64-darwin"
    log_info "  [ ] meta.description is set and accurate"
    log_info "  [ ] meta.license matches upstream"
    log_info "  [ ] meta.maintainers includes your name (after becoming maintainer)"
    log_info "  [ ] meta.homepage is correct"
    log_info "  [ ] Version matches upstream"
}

print_usage() {
    cat << EOF
Usage: $0 [OPTIONS]

Build and manage Nix packages for expresso-kit.

Options:
  (no args)       Build the package
  --check         Run all Nix flake checks (build, clippy, fmt)
  --shell         Enter Nix development shell
  --hash          Calculate source hash for nixpkgs PR
  --update        Update flake.lock inputs
  --nixpkgs-pr    Prepare package for nixpkgs PR submission
  --help          Show this help message

Installation methods:

  # With flakes (recommended)
  nix profile install github:amindWalker/expresso-kit

  # From local checkout
  cd packaging/nix && nix build && ./result/bin/expresso-kit

  # Development shell
  cd packaging/nix && nix develop

  # NixOS configuration
  { pkgs, ... }: {
    environment.systemPackages = [
      (pkgs.callPackage ./packaging/nix/package.nix { })
    ];
  }

  # Home Manager
  { pkgs, ... }: {
    home.packages = [
      (pkgs.callPackage ./packaging/nix/package.nix { })
    ];
  }

EOF
}

main() {
    case "${1:-}" in
        --check)
            check_nix
            run_checks
            ;;
        --shell)
            check_nix
            enter_shell
            ;;
        --hash)
            check_nix
            calculate_hash
            ;;
        --update)
            check_nix
            update_flake
            ;;
        --nixpkgs-pr)
            prepare_nixpkgs_pr
            ;;
        --help|-h)
            print_usage
            ;;
        "")
            check_nix
            build_package
            ;;
        *)
            log_error "Unknown option: $1"
            print_usage
            exit 1
            ;;
    esac
}

main "$@"
