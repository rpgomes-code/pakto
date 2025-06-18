use std::path::{Path, PathBuf};
use std::collections::HashMap;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tracing::{info, warn, debug};
use uuid::Uuid;

use crate::config::Config;
use crate::cli::{BundleStrategy, EsTarget};
use crate::errors::{PaktoError, CompatibilityIssue, Warning};
use crate::npm::NpmClient;
use crate::analyzer::PackageAnalyzer;
use crate::transformer::CodeTransformer;
use crate::bundler::Bundler;
use crate::output::OutputGenerator;

/// Main converter that orchestrates the conversion process
pub struct Converter {
    config: Config,
    npm_client: NpmClient,
    analyzer: PackageAnalyzer,
    transformer: CodeTransformer,
    bundler: Bundler,
    output_generator: OutputGenerator,
}

/// Options for package conversion
#[derive(Debug, Clone)]
pub struct ConvertOptions {
    pub output_path: Option<PathBuf>,
    pub name: Option<String>,
    pub namespace: Option<String>,
    pub minify: bool,
    pub target_es_version: EsTarget,
    pub include_polyfills: Vec<String>,
    pub exclude_dependencies: Vec<String>,
    pub bundle_strategy: BundleStrategy,
}

/// Result of package conversion
#[derive(Debug, Serialize)]
pub struct ConvertResult {
    /// Path to the generated file
    pub output_path: PathBuf,

    /// Size of the generated file in bytes
    pub size: usize,

    /// Warnings generated during conversion
    pub warnings: Vec<String>,

    /// Polyfills that were included
    pub polyfills_used: Vec<String>,

    /// Dependencies that were bundled
    pub dependencies_bundled: Vec<String>,

    /// Conversion statistics
    pub stats: ConversionStats,

    /// Unique conversion ID for tracking
    pub conversion_id: String,
}

/// Detailed conversion statistics
#[derive(Debug, Serialize)]
pub struct ConversionStats {
    /// Original package size (before conversion)
    pub original_size: usize,

    /// Number of files processed
    pub files_processed: usize,

    /// Number of dependencies resolved
    pub dependencies_resolved: usize,

    /// Compression ratio (if minified)
    pub compression_ratio: Option<f32>,

    /// Conversion time in milliseconds
    pub conversion_time_ms: u64,

    /// Compatibility score (0.0 - 1.0)
    pub compatibility_score: f32,
}

/// Package analysis result
#[derive(Debug, Serialize)]
pub struct AnalysisResult {
    /// Package information
    pub package_info: PackageInfo,

    /// Compatibility issues found
    pub compatibility_issues: Vec<CompatibilityIssue>,

    /// Required polyfills
    pub required_polyfills: Vec<String>,

    /// Dependency tree analysis
    pub dependency_analysis: DependencyAnalysis,

    /// Estimated bundle size
    pub estimated_size: EstimatedSize,

    /// Overall compatibility score
    pub compatibility_score: f32,

    /// Conversion feasibility
    pub feasible: bool,
}

#[derive(Debug, Serialize)]
pub struct PackageInfo {
    pub name: String,
    pub version: String,
    pub description: Option<String>,
    pub main: Option<String>,
    pub entry_points: Vec<String>,
    pub dependencies: HashMap<String, String>,
    pub dev_dependencies: HashMap<String, String>,
    pub keywords: Vec<String>,
    pub license: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct DependencyAnalysis {
    pub total_dependencies: usize,
    pub problematic_dependencies: Vec<String>,
    pub browser_compatible: Vec<String>,
    pub needs_polyfills: Vec<String>,
    pub circular_dependencies: Vec<Vec<String>>,
}

#[derive(Debug, Serialize)]
pub struct EstimatedSize {
    pub min_size: usize,
    pub max_size: usize,
    pub with_polyfills: usize,
    pub minified: usize,
}

impl Converter {
    /// Create a new converter instance
    pub async fn new(config: Config) -> Result<Self> {
        let npm_client = NpmClient::new(&config.npm).await?;
        let analyzer = PackageAnalyzer::new(&config);
        let transformer = CodeTransformer::new(&config);
        let bundler = Bundler::new(&config);
        let output_generator = OutputGenerator::new(&config);

        Ok(Self {
            config,
            npm_client,
            analyzer,
            transformer,
            bundler,
            output_generator,
        })
    }

    /// Convert an NPM package to OutSystems-compatible JavaScript
    pub async fn convert(
        &self,
        package: &str,
        options: ConvertOptions
    ) -> Result<ConvertResult> {
        let start_time = std::time::Instant::now();
        let conversion_id = Uuid::new_v4().to_string();

        info!("Starting conversion of package: {}", package);
        debug!("Conversion ID: {}", conversion_id);

        // Step 1: Analyze package
        info!("Analyzing package compatibility...");
        let analysis = self.analyze(package).await?;

        if !analysis.feasible {
            return Err(PaktoError::IncompatibleApi {
                api: "Multiple incompatible APIs".to_string(),
                suggestion: Some("This package is not suitable for OutSystems conversion".to_string()),
                location: None,
            });
        }

        // Step 2: Download package
        info!("Downloading package and dependencies...");
        let package_data = self.npm_client.download_package(package).await?;

        // Step 3: Transform code
        info!("Transforming code for browser compatibility...");
        let transformed = self.transformer.transform_package(
            &package_data,
            &options,
            &analysis,
        ).await?;

        // Step 4: Bundle dependencies
        info!("Bundling dependencies...");
        let bundled = self.bundler.bundle(
            &transformed,
            &options.bundle_strategy,
            &options.exclude_dependencies,
        ).await?;

        // Step 5: Generate output
        info!("Generating output file...");
        let output_path = self.determine_output_path(package, &options)?;
        let final_code = self.output_generator.generate(
            &bundled,
            &options,
            &analysis.package_info,
        )?;

        // Step 6: Write file
        std::fs::write(&output_path, &final_code)
            .with_context(|| format!("Failed to write output file: {}", output_path.display()))?;

        let conversion_time = start_time.elapsed();
        let file_size = final_code.len();

        // Collect warnings
        let mut warnings = Vec::new();
        for issue in &analysis.compatibility_issues {
            if matches!(issue.level, crate::errors::IssueLevel::Warning) {
                warnings.push(issue.message.clone());
            }
        }

        let result = ConvertResult {
            output_path,
            size: file_size,
            warnings,
            polyfills_used: analysis.required_polyfills,
            dependencies_bundled: bundled.bundled_dependencies,
            stats: ConversionStats {
                original_size: package_data.total_size,
                files_processed: transformed.files_processed,
                dependencies_resolved: analysis.dependency_analysis.total_dependencies,
                compression_ratio: if options.minify {
                    Some(file_size as f32 / bundled.unminified_size as f32)
                } else {
                    None
                },
                conversion_time_ms: conversion_time.as_millis() as u64,
                compatibility_score: analysis.compatibility_score,
            },
            conversion_id,
        };

        info!(
            "Conversion completed successfully in {}ms", 
            conversion_time.as_millis()
        );
        info!("Output file: {} ({} bytes)", result.output_path.display(), result.size);

        Ok(result)
    }

    /// Analyze package compatibility without converting
    pub async fn analyze(&self, package: &str) -> Result<AnalysisResult> {
        info!("Analyzing package: {}", package);

        // Get package metadata
        let package_info = self.npm_client.get_package_info(package).await?;

        // Download for analysis (might use cache)
        let package_data = self.npm_client.download_package(package).await?;

        // Analyze compatibility
        let analysis = self.analyzer.analyze(&package_data).await?;

        Ok(analysis)
    }

    /// Determine output path based on options and configuration
    fn determine_output_path(&self, package: &str, options: &ConvertOptions) -> Result<PathBuf> {
        if let Some(ref path) = options.output_path {
            return Ok(path.clone());
        }

        let name = options.name
            .as_deref()
            .unwrap_or(package);

        let filename = self.config.output.naming_pattern
            .replace("{name}", name)
            .replace("{package}", package);

        Ok(self.config.output.directory.join(filename))
    }
}

impl Default for ConvertOptions {
    fn default() -> Self {
        Self {
            output_path: None,
            name: None,
            namespace: None,
            minify: false,
            target_es_version: EsTarget::Es5,
            include_polyfills: Vec::new(),
            exclude_dependencies: Vec::new(),
            bundle_strategy: BundleStrategy::Inline,
        }
    }
}

// Placeholder structs that will be implemented in other modules
pub struct PackageData {
    pub total_size: usize,
    pub files: HashMap<PathBuf, String>,
    pub package_json: serde_json::Value,
}

pub struct TransformedPackage {
    pub files_processed: usize,
    pub code: String,
    pub source_map: Option<String>,
}

pub struct BundledCode {
    pub code: String,
    pub bundled_dependencies: Vec<String>,
    pub unminified_size: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_converter_creation() {
        let config = Config::default();
        let result = Converter::new(config).await;
        assert!(result.is_ok());
    }

    #[test]
    fn test_convert_options_default() {
        let options = ConvertOptions::default();
        assert_eq!(options.target_es_version, EsTarget::Es5);
        assert_eq!(options.bundle_strategy, BundleStrategy::Inline);
        assert!(!options.minify);
    }

    #[test]
    fn test_output_path_determination() {
        // This would require a full converter instance, 
        // so we'll test the logic separately
        let naming_pattern = "{name}-outsystems.js";
        let result = naming_pattern
            .replace("{name}", "test-package")
            .replace("{package}", "test-package");

        assert_eq!(result, "test-package-outsystems.js");
    }
}