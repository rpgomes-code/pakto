//! Basic usage example for Pakto
//!
//! This example demonstrates how to convert a simple NPM package
//! to OutSystems-compatible JavaScript.

use pakto::{Config, Converter, ConvertOptions};
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::init();

    println!("Pakto Basic Usage Example");
    println!("========================");

    // Load default configuration
    let config = Config::default();
    println!("✓ Configuration loaded");

    // Create converter
    let converter = Converter::new(config).await?;
    println!("✓ Converter initialized");

    // Define conversion options
    let options = ConvertOptions {
        output_path: Some(PathBuf::from("./examples/output/is-array-outsystems.js")),
        name: Some("IsArray".to_string()),
        namespace: Some("Utils".to_string()),
        minify: false,
        target_es_version: pakto::cli::EsTarget::Es5,
        include_polyfills: vec![],
        exclude_dependencies: vec![],
        bundle_strategy: pakto::cli::BundleStrategy::Inline,
    };

    // Convert a simple package
    let package_name = "is-array"; // Small, simple utility package
    println!("📦 Converting package: {}", package_name);

    match converter.convert(package_name, options).await {
        Ok(result) => {
            println!("✅ Conversion successful!");
            println!("📁 Output file: {}", result.output_path.display());
            println!("📊 File size: {} bytes", result.size);
            println!("⚡ Conversion time: {}ms", result.stats.conversion_time_ms);
            println!("🎯 Compatibility score: {:.2}", result.stats.compatibility_score);

            if !result.warnings.is_empty() {
                println!("⚠️  Warnings:");
                for warning in &result.warnings {
                    println!("  - {}", warning);
                }
            }

            if !result.polyfills_used.is_empty() {
                println!("🔧 Polyfills used: {:?}", result.polyfills_used);
            }
        }
        Err(e) => {
            println!("❌ Conversion failed: {}", e);

            // Try analyzing the package instead
            println!("🔍 Analyzing package compatibility...");
            match converter.analyze(package_name).await {
                Ok(analysis) => {
                    println!("📋 Analysis Results:");
                    println!("  Package: {} v{}", analysis.package_info.name, analysis.package_info.version);
                    println!("  Feasible: {}", analysis.feasible);
                    println!("  Compatibility score: {:.2}", analysis.compatibility_score);
                    println!("  Issues found: {}", analysis.compatibility_issues.len());

                    for issue in analysis.compatibility_issues.iter().take(5) {
                        println!("    - {:?}: {}", issue.level, issue.message);
                    }
                }
                Err(analysis_err) => {
                    println!("❌ Analysis also failed: {}", analysis_err);
                }
            }
        }
    }

    Ok(())
}
