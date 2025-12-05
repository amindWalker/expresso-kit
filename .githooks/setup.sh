#!/usr/bin/env bash

# Setup Git Hooks for Expresso-Kit
# =============================================================================
# Run this script to configure git to use the project's hooks.
# This configuration is LOCAL to this repository only (not global).

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

echo "Setting up git hooks for expresso-kit..."

# Configure git to use the .githooks directory (LOCAL to this repo only)
# This writes to .git/config, NOT ~/.gitconfig
git config --local core.hooksPath .githooks

# Make hooks executable
chmod +x "$REPO_ROOT/.githooks/pre-commit"
chmod +x "$REPO_ROOT/.githooks/pre-push"

# Make scripts executable
if [ -d "$REPO_ROOT/scripts" ]; then
    chmod +x "$REPO_ROOT/scripts/"* 2>/dev/null || true
fi

echo ""
echo "✓ Git hooks installed successfully!"
echo ""
echo "Configuration is LOCAL to this repository only."
echo "Verify with: git config --local --get core.hooksPath"
echo ""
echo "Hooks enabled:"
echo "  • pre-commit: Format check + compile check"
echo "  • pre-push:   Format + lint + tests + build"
echo ""
echo "Manual preparation (recommended before committing):"
echo "  ./scripts/prepare-commit           # Auto-fix + test"
echo "  ./scripts/prepare-commit --check   # Check only"
echo "  ./scripts/prepare-commit --no-test # Skip tests"
echo ""
echo "Shell alias (add to ~/.bashrc or ~/.zshrc or ~/.config/fish/config.fish):"
echo "  # Bash/Zsh:"
echo "  alias cargo-prepare='./scripts/prepare-commit'"
echo "  # Fish:"
echo "  alias cargo-prepare './scripts/prepare-commit'"
echo ""
echo "To bypass hooks temporarily (not recommended):"
echo "  git commit --no-verify"
echo "  git push --no-verify"
echo ""
echo "To remove hooks:"
echo "  git config --local --unset core.hooksPath"
