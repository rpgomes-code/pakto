#!/bin/bash

# Pakto CLI Demo Script
# This script demonstrates various Pakto CLI commands

set -e

echo "🚀 Pakto CLI Demo"
echo "================"

# Create output directory
mkdir -p ./examples/output

echo ""
echo "📋 1. Analyzing package compatibility..."
cargo run --example basic_usage 2>/dev/null || echo "Example not yet functional (expected during development)"

echo ""
echo "🔧 2. Initialize Pakto configuration..."
cargo run -- init --output-dir ./examples/

echo ""
echo "📦 3. Convert simple packages (when implementation is complete)..."
echo "   Commands that will work:"
echo "   cargo run -- convert is-array --output ./examples/output/is-array.js"
echo "   cargo run -- convert lodash --name Lodash --minify"
echo "   cargo run -- analyze some-package"

echo ""
echo "🎯 4. Advanced usage examples:"
echo "   # Convert with custom options"
echo "   cargo run -- convert jsotp \\"
echo "     --output ./examples/output/jsotp-outsystems.js \\"
echo "     --name JSOTP \\"
echo "     --namespace MyLibs \\"
echo "     --include-polyfills crypto,buffer \\"
echo "     --target es5 \\"
echo "     --minify"

echo ""
echo "   # Analyze before converting"
echo "   cargo run -- analyze crypto-js"

echo ""
echo "   # Batch convert multiple packages"
echo "   echo 'lodash' > packages.txt"
echo "   echo 'moment' >> packages.txt"
echo "   echo 'uuid' >> packages.txt"
echo "   # cargo run -- batch-convert packages.txt"

echo ""
echo "✅ Demo complete! Check ./examples/output/ for generated files."