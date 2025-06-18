//! Pakto - Convert NPM packages to OutSystems-compatible JavaScript bundles
//!
//! This crate provides functionality to automatically convert NPM packages
//! into single-file JavaScript bundles that work with the OutSystems platform.
//!
//! # Features
//!
//! - Module system conversion (CommonJS, ESM, UMD → IIFE)
//! - Browser polyfills for Node.js APIs
//! - Smart dependency bundling
//! - Compatibility analysis
//! - TypeScript support
//!
//! # Example
//!
//! ```rust,no_run
//! use pakto::{Config, Converter, ConvertOptions};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let config = Config::default();
//!     let converter = Converter::new(config).await?;
//!
//!     let options = ConvertOptions {
//!         output_path: Some("output.js".into()),
//!         minify: true,
//!         ..Default::default()
//!     };
//!
//!     let result = converter.convert("lodash", options).await?;
//!     println!("Converted! Output: {}", result.output_path.display());
//!
//!     Ok(())
//! }
//! ```

pub mod config;
pub mod cli;
pub mod converter;
pub mod analyzer;
pub mod transformer;
pub mod bundler;
pub mod npm;
pub mod output;
pub mod polyfills;
pub mod errors;

// Re-export main types for convenience
pub use config::Config;
pub use converter::{Converter, ConvertOptions, ConvertResult, AnalysisResult};
pub use errors::{PaktoError, Result};

/// Current version of Pakto
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Check if this version of Pakto is compatible with a given feature
pub fn is_feature_supported(feature: &str) -> bool {
    match feature {
        "es2015" | "es5" | "commonjs" | "esmodules" => true,
        "typescript" | "jsx" => true,
        "polyfills" => true,
        "minification" => true,
        "source-maps" => false, // Not yet implemented
        "web-workers" => false, // Not yet implemented
        _ => false,
    }
}

/// Get information about supported polyfills
pub fn supported_polyfills() -> Vec<&'static str> {
    vec![
        "crypto",
        "buffer",
        "events",
        "process",
        "util",
        "path",
    ]
}

/// Get information about supported module formats
pub fn supported_input_formats() -> Vec<&'static str> {
    vec![
        "CommonJS",
        "ES Modules",
        "UMD",
        "IIFE",
    ]
}

/// Get information about supported output targets
pub fn supported_output_targets() -> Vec<&'static str> {
    vec![
        "ES5",
        "ES2015",
        "ES2017",
        "ES2018",
        "ES2020",
        "ESNext",
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_constant() {
        assert!(!VERSION.is_empty());
        assert!(VERSION.chars().next().unwrap().is_ascii_digit());
    }

    #[test]
    fn test_feature_support() {
        assert!(is_feature_supported("es5"));
        assert!(is_feature_supported("typescript"));
        assert!(!is_feature_supported("nonexistent-feature"));
    }

    #[test]
    fn test_supported_lists() {
        assert!(!supported_polyfills().is_empty());
        assert!(!supported_input_formats().is_empty());
        assert!(!supported_output_targets().is_empty());

        assert!(supported_polyfills().contains(&"crypto"));
        assert!(supported_input_formats().contains(&"CommonJS"));
        assert!(supported_output_targets().contains(&"ES5"));
    }
}