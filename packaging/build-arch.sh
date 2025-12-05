#!/usr/bin/env bash

# Build script for Arch Linux AUR package
#
# Usage:
#   ./build-arch.sh              # Build package locally
#   ./build-arch.sh --install    # Build and install
#   ./build-arch.sh --srcinfo    # Generate .SRCINFO only
#
# Prerequisites:
#   - Arch Linux or Arch-based distro (Manjaro, EndeavourOS, etc.)
#   - base-devel package group installed
#   - rust toolchain installed

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
ARCH_DIR="$SCRIPT_DIR/arch"
BUILD_DIR="/tmp/expresso-kit-aur-build"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

log_info() { echo -e "${GREEN}[INFO]${NC} $1"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }

check_dependencies() {
    log_info "Checking dependencies..."

    local missing=()

    command -v makepkg &>/dev/null || missing+=("base-devel")
    command -v cargo &>/dev/null || missing+=("rust")
    command -v git &>/dev/null || missing+=("git")

    if [[ ${#missing[@]} -gt 0 ]]; then
        log_error "Missing dependencies: ${missing[*]}"
        log_info "Install with: sudo pacman -S ${missing[*]}"
        exit 1
    fi

    log_info "All dependencies satisfied"
}

generate_srcinfo() {
    log_info "Generating .SRCINFO..."
    cd "$ARCH_DIR"
    makepkg --printsrcinfo > .SRCINFO
    log_info ".SRCINFO generated at $ARCH_DIR/.SRCINFO"
}

build_package() {
    local install_flag="${1:-}"

    log_info "Preparing build directory..."
    rm -rf "$BUILD_DIR"
    mkdir -p "$BUILD_DIR"

    # Copy PKGBUILD to build directory
    cp "$ARCH_DIR/PKGBUILD" "$BUILD_DIR/"

    cd "$BUILD_DIR"

    log_info "Building package..."
    if [[ "$install_flag" == "--install" ]]; then
        makepkg -si --noconfirm
    else
        makepkg -s
    fi

    # Copy built package back
    local pkg_file
    pkg_file=$(ls expresso-kit-*.pkg.tar.* 2>/dev/null | head -1)

    if [[ -n "$pkg_file" ]]; then
        cp "$pkg_file" "$PROJECT_ROOT/"
        log_info "Package built: $PROJECT_ROOT/$pkg_file"
    fi
}

build_git_package() {
    log_info "Building from git (development version)..."

    rm -rf "$BUILD_DIR"
    mkdir -p "$BUILD_DIR"

    cp "$ARCH_DIR/PKGBUILD-git" "$BUILD_DIR/PKGBUILD"

    cd "$BUILD_DIR"
    makepkg -s

    local pkg_file
    pkg_file=$(ls expresso-kit-git-*.pkg.tar.* 2>/dev/null | head -1)

    if [[ -n "$pkg_file" ]]; then
        cp "$pkg_file" "$PROJECT_ROOT/"
        log_info "Git package built: $PROJECT_ROOT/$pkg_file"
    fi
}

print_usage() {
    echo "Usage: $0 [OPTIONS]"
    echo ""
    echo "Options:"
    echo "  --install     Build and install the package"
    echo "  --srcinfo     Generate .SRCINFO only"
    echo "  --git         Build from git source (development)"
    echo "  --help        Show this help message"
    echo ""
    echo "Examples:"
    echo "  $0                  # Build package"
    echo "  $0 --install        # Build and install"
    echo "  $0 --git --install  # Build git version and install"
}

main() {
    local install_flag=""
    local srcinfo_only=false
    local git_build=false

    while [[ $# -gt 0 ]]; do
        case "$1" in
            --install)
                install_flag="--install"
                shift
                ;;
            --srcinfo)
                srcinfo_only=true
                shift
                ;;
            --git)
                git_build=true
                shift
                ;;
            --help|-h)
                print_usage
                exit 0
                ;;
            *)
                log_error "Unknown option: $1"
                print_usage
                exit 1
                ;;
        esac
    done

    check_dependencies

    if $srcinfo_only; then
        generate_srcinfo
        exit 0
    fi

    if $git_build; then
        build_git_package
    else
        build_package "$install_flag"
    fi

    log_info "Done!"
}

main "$@"
