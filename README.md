# Pakto 📦

[![Crates.io](https://img.shields.io/crates/v/pakto.svg)](https://crates.io/crates/pakto)
[![Documentation](https://docs.rs/pakto/badge.svg)](https://docs.rs/pakto)
[![Build Status](https://github.com/yourusername/pakto/workflows/CI/badge.svg)](https://github.com/yourusername/pakto/actions)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/License-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE-MIT)

**Convert NPM packages to OutSystems-compatible JavaScript bundles**

Pakto automates the process of converting NPM packages into single-file JavaScript bundles that work seamlessly with the OutSystems platform. It handles module system conversion, provides browser polyfills for Node.js APIs, and generates optimized, minified output.

## ✨ Features

- 🔄 **Module System Conversion**: Converts CommonJS, ESM, and UMD modules to OutSystems-compatible IIFE format
- 🌐 **Browser Polyfills**: Automatic polyfills for Node.js APIs (crypto, Buffer, events, etc.)
- 📦 **Smart Bundling**: Intelligent dependency bundling with tree-shaking support
- 🎯 **OutSystems Optimized**: Generates code specifically optimized for OutSystems platform
- ⚡ **Fast & Efficient**: Built in Rust for maximum performance
- 🔍 **Compatibility Analysis**: Analyze package compatibility before conversion
- 📊 **Detailed Reporting**: Comprehensive conversion reports with warnings and statistics
- 🛠️ **Configurable**: Flexible configuration system for different use cases

## 🚀 Quick Start

### Installation

```bash
# Install from crates.io
cargo install pakto

# Or install from source
git clone https://github.com/yourusername/pakto
cd pakto
cargo install --path .
```

### Basic Usage

```bash
# Convert a package
pakto convert lodash

# Convert with custom options
pakto convert jsotp \
  --output ./my-libs/jsotp-outsystems.js \
  --name "JSOTP" \
  --namespace "MyLibs" \
  --minify \
  --target es5

# Analyze compatibility first
pakto analyze some-package

# Generate configuration file
pakto init
```

## 📋 Examples

### Converting the `jsotp` package

```bash
pakto convert jsotp \
  --output ./jsotp-outsystems.js \
  --name "JSOTP" \
  --include-polyfills crypto,buffer \
  --minify
```

This generates a single file that can be directly used in OutSystems:

```javascript
// Generated jsotp-outsystems.js
(function(global) {
  'use strict';
  
  // Polyfills for Node.js APIs
  var crypto = window.crypto || cryptoPolyfill;
  var Buffer = BufferPolyfill;
  
  // Your converted package code
  var JSOTP = {
    TOTP: function(secret) { /* ... */ },
    HOTP: function(secret) { /* ... */ }
  };
  
  // Expose to global scope for OutSystems
  global.JSOTP = JSOTP;
  
})(window);
```

### Batch conversion

```bash
# Create a packages.txt file
echo "lodash" > packages.txt
echo "moment" >> packages.txt
echo "uuid" >> packages.txt

# Convert all packages
pakto batch-convert packages.txt --output-dir ./outsystems-libs/
```

## ⚙️ Configuration

Pakto supports configuration via `pakto.toml` files:

```toml
[npm]
registry = "https://registry.npmjs.org"
timeout = 30

[output]
directory = "./dist"
naming_pattern = "{name}-outsystems.js"
minify = true
target = "es5"

[polyfills]
default_includes = ["buffer", "crypto", "events"]
default_excludes = ["fs", "child_process"]

[bundle]
strategy = "inline"
max_size = 5242880  # 5MB
exclude_dependencies = ["fsevents"]
```

## 🔧 Advanced Usage

### Custom Polyfills

```bash
# Use custom polyfill directory
pakto convert crypto-js --polyfill-dir ./my-polyfills/

# Exclude problematic polyfills
pakto convert some-package --exclude-polyfills fs,child_process
```

### Bundle Strategies

```bash
# Inline all dependencies (default)
pakto convert package --strategy inline

# Tree-shake unused code
pakto convert package --strategy selective

# Assume external CDN libraries
pakto convert package --strategy external

# Hybrid approach
pakto convert package --strategy hybrid
```

### Target Environments

```bash
# Target modern browsers
pakto convert package --target es2020

# Target older browsers (default)
pakto convert package --target es5

# Latest features
pakto convert package --target esnext
```

## 📊 Compatibility Analysis

Before converting, analyze package compatibility:

```bash
pakto analyze some-package
```

Output:
```json
{
  "package_info": {
    "name": "some-package",
    "version": "1.0.0"
  },
  "compatibility_issues": [
    {
      "level": "Warning",
      "message": "Uses Node.js 'fs' module",
      "suggestion": "File system operations not supported in browser"
    }
  ],
  "required_polyfills": ["buffer", "crypto"],
  "feasible": true,
  "compatibility_score": 0.85
}
```

## 🛠️ Development

### Prerequisites

- Rust 1.70+
- Node.js (for testing against real NPM packages)

### Building

```bash
git clone https://github.com/yourusername/pakto
cd pakto
cargo build --release
```

### Testing

```bash
# Run unit tests
cargo test

# Test with real packages
cargo test --test integration

# Test CLI commands
cargo test --test cli_tests
```

### Contributing

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

## 📝 License

This project is licensed under either of

- Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

## 🤝 Acknowledgments

- [SWC](https://swc.rs/) for JavaScript/TypeScript parsing and transformation
- [OutSystems](https://www.outsystems.com/) for inspiring this tool
- The Rust community for excellent crates and documentation

## 🔗 Related Projects

- [Browserify](http://browserify.org/) - Bundle npm modules for the browser
- [Webpack](https://webpack.js.org/) - Module bundler
- [Rollup](https://rollupjs.org/) - Module bundler for ES modules
- [Parcel](https://parceljs.org/) - Zero-config build tool

---

**Pakto** - Making NPM packages work seamlessly with OutSystems! 🚀