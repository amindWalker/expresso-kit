#!/usr/bin/env bash

# Prepare Commit - All-in-One Pre-Commit Preparation
# =============================================================================
# Runs all quality checks and auto-fixes what it can:
#   1. Format code (auto-fix)
#   2. Run clippy with auto-fix
#   3. Run tests
#   4. Final compile check
#
# Usage: ./scripts/prepare-commit [--check] [--no-test]
#   --check   : Only check, don't auto-fix
#   --no-test : Skip tests for faster iteration

set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
BOLD='\033[1m'
NC='\033[0m'

# Parse arguments
CHECK_ONLY=false
SKIP_TESTS=false
for arg in "$@"; do
    case $arg in
        --check) CHECK_ONLY=true ;;
        --no-test) SKIP_TESTS=true ;;
    esac
done

echo -e "${BLUE}${BOLD}"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "  ☕ Expresso-Kit: Prepare Commit"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo -e "${NC}"

FAILED=0
START_TIME=$(date +%s)

# Step 1: Format
echo -e "${YELLOW}[1/4]${NC} ${CYAN}Formatting code...${NC}"
if $CHECK_ONLY; then
    if cargo +nightly fmt --all -- --check; then
        echo -e "${GREEN}  ✓ Formatting OK${NC}"
    else
        echo -e "${RED}  ✗ Formatting issues found (run without --check to fix)${NC}"
        FAILED=1
    fi
else
    cargo +nightly fmt --all
    echo -e "${GREEN}  ✓ Formatted${NC}"
fi

# Step 2: Clippy (with auto-fix if not check-only)
echo -e "\n${YELLOW}[2/4]${NC} ${CYAN}Running clippy...${NC}"
if $CHECK_ONLY; then
    if cargo clippy --all-features --workspace --tests -- -D warnings 2>/dev/null; then
        echo -e "${GREEN}  ✓ Clippy OK${NC}"
    else
        echo -e "${RED}  ✗ Clippy found issues${NC}"
        FAILED=1
    fi
else
    # Try to auto-fix, then check remaining issues
    cargo clippy --all-features --workspace --tests --fix --allow-dirty --allow-staged 2>/dev/null || true
    if cargo clippy --all-features --workspace --tests -- -D warnings 2>/dev/null; then
        echo -e "${GREEN}  ✓ Clippy OK (auto-fixed where possible)${NC}"
    else
        echo -e "${RED}  ✗ Clippy found issues that need manual fix${NC}"
        FAILED=1
    fi
fi

# Step 3: Tests
if $SKIP_TESTS; then
    echo -e "\n${YELLOW}[3/4]${NC} ${CYAN}Tests skipped (--no-test)${NC}"
else
    echo -e "\n${YELLOW}[3/4]${NC} ${CYAN}Running tests...${NC}"
    if cargo test --workspace --all-features 2>/dev/null; then
        echo -e "${GREEN}  ✓ Tests passed${NC}"
    else
        echo -e "${RED}  ✗ Tests failed${NC}"
        FAILED=1
    fi
fi

# Step 4: Final check
echo -e "\n${YELLOW}[4/4]${NC} ${CYAN}Final compile check...${NC}"
if cargo check --workspace --all-features 2>/dev/null; then
    echo -e "${GREEN}  ✓ Compile OK${NC}"
else
    echo -e "${RED}  ✗ Compile errors${NC}"
    FAILED=1
fi

# Summary
END_TIME=$(date +%s)
DURATION=$((END_TIME - START_TIME))

echo -e "\n${BLUE}${BOLD}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
if [ $FAILED -eq 0 ]; then
    echo -e "${GREEN}${BOLD}  ✓ Ready to commit! ${NC}${GREEN}(${DURATION}s)${NC}"
    echo -e "${CYAN}  Run: git add -A && git commit -m \"message\"${NC}"
else
    echo -e "${RED}${BOLD}  ✗ Issues found - fix before committing${NC}"
fi
echo -e "${BLUE}${BOLD}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"

exit $FAILED
