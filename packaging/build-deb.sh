#!/usr/bin/env bash

# Build script for Ubuntu/Debian .deb package
#
# Usage:
#   ./build-deb.sh              # Build .deb package
#   ./build-deb.sh --install    # Build and install
#   ./build-deb.sh --docker     # Build using Docker (cross-platform)
#
# Prerequisites (native build):
#   - Ubuntu/Debian system
#   - build-essential, debhelper, cargo, rustc
#
# Prerequisites (Docker build):
#   - Docker installed and running

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
DEBIAN_DIR="$SCRIPT_DIR/debian"
BUILD_DIR="/tmp/expresso-kit-deb-build"

# Package info
PKG_NAME="expresso-kit"
PKG_VERSION="0.1.0"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

log_info() { echo -e "${GREEN}[INFO]${NC} $1"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }

check_native_dependencies() {
    log_info "Checking dependencies..."

    local missing=()

    command -v dpkg-buildpackage &>/dev/null || missing+=("dpkg-dev")
    command -v debhelper &>/dev/null || command -v dh &>/dev/null || missing+=("debhelper")
    command -v cargo &>/dev/null || missing+=("cargo")
    command -v rustc &>/dev/null || missing+=("rustc")

    if [[ ${#missing[@]} -gt 0 ]]; then
        log_error "Missing dependencies: ${missing[*]}"
        log_info "Install with: sudo apt install build-essential debhelper cargo rustc"
        exit 1
    fi

    log_info "All dependencies satisfied"
}

build_native() {
    local install_flag="${1:-}"

    log_info "Preparing build directory..."
    rm -rf "$BUILD_DIR"
    mkdir -p "$BUILD_DIR/${PKG_NAME}-${PKG_VERSION}"

    # Copy source files
    log_info "Copying source files..."
    cd "$PROJECT_ROOT"

    # Copy everything except target and .git
    rsync -av --exclude='target' --exclude='.git' --exclude='node_modules' \
        . "$BUILD_DIR/${PKG_NAME}-${PKG_VERSION}/"

    # Copy debian directory
    cp -r "$DEBIAN_DIR" "$BUILD_DIR/${PKG_NAME}-${PKG_VERSION}/debian"

    cd "$BUILD_DIR/${PKG_NAME}-${PKG_VERSION}"

    log_info "Building .deb package..."
    dpkg-buildpackage -us -uc -b

    # Copy built package
    cd "$BUILD_DIR"
    local deb_file
    deb_file=$(ls ${PKG_NAME}_*.deb 2>/dev/null | head -1)

    if [[ -n "$deb_file" ]]; then
        cp "$deb_file" "$PROJECT_ROOT/"
        log_info "Package built: $PROJECT_ROOT/$deb_file"

        if [[ "$install_flag" == "--install" ]]; then
            log_info "Installing package..."
            sudo dpkg -i "$PROJECT_ROOT/$deb_file"
            sudo apt-get install -f -y  # Fix any dependency issues
        fi
    else
        log_error "No .deb file found after build"
        exit 1
    fi
}

build_docker() {
    local install_flag="${1:-}"

    log_info "Building using Docker..."

    # Create temporary Dockerfile
    local dockerfile="$BUILD_DIR/Dockerfile.deb"
    mkdir -p "$BUILD_DIR"

    cat > "$dockerfile" << 'DOCKERFILE'
FROM ubuntu:22.04

ENV DEBIAN_FRONTEND=noninteractive

# Install build dependencies
RUN apt-get update && apt-get install -y \
    build-essential \
    debhelper \
    devscripts \
    curl \
    git \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Install Rust
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
ENV PATH="/root/.cargo/bin:${PATH}"

WORKDIR /build

# Build script
COPY build-internal.sh /build-internal.sh
RUN chmod +x /build-internal.sh

ENTRYPOINT ["/build-internal.sh"]
DOCKERFILE

    # Create internal build script
    cat > "$BUILD_DIR/build-internal.sh" << 'BUILDSCRIPT'
#!/bin/bash
set -euo pipefail

PKG_NAME="expresso-kit"
PKG_VERSION="0.1.0"

cd /build
mkdir -p "${PKG_NAME}-${PKG_VERSION}"

# Copy source (mounted at /src)
cp -r /src/* "${PKG_NAME}-${PKG_VERSION}/" 2>/dev/null || true
cp -r /src/.[!.]* "${PKG_NAME}-${PKG_VERSION}/" 2>/dev/null || true

# Ensure debian directory exists
if [[ ! -d "${PKG_NAME}-${PKG_VERSION}/debian" ]]; then
    cp -r /src/packaging/debian "${PKG_NAME}-${PKG_VERSION}/"
fi

cd "${PKG_NAME}-${PKG_VERSION}"

# Build package
dpkg-buildpackage -us -uc -b

# Copy output
cp /build/*.deb /output/ 2>/dev/null || true
BUILDSCRIPT

    # Build Docker image
    log_info "Building Docker image..."
    docker build -t expresso-kit-deb-builder -f "$dockerfile" "$BUILD_DIR"

    # Run build in container
    mkdir -p "$BUILD_DIR/output"

    log_info "Running build in container..."
    docker run --rm \
        -v "$PROJECT_ROOT:/src:ro" \
        -v "$BUILD_DIR/output:/output" \
        expresso-kit-deb-builder

    # Copy output
    local deb_file
    deb_file=$(ls "$BUILD_DIR/output"/*.deb 2>/dev/null | head -1)

    if [[ -n "$deb_file" ]]; then
        cp "$deb_file" "$PROJECT_ROOT/"
        log_info "Package built: $PROJECT_ROOT/$(basename "$deb_file")"

        if [[ "$install_flag" == "--install" ]]; then
            log_info "Installing package..."
            sudo dpkg -i "$PROJECT_ROOT/$(basename "$deb_file")"
            sudo apt-get install -f -y
        fi
    else
        log_error "No .deb file found after Docker build"
        exit 1
    fi
}

# Simple binary .deb build (without full Debian tooling)
build_simple() {
    local install_flag="${1:-}"

    log_info "Building simple .deb package..."

    # Build the binary first
    log_info "Compiling binary..."
    cd "$PROJECT_ROOT"
    cargo build --release --locked

    local deb_dir="$BUILD_DIR/${PKG_NAME}_${PKG_VERSION}_amd64"
    rm -rf "$deb_dir"
    mkdir -p "$deb_dir/DEBIAN"
    mkdir -p "$deb_dir/usr/bin"
    mkdir -p "$deb_dir/usr/share/doc/${PKG_NAME}"

    # Copy binary
    cp "target/release/${PKG_NAME}" "$deb_dir/usr/bin/"
    chmod 755 "$deb_dir/usr/bin/${PKG_NAME}"

    # Copy documentation
    cp README.md "$deb_dir/usr/share/doc/${PKG_NAME}/"
    cp GUIDE.md "$deb_dir/usr/share/doc/${PKG_NAME}/"
    cp LICENSE "$deb_dir/usr/share/doc/${PKG_NAME}/copyright"

    # Create control file
    cat > "$deb_dir/DEBIAN/control" << EOF
Package: ${PKG_NAME}
Version: ${PKG_VERSION}
Section: devel
Priority: optional
Architecture: amd64
Depends: libc6 (>= 2.31), libgcc-s1 (>= 3.0)
Recommends: docker.io, docker-compose, git
Maintainer: Breno Rocha <bhrochamail@gmail.com>
Homepage: https://github.com/amindWalker/expresso-kit
Description: Automated project validator for local/cloud environments
 EspressoKit is an automated project validator that validates repository
 configurations for local/cloud environments with tests against CI
 deployment to guarantee idempotent and reproducible results.
EOF

    # Calculate installed size
    local installed_size
    installed_size=$(du -sk "$deb_dir" | cut -f1)
    echo "Installed-Size: $installed_size" >> "$deb_dir/DEBIAN/control"

    # Build .deb
    log_info "Creating .deb package..."
    dpkg-deb --build "$deb_dir"

    # Move to project root
    mv "${deb_dir}.deb" "$PROJECT_ROOT/"

    log_info "Package built: $PROJECT_ROOT/${PKG_NAME}_${PKG_VERSION}_amd64.deb"

    if [[ "$install_flag" == "--install" ]]; then
        log_info "Installing package..."
        sudo dpkg -i "$PROJECT_ROOT/${PKG_NAME}_${PKG_VERSION}_amd64.deb"
        sudo apt-get install -f -y
    fi
}

print_usage() {
    echo "Usage: $0 [OPTIONS]"
    echo ""
    echo "Options:"
    echo "  --install     Build and install the package"
    echo "  --docker      Build using Docker (works on any platform)"
    echo "  --simple      Build simple .deb without full Debian tooling"
    echo "  --help        Show this help message"
    echo ""
    echo "Examples:"
    echo "  $0                  # Build using native tools"
    echo "  $0 --install        # Build and install"
    echo "  $0 --docker         # Build using Docker"
    echo "  $0 --simple         # Build simple .deb (cargo + dpkg-deb only)"
}

main() {
    local install_flag=""
    local use_docker=false
    local use_simple=false

    while [[ $# -gt 0 ]]; do
        case "$1" in
            --install)
                install_flag="--install"
                shift
                ;;
            --docker)
                use_docker=true
                shift
                ;;
            --simple)
                use_simple=true
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

    if $use_docker; then
        command -v docker &>/dev/null || { log_error "Docker not found"; exit 1; }
        build_docker "$install_flag"
    elif $use_simple; then
        command -v cargo &>/dev/null || { log_error "cargo not found"; exit 1; }
        command -v dpkg-deb &>/dev/null || { log_error "dpkg-deb not found"; exit 1; }
        build_simple "$install_flag"
    else
        check_native_dependencies
        build_native "$install_flag"
    fi

    log_info "Done!"
}

main "$@"
