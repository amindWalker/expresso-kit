# Packaging

This directory contains packaging scripts and configuration files for building distributable packages of `expresso-kit`.

## Supported Formats

| Format | Platform | Directory |
|--------|----------|-----------|
| **Nix** | Any Linux/macOS (x86_64, aarch64) | `nix/` |
| **AUR** | Arch Linux | `arch/` |
| **.deb** | Ubuntu/Debian | `debian/` |
| **PPA** | Ubuntu (Launchpad) | `debian/` + `build-ppa.sh` |

## Nix (Universal - Linux & macOS)

Nix works on **any Linux distribution** and **macOS**, supporting both **x86_64** and **aarch64** architectures.

### Installing with Flakes (Recommended)

```bash
# Run directly without installing
nix run github:amindWalker/expresso-kit

# Install to profile
nix profile install github:amindWalker/expresso-kit

# Or from local checkout
nix profile install .#expresso-kit
```

### Installing with Legacy Nix

```bash
# Build and run
nix-build && ./result/bin/expresso-kit

# Install to profile
nix-env -if .
```

### NixOS Configuration

```nix
# In your configuration.nix or home.nix
{ pkgs, ... }: {
  environment.systemPackages = [
    (pkgs.callPackage /path/to/expresso-kit/packaging/nix/package.nix { })
  ];
}
```

### Development Shell

```bash
# With flakes
nix develop

# Without flakes
nix-shell
```

See [Nix Packaging Guide](#nix-packaging-guide) below for nixpkgs PR submission.

## Installing from PPA (Ubuntu Users)

Once published, users can install with:

```bash
# Add the PPA
sudo add-apt-repository ppa:amindwalker/expresso-kit
sudo apt update

# Install
sudo apt install expresso-kit
```
## Arch Linux (AUR)

### Prerequisites

```bash
# Install base development tools
sudo pacman -S base-devel git rust
```

### Building Locally

```bash
cd packaging

# Option 1: Use the build script
./build-arch.sh

# Option 2: Build and install
./build-arch.sh --install

# Option 3: Build from git (development version)
./build-arch.sh --git
```

### Manual Build

```bash
cd arch

# Build the package
makepkg -si

# Or without installing
makepkg -s
```

### Publishing to AUR

1. Create an AUR account at https://aur.archlinux.org
2. Set up SSH keys for AUR (`ssh-keygen` locally then paste public key in AUR)
3. Clone the AUR package repository:
   ```bash
   git clone ssh://aur@aur.archlinux.org/expresso-kit.git aur-expresso-kit
   ```
4. Copy PKGBUILD and .SRCINFO:
   ```bash
   cp arch/PKGBUILD aur-expresso-kit/
   cd aur-expresso-kit
   makepkg --printsrcinfo > .SRCINFO
   ```
5. Commit and push:
   ```bash
   git add PKGBUILD .SRCINFO
   git commit -m "Update to version X.Y.Z"
   git push
   ```

### Package Variants

| File | Description |
|------|-------------|
| `PKGBUILD` | Release version (from tagged releases) |
| `PKGBUILD-git` | Git version (latest development) |


## Ubuntu/Debian (.deb) - Local Build

### Prerequisites

```bash
# Install build tools
sudo apt update
sudo apt install build-essential debhelper devscripts cargo rustc
```

### Building Methods

#### Method 1: Simple Build (Recommended for quick testing)

Uses just `cargo` and `dpkg-deb`:

```bash
./build-deb.sh --simple
```

#### Method 2: Full Debian Build

Uses the complete Debian packaging toolchain:

```bash
./build-deb.sh
```

#### Method 3: Docker Build (Cross-platform)

Build on any platform (macOS, Windows, other Linux):

```bash
./build-deb.sh --docker
```

### Installing

```bash
# Build and install in one step
./build-deb.sh --simple --install

# Or install manually
sudo dpkg -i expresso-kit_0.1.0_amd64.deb
sudo apt-get install -f  # Fix dependencies if needed
```

### Uninstalling

```bash
sudo apt remove expresso-kit
```

## Ubuntu PPA (Launchpad)

PPAs allow Ubuntu users to install with `apt` and receive automatic updates.

### One-Time Setup

#### 1. Create Launchpad Account

Go to https://launchpad.net/+login and sign in with Ubuntu One.

#### 2. Create GPG Key

```bash
# Generate a new GPG key
gpg --full-generate-key
# Choose: RSA and RSA, 4096 bits, key does not expire
# Enter your name and email (must match Launchpad account)

# List your keys to get the ID
gpg --list-secret-keys --keyid-format LONG
# Output shows something like: sec rsa4096/ABCD1234EFGH5678

# Export and upload to Ubuntu keyserver
gpg --keyserver keyserver.ubuntu.com --send-keys ABCD1234EFGH5678
```

#### 3. Add GPG Key to Launchpad

1. Go to https://launchpad.net/~/+editpgpkeys
2. Paste your GPG fingerprint
3. Click "Import Key"
4. Check your email and follow the verification link

#### 4. Create a PPA

1. Go to https://launchpad.net/~/+activate-ppa
2. Name: `expresso-kit`
3. Display name: `Expresso Kit`
4. Description: `Automated project validator for local/cloud environments`
5. Click "Activate"

#### 5. Set Environment Variables

Add to your `~/.bashrc` or `~/.zshrc`:

```bash
export DEBFULLNAME='Your Full Name'
export DEBEMAIL='your.email@example.com'
```

### Building and Uploading to PPA

```bash
cd packaging

# Build source packages for all Ubuntu releases
./build-ppa.sh

# Build and upload to your PPA
./build-ppa.sh --upload --ppa ppa:yourusername/expresso-kit

# Build only for Ubuntu 22.04 (jammy)
./build-ppa.sh --release jammy --upload --ppa ppa:yourusername/expresso-kit
```

### Supported Ubuntu Releases

The PPA script builds for:

| Release | Codename | Status |
|---------|----------|--------|
| Ubuntu 24.04 | noble | LTS |
| Ubuntu 22.04 | jammy | LTS |
| Ubuntu 20.04 | focal | LTS |

### After Upload

1. Go to your PPA page: `https://launchpad.net/~yourusername/+archive/ubuntu/expresso-kit`
2. Wait for builds to complete (can take 30-60 minutes)
3. Check build logs if there are failures

### Users Installing from Your PPA

```bash
sudo add-apt-repository ppa:yourusername/expresso-kit
sudo apt update
sudo apt install expresso-kit
```

## Version Bumping

When releasing a new version, update these files:

1. **Cargo.toml** (workspace): Update `version = "X.Y.Z"`
2. **arch/PKGBUILD**: Update `pkgver=X.Y.Z`
3. **arch/.SRCINFO**: Regenerate with `makepkg --printsrcinfo > .SRCINFO`
4. **debian/changelog**: Add new entry at the top
5. **build-ppa.sh**: Update `PKG_VERSION`
6. **build-deb.sh**: Update `PKG_VERSION`

### Example: Bumping to 0.2.0

```bash
# 1. Update Cargo.toml
sed -i 's/version = "0.1.0"/version = "0.2.0"/' Cargo.toml

# 2. Update PKGBUILD
sed -i 's/pkgver=0.1.0/pkgver=0.2.0/' packaging/arch/PKGBUILD

# 3. Regenerate .SRCINFO
cd packaging/arch && makepkg --printsrcinfo > .SRCINFO && cd ../..

# 4. Update build scripts
sed -i 's/PKG_VERSION="0.1.0"/PKG_VERSION="0.2.0"/' packaging/build-*.sh

# 5. Update debian/changelog
cd packaging/debian
dch -v 0.2.0-1 "New upstream release"
```

## Checksums

After creating a release tarball, update the `sha256sums` in `PKGBUILD`:

```bash
# Generate checksum
sha256sum expresso-kit-0.1.0.tar.gz

# Update PKGBUILD
sha256sums=('your_checksum_here')
```

## CI/CD Integration

The build scripts can be integrated into GitHub Actions:

```yaml
# .github/workflows/package.yml
name: Build Packages

on:
  release:
    types: [published]

jobs:
  build-deb:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Install Rust
        uses: dtolnay/rust-action@stable
      - name: Build .deb
        run: ./packaging/build-deb.sh --simple
      - name: Upload artifact
        uses: actions/upload-artifact@v4
        with:
          name: deb-package
          path: "*.deb"

  build-arch:
    runs-on: ubuntu-latest
    container: archlinux:latest
    steps:
      - uses: actions/checkout@v4
      - name: Install dependencies
        run: pacman -Syu --noconfirm base-devel rust git
      - name: Build package
        run: |
          useradd -m builder
          chown -R builder:builder .
          su builder -c "./packaging/build-arch.sh"
      - name: Upload artifact
        uses: actions/upload-artifact@v4
        with:
          name: arch-package
          path: "*.pkg.tar.*"

  upload-ppa:
    runs-on: ubuntu-latest
    if: startsWith(github.ref, 'refs/tags/')
    steps:
      - uses: actions/checkout@v4
      - name: Install dependencies
        run: sudo apt-get install -y devscripts debhelper dput gnupg
      - name: Import GPG key
        run: echo "${{ secrets.GPG_PRIVATE_KEY }}" | gpg --import
      - name: Build and upload to PPA
        env:
          DEBFULLNAME: ${{ secrets.DEBFULLNAME }}
          DEBEMAIL: ${{ secrets.DEBEMAIL }}
        run: ./packaging/build-ppa.sh --upload --ppa ppa:amindwalker/expresso-kit
```

## Troubleshooting

### Arch Linux

**Error: "PKGBUILD does not exist"**
```bash
cd packaging/arch  # Make sure you're in the right directory
```

**Error: "Cannot find package"**
```bash
# The source tarball doesn't exist yet
# Use PKGBUILD-git for building from local git
cp PKGBUILD-git PKGBUILD
makepkg -si
```

### Debian / Local Build

**Error: "cargo: command not found"**
```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env
```

**Error: "dpkg-buildpackage failed"**
```bash
# Use simple build method instead
./build-deb.sh --simple
```

### PPA / Launchpad

**Error: "GPG key not found"**
```bash
# Make sure your key is uploaded to Ubuntu keyserver
gpg --keyserver keyserver.ubuntu.com --send-keys YOUR_KEY_ID

# Wait a few minutes, then verify
gpg --keyserver keyserver.ubuntu.com --recv-keys YOUR_KEY_ID
```

**Error: "Unable to find distroseries"**
```bash
# Check if the Ubuntu release codename is correct
# Valid codenames: noble (24.04), jammy (22.04), focal (20.04)
./build-ppa.sh --release jammy
```

**Error: "Signature verification failed"**
```bash
# Your GPG key might not be on Launchpad
# Go to: https://launchpad.net/~/+editpgpkeys
# Add your key fingerprint
```

**Build fails on Launchpad**
- Check the build log on your PPA page
- Common issues:
  - Missing build dependencies → Update `debian/control`
  - Network issues → PPA builders have limited network access
  - Rust version → Launchpad may have older Rust, script installs rustup

### Nix

**Error: "experimental feature 'flakes' is disabled"**
```bash
# Enable flakes in ~/.config/nix/nix.conf:
mkdir -p ~/.config/nix
echo "experimental-features = nix-command flakes" >> ~/.config/nix/nix.conf
```

**Error: "hash mismatch"**
```bash
# Recalculate the hash
./build-nix.sh --hash
# Update package.nix with the new hash
```

**Build fails on Darwin (macOS)**
- Ensure Xcode Command Line Tools are installed: `xcode-select --install`
- The package includes Darwin-specific frameworks (Security, SystemConfiguration)

## Testing with `act` (Local CI)

Run nixpkgs-compatible CI checks locally before pushing, using [act](https://github.com/nektos/act).

### Installing `act`

```bash
# macOS
brew install act

# Linux
curl -s https://raw.githubusercontent.com/nektos/act/master/install.sh | sudo bash

# Nix
nix-shell -p act
```

### Quick Start

```bash
cd packaging

# Run all Linux checks
./run-nix-ci.sh

# Quick flake check only (fastest with act)
./run-nix-ci.sh --quick

# Use Nix directly (fastest overall)
./run-nix-ci.sh --nix

# List all available jobs
./run-nix-ci.sh --list
```

### Running Specific Jobs

```bash
# Run flake checks
act -j nix-flake-check -W .github/workflows/nix-ci.yml

# Run Linux build
act -j nix-build-linux -W .github/workflows/nix-ci.yml --matrix arch:x86_64

# Run package validation
act -j nix-package-validation -W .github/workflows/nix-ci.yml

# Run code quality checks
act -j nix-code-quality -W .github/workflows/nix-ci.yml
```

### What Gets Tested

The CI workflow (`nix-ci.yml`) mirrors nixpkgs CI:

| Job | Description | Platforms |
|-----|-------------|-----------|
| `nix-flake-check` | Flake structure & evaluation | Linux |
| `nix-build-linux` | Full build | x86_64, aarch64 |
| `nix-build-darwin` | Full build | x86_64, aarch64 (GitHub only) |
| `nix-package-validation` | Validate package.nix | Linux |
| `nix-code-quality` | Formatting, Clippy | Linux |
| `nix-pr-summary` | Generate PR readiness report | Linux |

### Limitations

- **macOS builds**: Cannot run in act, must use actual GitHub Actions
- **ARM (aarch64)**: Requires ARM Docker setup
- **Speed**: act is slower than native Nix; use `./run-nix-ci.sh --nix` for fastest testing

### Recommended Workflow

1. **During development**: Use `./run-nix-ci.sh --nix` for fast iteration
2. **Before commit**: Run `./run-nix-ci.sh --quick` to validate flake
3. **Before PR**: Run `./run-nix-ci.sh` for full Linux checks
4. **Final validation**: Push to GitHub for macOS builds

## Nix Packaging Guide

### Submitting to nixpkgs

Follow these steps for a high-quality nixpkgs PR:

#### 1. Prepare the Package

```bash
cd packaging
./build-nix.sh --nixpkgs-pr
```

#### 2. Fork and Clone nixpkgs

```bash
git clone https://github.com/NixOS/nixpkgs.git
cd nixpkgs
git checkout -b expresso-kit-init
```

#### 3. Add the Package

```bash
# Create package directory (new by-name structure)
mkdir -p pkgs/by-name/ex/expresso-kit

# Copy package.nix
cp /path/to/expresso-kit/packaging/nix/package.nix pkgs/by-name/ex/expresso-kit/
```

#### 4. Calculate Correct Hash

```bash
# Method 1: Using nix-prefetch-github
nix-shell -p nix-prefetch-github
nix-prefetch-github amindWalker expresso-kit --rev v0.1.0

# Method 2: Build and let Nix tell you
nix-build -A expresso-kit
# Copy the correct hash from the error message
```

#### 5. Test All Platforms

```bash
# Test on your platform
nix-build -A expresso-kit

# Test with --system flag (requires proper setup)
nix-build -A expresso-kit --system x86_64-linux
nix-build -A expresso-kit --system aarch64-linux
nix-build -A expresso-kit --system x86_64-darwin
nix-build -A expresso-kit --system aarch64-darwin
```

#### 6. Run nixpkgs Checks

```bash
# Check package evaluation
nix-env -f . -qaP -A expresso-kit

# Check meta
nix-instantiate --eval -A expresso-kit.meta

# Run package tests
nix-build -A expresso-kit.tests  # if tests are defined
```

#### 7. Commit and PR

```bash
git add pkgs/by-name/ex/expresso-kit
git commit -m "expresso-kit: init at 0.1.0"
git push origin expresso-kit-init
# Create PR on GitHub
```

### nixpkgs PR Checklist

Before submitting:

- [ ] Package follows [by-name structure](https://github.com/NixOS/nixpkgs/blob/master/pkgs/by-name/README.md)
- [ ] `meta.description` is under 80 chars, no period at end
- [ ] `meta.license` matches upstream LICENSE file
- [ ] `meta.homepage` is correct and accessible
- [ ] `meta.maintainers` is empty (add yourself after becoming a maintainer)
- [ ] `meta.mainProgram` is set correctly
- [ ] `meta.platforms` is appropriate (usually `lib.platforms.unix`)
- [ ] Package builds on all supported platforms
- [ ] Hash is SRI format (`sha256-...`)
- [ ] No unnecessary build flags
- [ ] Tests pass or are appropriately skipped
- [ ] Commit message follows format: `pkgname: init at version`

### Becoming a nixpkgs Maintainer

After your PR is merged:

1. Create PR to add yourself to `maintainers/maintainer-list.nix`
2. Format:
   ```nix
   amindWalker = {
     email = "bhrochamail@gmail.com";
     github = "amindWalker";
     githubId = YOUR_GITHUB_ID;
     name = "Breno Rocha";
   };
   ```
3. Update your package to add yourself to `meta.maintainers`

## File Structure

```
packaging/
├── README.md           # This file
├── build-arch.sh       # Arch Linux build script
├── build-deb.sh        # Debian/Ubuntu local build script
├── build-nix.sh        # Nix build and PR preparation script
├── build-ppa.sh        # Ubuntu PPA (Launchpad) build script
├── run-nix-ci.sh       # Run nixpkgs CI checks locally with act
├── arch/
│   ├── PKGBUILD        # Release package build
│   ├── PKGBUILD-git    # Git/development build
│   └── .SRCINFO        # AUR metadata
├── debian/
│   ├── changelog       # Version history
│   ├── compat          # Debhelper compatibility
│   ├── control         # Package metadata
│   ├── copyright       # License info
│   ├── rules           # Build rules (Makefile)
│   └── expresso-kit.docs  # Documentation files
└── nix/
    ├── package.nix     # nixpkgs-compatible package definition
    └── flake.nix       # Development flake (uses crane)

# Root level files:
.actrc                  # act configuration for local CI testing
.github/workflows/
└── nix-ci.yml          # nixpkgs-compatible CI workflow
flake.nix               # Main project flake
default.nix             # Legacy nix-build support
shell.nix               # Legacy nix-shell support
```
