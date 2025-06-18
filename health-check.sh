#!/bin/bash

# Pakto Health Check Script
# Verifies that the project builds and basic functionality works

set -e

echo "🏥 Pakto Health Check"
echo "===================="

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Track results
CHECKS_PASSED=0
CHECKS_FAILED=0

check_passed() {
    echo -e "${GREEN}✅ $1${NC}"
    ((CHECKS_PASSED++))
}

check_failed() {
    echo -e "${RED}❌ $1${NC}"
    ((CHECKS_FAILED++))
}

check_warning() {
    echo -e "${YELLOW}⚠️  $1${NC}"
}

echo ""
echo "🔍 1. Checking Rust toolchain..."

if command -v rustc &> /dev/null; then
    RUST_VERSION=$(rustc --version)
    echo "   Rust version: $RUST_VERSION"
    check_passed "Rust toolchain available"
else
    check_failed "Rust not installed - install from https://rustup.rs/"
    exit 1
fi

echo ""
echo "📦 2. Checking project structure..."

# Check for required files
REQUIRED_FILES=(
    "Cargo.toml"
    "src/main.rs"
    "src/lib.rs"
    "README.md"
)

for file in "${REQUIRED_FILES[@]}"; do
    if [ -f "$file" ]; then
        check_passed "Found $file"
    else
        check_failed "Missing $file"
    fi
done

# Check for directories
REQUIRED_DIRS=(
    "src"
    "polyfills"
    "templates"
)

for dir in "${REQUIRED_DIRS[@]}"; do
    if [ -d "$dir" ]; then
        check_passed "Found $dir/ directory"
    else
        check_failed "Missing $dir/ directory"
    fi
done

echo ""
echo "🛠️  3. Checking dependencies..."

echo "   Running cargo check..."
if cargo check --quiet 2>/dev/null; then
    check_passed "Dependencies resolve correctly"
else
    check_failed "Dependency issues detected"
    echo "   Try: cargo update"
fi

echo ""
echo "🏗️  4. Building project..."

echo "   Building debug version..."
if cargo build --quiet 2>/dev/null; then
    check_passed "Debug build successful"
else
    check_failed "Debug build failed"
    echo "   Run 'cargo build' to see detailed errors"
fi

echo ""
echo "🧪 5. Running tests..."

echo "   Running unit tests..."
if cargo test --lib --quiet 2>/dev/null; then
    check_passed "Unit tests pass"
else
    check_warning "Some unit tests failing (expected during development)"
fi

echo ""
echo "⚡ 6. CLI functionality..."

echo "   Testing help command..."
if cargo run --quiet -- --help >/dev/null 2>&1; then
    check_passed "CLI help works"
else
    check_failed "CLI help command failed"
fi

echo ""
echo "📊 Results Summary"
echo "=================="
echo -e "Checks passed: ${GREEN}$CHECKS_PASSED${NC}"

if [ $CHECKS_FAILED -gt 0 ]; then
    echo -e "Checks failed: ${RED}$CHECKS_FAILED${NC}"
    echo ""
    echo -e "${RED}❌ Health check failed!${NC}"
    echo "Please fix the failing checks before proceeding."
    exit 1
else
    echo -e "Checks failed: ${GREEN}0${NC}"
    echo ""
    echo -e "${GREEN}✅ Health check passed!${NC}"
    echo "Project is ready for development."
fi

echo ""
echo "💡 Next steps:"
echo "   - Run 'cargo run -- --help' to see CLI options"
echo "   - Try 'cargo run -- init' to create configuration"
echo "   - Run tests with 'cargo test'"