#!/bin/bash

# Test script for real package conversions
# Use this to test against actual NPM packages

set -e

echo "🧪 Testing Pakto with Real NPM Packages"
echo "======================================="

# List of test packages (small, simple ones for testing)
TEST_PACKAGES=(
    "is-array"
    "is-string"
    "is-number"
    "camelcase"
    "kebab-case"
)

# Create test output directory
mkdir -p ./test-output

for package in "${TEST_PACKAGES[@]}"; do
    echo ""
    echo "📦 Testing package: $package"

    # Analyze first
    echo "  🔍 Analyzing..."
    cargo run -- analyze "$package" > "./test-output/${package}-analysis.json" 2>&1 || {
        echo "  ❌ Analysis failed"
        continue
    }

    # Try to convert
    echo "  🔧 Converting..."
    cargo run -- convert "$package" \
        --output "./test-output/${package}-outsystems.js" \
        --name "$(echo $package | sed 's/-//g' | sed 's/.*/\u&/')" \
        --minify 2>&1 || {
        echo "  ❌ Conversion failed (expected during development)"
        continue
    }

    echo "  ✅ Success!"
done

echo ""
echo "📊 Test Results Summary:"
ls -la ./test-output/

echo ""
echo "🎯 Check ./test-output/ for generated files and analysis reports"