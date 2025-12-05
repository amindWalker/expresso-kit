#!/usr/bin/env bash

# Build and publish to Ubuntu PPA (Launchpad)
#
# Prerequisites:
#   1. Launchpad account: https://launchpad.net
#   2. GPG key uploaded to Launchpad
#   3. PPA created (e.g., ppa:your-username/package-name)
#
# Usage:
#   ./build-ppa.sh                    # Build source package only
#   ./build-ppa.sh --upload           # Build and upload to PPA
#   ./build-ppa.sh --upload --ppa ppa:user/repo  # Specify PPA
#
# Environment variables:
#   DEBFULLNAME  - Your full name (for changelog)
#   DEBEMAIL     - Your email (must match GPG key)
#   GPG_KEY_ID   - Your GPG key ID (optional, auto-detected)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
BUILD_DIR="/tmp/expresso-kit-ppa-build"

# Package info
PKG_NAME="expresso-kit"
PKG_VERSION="0.1.0"

# Ubuntu releases to build for
UBUNTU_RELEASES=("noble" "jammy" "focal")  # 24.04, 22.04, 20.04

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
    log_info "Checking dependencies..."

    local missing=()

    command -v debuild &>/dev/null || missing+=("devscripts")
    command -v dh &>/dev/null || missing+=("debhelper")
    command -v dpkg-source &>/dev/null || missing+=("dpkg-dev")
    command -v gpg &>/dev/null || missing+=("gnupg")
    command -v dput &>/dev/null || missing+=("dput")

    if [[ ${#missing[@]} -gt 0 ]]; then
        log_error "Missing dependencies: ${missing[*]}"
        log_info "Install with: sudo apt install ${missing[*]}"
        exit 1
    fi

    # Check environment variables
    if [[ -z "${DEBFULLNAME:-}" ]]; then
        log_error "DEBFULLNAME not set. Export your name:"
        log_info "  export DEBFULLNAME='Your Full Name'"
        exit 1
    fi

    if [[ -z "${DEBEMAIL:-}" ]]; then
        log_error "DEBEMAIL not set. Export your email:"
        log_info "  export DEBEMAIL='your.email@example.com'"
        exit 1
    fi

    # Check GPG key
    if ! gpg --list-secret-keys "${DEBEMAIL}" &>/dev/null; then
        log_error "No GPG key found for ${DEBEMAIL}"
        log_info "Create one with: gpg --full-generate-key"
        log_info "Then upload to Launchpad: https://launchpad.net/~/+editpgpkeys"
        exit 1
    fi

    log_info "All dependencies satisfied"
}

prepare_source() {
    local release="$1"
    local ppa_version="${PKG_VERSION}-1ppa1~${release}1"

    log_step "Preparing source for ${release} (version ${ppa_version})..."

    local src_dir="${BUILD_DIR}/${PKG_NAME}-${PKG_VERSION}"
    rm -rf "$src_dir"
    mkdir -p "$src_dir"

    # Copy source files (exclude build artifacts)
    cd "$PROJECT_ROOT"
    rsync -av \
        --exclude='target' \
        --exclude='.git' \
        --exclude='node_modules' \
        --exclude='*.deb' \
        --exclude='*.changes' \
        --exclude='*.build' \
        --exclude='*.buildinfo' \
        --exclude='*.dsc' \
        . "$src_dir/"

    # Copy debian directory
    cp -r "$SCRIPT_DIR/debian" "$src_dir/"

    # Update changelog for this release
    cd "$src_dir"

    # Create release-specific changelog entry
    cat > debian/changelog << EOF
${PKG_NAME} (${ppa_version}) ${release}; urgency=medium

  * Build for Ubuntu ${release}
  * See upstream changelog for details

 -- ${DEBFULLNAME} <${DEBEMAIL}>  $(date -R)
EOF

    # Create orig tarball
    cd "$BUILD_DIR"
    tar czf "${PKG_NAME}_${PKG_VERSION}.orig.tar.gz" "${PKG_NAME}-${PKG_VERSION}"

    echo "$src_dir"
}

build_source_package() {
    local src_dir="$1"

    log_step "Building source package..."

    cd "$src_dir"

    # Build source package (signed)
    debuild -S -sa -k"${GPG_KEY_ID:-${DEBEMAIL}}"

    log_info "Source package built successfully"
}

upload_to_ppa() {
    local release="$1"
    local ppa="$2"
    local ppa_version="${PKG_VERSION}-1ppa1~${release}1"

    log_step "Uploading to PPA: ${ppa}..."

    cd "$BUILD_DIR"

    local changes_file="${PKG_NAME}_${ppa_version}_source.changes"

    if [[ ! -f "$changes_file" ]]; then
        log_error "Changes file not found: $changes_file"
        exit 1
    fi

    # Upload to PPA
    dput "$ppa" "$changes_file"

    log_info "Uploaded to ${ppa} for ${release}"
}

print_usage() {
    cat << EOF
Usage: $0 [OPTIONS]

Build source packages for Ubuntu PPA.

Options:
  --upload              Upload to PPA after building
  --ppa PPA             PPA to upload to (default: ppa:${USER}/expresso-kit)
  --release RELEASE     Build only for specific release (e.g., jammy)
  --help                Show this help message

Environment Variables (required):
  DEBFULLNAME           Your full name (for changelog)
  DEBEMAIL              Your email (must match GPG key on Launchpad)
  GPG_KEY_ID            GPG key ID (optional, auto-detected from email)

Examples:
  # Build source packages for all Ubuntu releases
  $0

  # Build and upload to your PPA
  $0 --upload --ppa ppa:myusername/expresso-kit

  # Build only for Ubuntu 22.04 (jammy)
  $0 --release jammy

Setup Steps:
  1. Create Launchpad account: https://launchpad.net/+login
  2. Create GPG key: gpg --full-generate-key
  3. Upload GPG to Launchpad: https://launchpad.net/~/+editpgpkeys
  4. Create PPA: https://launchpad.net/~/+activate-ppa
  5. Export variables:
       export DEBFULLNAME='Your Name'
       export DEBEMAIL='your.email@example.com'
  6. Run: $0 --upload --ppa ppa:yourusername/expresso-kit
EOF
}

main() {
    local do_upload=false
    local ppa="ppa:${USER}/expresso-kit"
    local single_release=""

    while [[ $# -gt 0 ]]; do
        case "$1" in
            --upload)
                do_upload=true
                shift
                ;;
            --ppa)
                ppa="$2"
                shift 2
                ;;
            --release)
                single_release="$2"
                shift 2
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

    # Determine which releases to build
    local releases=("${UBUNTU_RELEASES[@]}")
    if [[ -n "$single_release" ]]; then
        releases=("$single_release")
    fi

    log_info "Building for releases: ${releases[*]}"

    # Clean build directory
    rm -rf "$BUILD_DIR"
    mkdir -p "$BUILD_DIR"

    for release in "${releases[@]}"; do
        log_info "=== Processing ${release} ==="

        local src_dir
        src_dir=$(prepare_source "$release")

        build_source_package "$src_dir"

        if $do_upload; then
            upload_to_ppa "$release" "$ppa"
        fi

        log_info "=== Completed ${release} ==="
        echo
    done

    log_info "All done!"

    if ! $do_upload; then
        log_info "Source packages are in: $BUILD_DIR"
        log_info "To upload, run: $0 --upload --ppa $ppa"
    fi
}

main "$@"
