use std::collections::HashMap;
use std::path::{Path, PathBuf};
use anyhow::{Context, Result};
use tracing::{debug, info, warn};
use regex::Regex;

use crate::config::Config;
use crate::converter::{PackageData, TransformedPackage, ConvertOptions, AnalysisResult};
use crate::cli::EsTarget;
use crate::errors::{PaktoError, Result as PaktoResult};
use crate::polyfills::PolyfillRegistry;

/// Simplified transformer for initial development
/// This version uses regex-based transformations instead of full AST parsing
pub struct CodeTransformer {
    config: Config,
    polyfills: PolyfillRegistry,
}

/// Module transformation result
#[derive(Debug)]
struct ModuleTransformResult {
    code: String,
    polyfills_used: Vec<String>,
    source_map: Option<String>,
}

impl CodeTransformer {
    pub fn new(config: &Config) -> Self {
        Self {
            config: config.clone(),
            polyfills: PolyfillRegistry::new(),
        }
    }

    pub async fn transform_package(
        &self,
        package_data: &PackageData,
        options: &ConvertOptions,
        analysis: &AnalysisResult,
    ) -> PaktoResult<TransformedPackage> {
        info!("Starting code transformation");

        let mut transformed_files = HashMap::new();
        let mut files_processed = 0;
        let mut all_polyfills = Vec::new();

        // Transform each file
        for (path, content) in &package_data.files {
            if self.should_transform_file(path) {
                debug!("Transforming file: {}", path.display());

                match self.transform_file(path, content, options, analysis).await {
                    Ok(result) => {
                        transformed_files.insert(path.clone(), result.code);
                        all_polyfills.extend(result.polyfills_used);
                        files_processed += 1;
                    }
                    Err(e) => {
                        warn!("Failed to transform file {}: {}", path.display(), e);
                        // Include original file as fallback
                        transformed_files.insert(path.clone(), content.clone());
                        files_processed += 1;
                    }
                }
            }
        }

        // Bundle all files into a single module
        let bundled_code = self.bundle_files(transformed_files, options, analysis)?;

        // Inject required polyfills
        let final_code = self.inject_polyfills(&bundled_code, &all_polyfills, options)?;

        Ok(TransformedPackage {
            files_processed,
            code: final_code,
            source_map: None,
        })
    }

    /// Transform a single file using regex-based approach
    async fn transform_file(
        &self,
        path: &Path,
        content: &str,
        _options: &ConvertOptions,
        _analysis: &AnalysisResult,
    ) -> Result<ModuleTransformResult> {
        let mut transformed_code = content.to_string();
        let mut polyfills_used = Vec::new();

        // Transform require() calls for Node.js APIs
        let require_regex = Regex::new(r#"require\s*\(\s*['"`]([^'"`]+)['"`]\s*\)"#)?;
        transformed_code = require_regex.replace_all(&transformed_code, |caps: &regex::Captures| {
            let module_name = &caps[1];
            match module_name {
                "crypto" => {
                    polyfills_used.push("crypto".to_string());
                    "cryptoPolyfill".to_string()
                }
                "buffer" => {
                    polyfills_used.push("buffer".to_string());
                    "BufferPolyfill".to_string()
                }
                "events" => {
                    polyfills_used.push("events".to_string());
                    "EventEmitterPolyfill".to_string()
                }
                "process" => {
                    polyfills_used.push("process".to_string());
                    "processPolyfill".to_string()
                }
                _ => caps[0].to_string(),
            }
        }).to_string();

        // Transform ES6 import statements
        let import_regex = Regex::new(r#"import\s+.*?\s+from\s+['"`]([^'"`]+)['"`]"#)?;
        transformed_code = import_regex.replace_all(&transformed_code, |caps: &regex::Captures| {
            let module_name = &caps[1];
            match module_name {
                "crypto" => {
                    polyfills_used.push("crypto".to_string());
                    caps[0].replace(module_name, "cryptoPolyfill")
                }
                "buffer" => {
                    polyfills_used.push("buffer".to_string());
                    caps[0].replace(module_name, "BufferPolyfill")
                }
                "events" => {
                    polyfills_used.push("events".to_string());
                    caps[0].replace(module_name, "EventEmitterPolyfill")
                }
                "process" => {
                    polyfills_used.push("process".to_string());
                    caps[0].replace(module_name, "processPolyfill")
                }
                _ => caps[0].to_string(),
            }
        }).to_string();

        // Transform process.env access
        let process_env_regex = Regex::new(r"\bprocess\.env\b")?;
        if process_env_regex.is_match(&transformed_code) {
            polyfills_used.push("process".to_string());
            transformed_code = process_env_regex.replace_all(&transformed_code, "processPolyfill.env").to_string();
        }

        // Basic CommonJS to browser module transformation
        transformed_code = self.transform_commonjs_to_browser(&transformed_code)?;

        // Remove or comment out incompatible Node.js API usage
        transformed_code = self.handle_incompatible_apis(&transformed_code)?;

        polyfills_used.sort();
        polyfills_used.dedup();

        Ok(ModuleTransformResult {
            code: transformed_code,
            polyfills_used,
            source_map: None,
        })
    }

    /// Transform CommonJS modules to browser-compatible format
    fn transform_commonjs_to_browser(&self, code: &str) -> Result<String> {
        let mut transformed = code.to_string();

        // Wrap in IIFE if it looks like a CommonJS module
        if code.contains("module.exports") || code.contains("exports.") {
            transformed = format!(
                "(function(module, exports) {{\n{}\nreturn module.exports;\n}})({{exports: {{}}}}, {{}});",
                transformed
            );
        }

        // Transform module.exports = ... to return ...
        let module_exports_regex = Regex::new(r"module\.exports\s*=\s*")?;
        if module_exports_regex.is_match(&transformed) {
            // For simple cases, just replace with return
            // More complex cases would need proper AST parsing
            transformed = module_exports_regex.replace(&transformed, "return ").to_string();
        }

        Ok(transformed)
    }

    /// Handle incompatible Node.js APIs
    fn handle_incompatible_apis(&self, code: &str) -> Result<String> {
        let mut transformed = code.to_string();

        // Comment out file system operations
        let fs_regex = Regex::new(r".*require\s*\(\s*['\"']fs['\"']\s*\).*")?;
        transformed = fs_regex.replace_all(&transformed, "// $0 // File system not available in browser").to_string();

        // Comment out child_process operations
        let child_process_regex = Regex::new(r".*require\s*\(\s*['\"']child_process['\"']\s*\).*")?;
        transformed = child_process_regex.replace_all(&transformed, "// $0 // Child processes not available in browser").to_string();

        // Comment out os operations
        let os_regex = Regex::new(r".*require\s*\(\s*['\"']os['\"']\s*\).*")?;
        transformed = os_regex.replace_all(&transformed, "// $0 // OS module not available in browser").to_string();

        Ok(transformed)
    }

    /// Bundle multiple files into a single module
    fn bundle_files(
        &self,
        files: HashMap<PathBuf, String>,
        options: &ConvertOptions,
        analysis: &AnalysisResult,
    ) -> PaktoResult<String> {
        debug!("Bundling {} files", files.len());

        let mut bundled_code = String::new();

        // Generate module header
        bundled_code.push_str(&self.generate_module_header(options, analysis)?);

        // Add all file contents
        for (path, content) in files {
            bundled_code.push_str(&format!(
                "\n  // === {} ===\n",
                path.display()
            ));

            // Indent the content to match the IIFE structure
            for line in content.lines() {
                bundled_code.push_str("  ");
                bundled_code.push_str(line);
                bundled_code.push('\n');
            }
            bundled_code.push('\n');
        }

        // Generate module footer
        bundled_code.push_str(&self.generate_module_footer(options, analysis)?);

        Ok(bundled_code)
    }

    /// Generate module header
    fn generate_module_header(
        &self,
        options: &ConvertOptions,
        analysis: &AnalysisResult,
    ) -> PaktoResult<String> {
        let mut header = String::new();

        // Add header comment
        header.push_str(&format!(
            "/**\n * {} - OutSystems Compatible Bundle\n",
            analysis.package_info.name
        ));
        if let Some(ref description) = analysis.package_info.description {
            header.push_str(&format!(" * {}\n", description));
        }
        header.push_str(&format!(" * Version: {}\n", analysis.package_info.version));
        header.push_str(" * Generated by Pakto\n");
        header.push_str(" */\n");

        // Start UMD wrapper
        header.push_str("(function(global, factory) {\n");
        header.push_str("  'use strict';\n\n");

        // UMD pattern
        header.push_str("  if (typeof module === 'object' && typeof module.exports === 'object') {\n");
        header.push_str("    module.exports = factory();\n");
        header.push_str("  } else if (typeof define === 'function' && define.amd) {\n");
        header.push_str("    define(factory);\n");
        header.push_str("  } else {\n");

        let global_name = options.name.as_deref()
            .unwrap_or(&analysis.package_info.name)
            .replace(['-', '@', '/'], "_");

        if let Some(ref namespace) = options.namespace {
            header.push_str(&format!("    global.{} = global.{} || {{}};\n", namespace, namespace));
            header.push_str(&format!("    global.{}.{} = factory();\n", namespace, global_name));
        } else {
            header.push_str(&format!("    global.{} = factory();\n", global_name));
        }

        header.push_str("  }\n");
        header.push_str("})(typeof window !== 'undefined' ? window : this, function() {\n");
        header.push_str("  'use strict';\n\n");

        Ok(header)
    }

    /// Generate module footer
    fn generate_module_footer(
        &self,
        _options: &ConvertOptions,
        _analysis: &AnalysisResult,
    ) -> PaktoResult<String> {
        let mut footer = String::new();

        footer.push_str("\n  // Return the module\n");
        footer.push_str("  return typeof module !== 'undefined' && module.exports ? module.exports : {};\n");
        footer.push_str("});\n");

        Ok(footer)
    }

    /// Inject polyfills into the code
    fn inject_polyfills(
        &self,
        code: &str,
        polyfills_needed: &[String],
        _options: &ConvertOptions,
    ) -> PaktoResult<String> {
        if polyfills_needed.is_empty() {
            return Ok(code.to_string());
        }

        debug!("Injecting polyfills: {:?}", polyfills_needed);

        let mut polyfilled_code = String::new();

        // Find insertion point (after factory function starts)
        let lines: Vec<&str> = code.lines().collect();
        let mut injection_point = 0;

        for (i, line) in lines.iter().enumerate() {
            if line.contains("'use strict';") && line.contains("  ") {
                injection_point = i + 1;
                break;
            }
        }

        // Add lines up to injection point
        for (i, line) in lines.iter().enumerate() {
            polyfilled_code.push_str(line);
            polyfilled_code.push('\n');

            if i == injection_point {
                polyfilled_code.push_str("\n  // === Polyfills ===\n");

                for polyfill_name in polyfills_needed {
                    if let Some(polyfill_code) = self.polyfills.get_polyfill(polyfill_name) {
                        polyfilled_code.push_str(&format!("  // Polyfill: {}\n", polyfill_name));

                        // Indent polyfill code
                        for polyfill_line in polyfill_code.lines() {
                            if !polyfill_line.trim().is_empty() {
                                polyfilled_code.push_str("  ");
                                polyfilled_code.push_str(polyfill_line);
                            }
                            polyfilled_code.push('\n');
                        }
                        polyfilled_code.push('\n');
                    }
                }

                polyfilled_code.push_str("  // === End Polyfills ===\n\n");
            }
        }

        Ok(polyfilled_code)
    }

    /// Check if file should be transformed
    fn should_transform_file(&self, path: &Path) -> bool {
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            matches!(ext.to_lowercase().as_str(), "js" | "ts" | "jsx" | "tsx" | "mjs" | "cjs")
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    #[test]
    fn test_transformer_creation() {
        let config = Config::default();
        let transformer = CodeTransformer::new(&config);
        assert!(transformer.polyfills.get_polyfill("crypto").is_some());
    }

    #[test]
    fn test_should_transform_file() {
        let config = Config::default();
        let transformer = CodeTransformer::new(&config);

        assert!(transformer.should_transform_file(Path::new("test.js")));
        assert!(transformer.should_transform_file(Path::new("test.ts")));
        assert!(!transformer.should_transform_file(Path::new("test.md")));
        assert!(!transformer.should_transform_file(Path::new("test.json")));
    }

    #[test]
    fn test_commonjs_transformation() {
        let config = Config::default();
        let transformer = CodeTransformer::new(&config);

        let input = r#"
const crypto = require('crypto');
module.exports = { hash: crypto.createHash };
"#;

        let result = transformer.transform_commonjs_to_browser(input).unwrap();
        assert!(result.contains("(function(module, exports)"));
    }

    #[test]
    fn test_polyfill_detection() {
        let config = Config::default();
        let transformer = CodeTransformer::new(&config);

        let input = r#"
const crypto = require('crypto');
const buffer = require('buffer');
"#;

        // This is a simplified test - in reality we'd need to run the full transform
        assert!(input.contains("crypto"));
        assert!(input.contains("buffer"));
    }
}