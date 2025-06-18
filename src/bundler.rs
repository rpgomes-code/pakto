use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use anyhow::{Context, Result};
use tracing::{debug, info, warn};
use regex::Regex;

use crate::config::Config;
use crate::converter::{TransformedPackage, BundledCode};
use crate::cli::BundleStrategy;
use crate::errors::{PaktoError, Result as PaktoResult};

/// Handles dependency bundling and module resolution
pub struct Bundler {
    config: Config,
}

/// Bundle optimization options
#[derive(Debug, Clone)]
struct BundleOptions {
    tree_shake: bool,
    deduplicate: bool,
    inline_small_modules: bool,
    max_inline_size: usize,
    exclude_patterns: Vec<Regex>,
}

impl Bundler {
    pub fn new(config: &Config) -> Self {
        Self {
            config: config.clone(),
        }
    }

    pub async fn bundle(
        &self,
        transformed: &TransformedPackage,
        strategy: &BundleStrategy,
        exclude_dependencies: &[String],
    ) -> PaktoResult<BundledCode> {
        info!("Starting dependency bundling with strategy: {:?}", strategy);

        // Create bundle options based on strategy
        let bundle_options = self.create_bundle_options(strategy, exclude_dependencies)?;

        // Process the code based on strategy
        let (processed_code, bundled_deps) = match strategy {
            BundleStrategy::Inline => {
                self.bundle_inline(&transformed.code, &bundle_options).await?
            }
            BundleStrategy::Selective => {
                self.bundle_selective(&transformed.code, &bundle_options).await?
            }
            BundleStrategy::External => {
                self.bundle_external(&transformed.code, &bundle_options).await?
            }
            BundleStrategy::Hybrid => {
                self.bundle_hybrid(&transformed.code, &bundle_options).await?
            }
        };

        // Optimize the bundle
        let optimized_code = self.optimize_bundle(&processed_code, &bundle_options)?;

        // Validate the final bundle
        self.validate_bundle(&optimized_code)?;

        Ok(BundledCode {
            code: optimized_code,
            bundled_dependencies: bundled_deps,
            unminified_size: processed_code.len(),
        })
    }

    /// Create bundle options based on strategy
    fn create_bundle_options(
        &self,
        strategy: &BundleStrategy,
        exclude_dependencies: &[String],
    ) -> PaktoResult<BundleOptions> {
        let exclude_patterns = exclude_dependencies
            .iter()
            .map(|pattern| Regex::new(pattern))
            .collect::<Result<Vec<_>, _>>()?;

        Ok(BundleOptions {
            tree_shake: matches!(strategy, BundleStrategy::Selective | BundleStrategy::Hybrid),
            deduplicate: true,
            inline_small_modules: matches!(strategy, BundleStrategy::Inline | BundleStrategy::Hybrid),
            max_inline_size: self.config.bundle.max_size / 10,
            exclude_patterns,
        })
    }

    /// Bundle all dependencies inline
    async fn bundle_inline(
        &self,
        code: &str,
        options: &BundleOptions,
    ) -> PaktoResult<(String, Vec<String>)> {
        debug!("Bundling with inline strategy");

        let dependencies = self.extract_dependencies(code)?;
        let mut bundled_code = String::new();
        let mut bundled_deps = Vec::new();

        // Add comment header
        bundled_code.push_str("// === Inline Bundle ===\n");
        bundled_code.push_str("// All dependencies are included in this bundle\n\n");

        // Add dependency stubs for now (in a real implementation, we'd fetch actual dependencies)
        for dep in &dependencies {
            if !self.should_exclude_dependency(dep, options) {
                bundled_code.push_str(&format!("// Dependency: {}\n", dep));
                bundled_code.push_str(&self.create_dependency_stub(dep)?);
                bundled_code.push_str("\n\n");
                bundled_deps.push(dep.clone());
            }
        }

        // Add the main code
        bundled_code.push_str("// === Main Code ===\n");
        bundled_code.push_str(code);

        Ok((bundled_code, bundled_deps))
    }

    /// Bundle with selective inclusion (tree-shaking)
    async fn bundle_selective(
        &self,
        code: &str,
        options: &BundleOptions,
    ) -> PaktoResult<(String, Vec<String>)> {
        debug!("Bundling with selective strategy");

        let dependencies = self.extract_dependencies(code)?;
        let used_exports = self.analyze_used_exports(code, &dependencies)?;

        let mut bundled_code = String::new();
        let mut bundled_deps = Vec::new();

        bundled_code.push_str("// === Selective Bundle (Tree-shaken) ===\n");
        bundled_code.push_str("// Only used exports are included\n\n");

        for (dep, exports) in used_exports {
            if !self.should_exclude_dependency(&dep, options) {
                bundled_code.push_str(&format!("// Dependency: {} (exports: {:?})\n", dep, exports));
                bundled_code.push_str(&self.create_selective_dependency_stub(&dep, &exports)?);
                bundled_code.push_str("\n\n");
                bundled_deps.push(dep);
            }
        }

        bundled_code.push_str("// === Main Code ===\n");
        bundled_code.push_str(code);

        Ok((bundled_code, bundled_deps))
    }

    /// Bundle with external dependencies
    async fn bundle_external(
        &self,
        code: &str,
        options: &BundleOptions,
    ) -> PaktoResult<(String, Vec<String>)> {
        debug!("Bundling with external strategy");

        let dependencies = self.extract_dependencies(code)?;
        let mut bundled_code = String::new();
        let external_deps: Vec<String> = dependencies.into_iter()
            .filter(|dep| !self.should_exclude_dependency(dep, options))
            .collect();

        bundled_code.push_str("// === External Bundle ===\n");
        bundled_code.push_str("// Dependencies are expected to be available externally\n\n");

        if !external_deps.is_empty() {
            bundled_code.push_str("// External dependency checks\n");
            for dep in &external_deps {
                let global_name = self.dependency_to_global_name(dep);
                bundled_code.push_str(&format!(
                    "if (typeof {} === 'undefined') {{\n",
                    global_name
                ));
                bundled_code.push_str(&format!(
                    "  throw new Error('External dependency not found: {} (expected global: {})');\n",
                    dep, global_name
                ));
                bundled_code.push_str("}\n");
            }
            bundled_code.push_str("\n");
        }

        bundled_code.push_str("// === Main Code ===\n");
        bundled_code.push_str(code);

        Ok((bundled_code, external_deps))
    }

    /// Bundle with hybrid strategy
    async fn bundle_hybrid(
        &self,
        code: &str,
        options: &BundleOptions,
    ) -> PaktoResult<(String, Vec<String>)> {
        debug!("Bundling with hybrid strategy");

        let dependencies = self.extract_dependencies(code)?;
        let mut bundled_code = String::new();
        let mut bundled_deps = Vec::new();

        bundled_code.push_str("// === Hybrid Bundle ===\n");
        bundled_code.push_str("// Mix of inline and external dependencies\n\n");

        let mut inline_deps = Vec::new();
        let mut external_deps = Vec::new();

        // Categorize dependencies
        for dep in dependencies {
            if self.should_exclude_dependency(&dep, options) {
                continue;
            }

            if self.should_inline_dependency(&dep) {
                inline_deps.push(dep);
            } else {
                external_deps.push(dep);
            }
        }

        // Add inline dependencies
        if !inline_deps.is_empty() {
            bundled_code.push_str("// === Inline Dependencies ===\n");
            for dep in &inline_deps {
                bundled_code.push_str(&format!("// Dependency: {}\n", dep));
                bundled_code.push_str(&self.create_dependency_stub(dep)?);
                bundled_code.push_str("\n");
                bundled_deps.push(dep.clone());
            }
            bundled_code.push_str("\n");
        }

        // Add external dependency checks
        if !external_deps.is_empty() {
            bundled_code.push_str("// === External Dependencies ===\n");
            for dep in &external_deps {
                let global_name = self.dependency_to_global_name(dep);
                bundled_code.push_str(&format!(
                    "var {} = (typeof {} !== 'undefined') ? {} : require('{}');\n",
                    self.dependency_to_variable_name(dep),
                    global_name,
                    global_name,
                    dep
                ));
                bundled_deps.push(dep.clone());
            }
            bundled_code.push_str("\n");
        }

        bundled_code.push_str("// === Main Code ===\n");
        bundled_code.push_str(code);

        Ok((bundled_code, bundled_deps))
    }

    /// Extract dependencies from code
    fn extract_dependencies(&self, code: &str) -> PaktoResult<Vec<String>> {
        let mut dependencies = HashSet::new();

        // Extract require() calls
        let require_regex = Regex::new(r#"require\s*\(\s*['"`]([^'"`]+)['"`]\s*\)"#)?;
        for cap in require_regex.captures_iter(code) {
            dependencies.insert(cap[1].to_string());
        }

        // Extract import statements
        let import_regex = Regex::new(r#"(?:import|from)\s+['"`]([^'"`]+)['"`]"#)?;
        for cap in import_regex.captures_iter(code) {
            dependencies.insert(cap[1].to_string());
        }

        Ok(dependencies.into_iter().collect())
    }

    /// Analyze which exports are actually used
    fn analyze_used_exports(
        &self,
        code: &str,
        dependencies: &[String],
    ) -> PaktoResult<HashMap<String, Vec<String>>> {
        let mut used_exports = HashMap::new();

        for dep in dependencies {
            let var_name = self.dependency_to_variable_name(dep);
            let mut exports = Vec::new();

            // Look for property access patterns
            let usage_regex = Regex::new(&format!(r"{}\.(\w+)", regex::escape(&var_name)))?;
            for cap in usage_regex.captures_iter(code) {
                exports.push(cap[1].to_string());
            }

            // Look for destructuring patterns
            let destructure_regex = Regex::new(&format!(
                r"(?:const|let|var)\s*\{{\s*([^}}]+)\s*\}}\s*=\s*{}",
                regex::escape(&var_name)
            ))?;
            if let Some(cap) = destructure_regex.captures(code) {
                let destructured = cap[1].split(',')
                    .map(|s| s.trim().to_string())
                    .collect::<Vec<_>>();
                exports.extend(destructured);
            }

            if !exports.is_empty() {
                exports.sort();
                exports.dedup();
                used_exports.insert(dep.clone(), exports);
            }
        }

        Ok(used_exports)
    }

    /// Create a stub for a dependency
    fn create_dependency_stub(&self, dep_name: &str) -> PaktoResult<String> {
        let var_name = self.dependency_to_variable_name(dep_name);

        // Create basic stubs for common dependencies
        let stub = match dep_name {
            "lodash" => format!(
                "var {} = {{\n  map: function(arr, fn) {{ return arr.map(fn); }},\n  filter: function(arr, fn) {{ return arr.filter(fn); }},\n  reduce: function(arr, fn, init) {{ return arr.reduce(fn, init); }}\n}};",
                var_name
            ),
            "uuid" => format!(
                "var {} = {{\n  v4: function() {{ return 'xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx'.replace(/[xy]/g, function(c) {{\n    var r = Math.random() * 16 | 0, v = c == 'x' ? r : (r & 0x3 | 0x8);\n    return v.toString(16);\n  }}); }}\n}};",
                var_name
            ),
            "moment" => format!(
                "var {} = function(date) {{\n  return {{\n    format: function(fmt) {{ return new Date(date).toISOString(); }},\n    valueOf: function() {{ return new Date(date).getTime(); }}\n  }};\n}};",
                var_name
            ),
            _ => format!(
                "var {} = {{\n  // Stub for dependency: {}\n  // Add actual implementation or use external library\n}};",
                var_name, dep_name
            ),
        };

        Ok(stub)
    }

    /// Create a selective stub with only used exports
    fn create_selective_dependency_stub(
        &self,
        dep_name: &str,
        exports: &[String],
    ) -> PaktoResult<String> {
        let var_name = self.dependency_to_variable_name(dep_name);

        let mut stub = format!("var {} = {{\n", var_name);

        for export in exports {
            stub.push_str(&format!(
                "  {}: function() {{ /* Implementation for {} */ }},\n",
                export, export
            ));
        }

        stub.push_str("};");

        Ok(stub)
    }

    /// Check if dependency should be excluded
    fn should_exclude_dependency(&self, dep_name: &str, options: &BundleOptions) -> bool {
        // Check against exclude patterns
        for pattern in &options.exclude_patterns {
            if pattern.is_match(dep_name) {
                return true;
            }
        }

        // Check against config exclusions
        if self.config.bundle.exclude_dependencies.contains(&dep_name.to_string()) {
            return true;
        }

        // Always exclude Node.js built-ins that can't be polyfilled
        matches!(dep_name, "fs" | "child_process" | "cluster" | "worker_threads" | "net" | "http" | "https")
    }

    /// Check if dependency should be inlined in hybrid mode
    fn should_inline_dependency(&self, dep_name: &str) -> bool {
        // Force inline if specified in config
        if self.config.bundle.force_inline.contains(&dep_name.to_string()) {
            return true;
        }

        // Inline small utility libraries
        matches!(dep_name, 
            "is-array" | "is-string" | "is-number" | "camelcase" | "kebab-case" |
            "uuid" | "lodash.get" | "lodash.set" | "lodash.pick"
        )
    }

    /// Convert dependency name to variable name
    fn dependency_to_variable_name(&self, dep_name: &str) -> String {
        dep_name
            .replace(['/', '-', '.', '@'], "_")
            .replace(|c: char| !c.is_alphanumeric() && c != '_', "")
    }

    /// Convert dependency name to global name
    fn dependency_to_global_name(&self, dep_name: &str) -> String {
        match dep_name {
            "lodash" => "_".to_string(),
            "jquery" => "$".to_string(),
            "moment" => "moment".to_string(),
            "uuid" => "uuid".to_string(),
            _ => dep_name.replace(['/', '-'], "_"),
        }
    }

    /// Optimize the bundled code
    fn optimize_bundle(&self, code: &str, options: &BundleOptions) -> PaktoResult<String> {
        let mut optimized = code.to_string();

        if options.deduplicate {
            optimized = self.deduplicate_code(&optimized)?;
        }

        // Remove unnecessary comments in production
        optimized = self.clean_comments(&optimized)?;

        // Remove extra whitespace
        optimized = self.clean_whitespace(&optimized);

        Ok(optimized)
    }

    /// Remove duplicate code sections
    fn deduplicate_code(&self, code: &str) -> PaktoResult<String> {
        // Simple deduplication of variable declarations
        let lines: Vec<&str> = code.lines().collect();
        let mut seen_vars = HashSet::new();
        let mut deduplicated_lines = Vec::new();

        for line in lines {
            let trimmed = line.trim();

            // Check for variable declarations
            if trimmed.starts_with("var ") {
                if let Some(var_name) = trimmed.split_whitespace().nth(1) {
                    let var_name = var_name.trim_end_matches(&['=', ';'][..]);
                    if seen_vars.contains(var_name) {
                        continue; // Skip duplicate variable declaration
                    }
                    seen_vars.insert(var_name.to_string());
                }
            }

            deduplicated_lines.push(line);
        }

        Ok(deduplicated_lines.join("\n"))
    }

    /// Clean up comments for production
    fn clean_comments(&self, code: &str) -> PaktoResult<String> {
        // Remove single-line comments but keep important ones
        let comment_regex = Regex::new(r"^\s*//(?!\s*===).*$")?;
        let lines: Vec<String> = code.lines()
            .map(|line| {
                if comment_regex.is_match(line) && !line.contains("@") {
                    String::new() // Remove non-essential comments
                } else {
                    line.to_string()
                }
            })
            .collect();

        Ok(lines.join("\n"))
    }

    /// Clean up whitespace
    fn clean_whitespace(&self, code: &str) -> String {
        // Remove trailing whitespace and excessive blank lines
        let lines: Vec<String> = code.lines()
            .map(|line| line.trim_end().to_string())
            .collect();

        let mut cleaned_lines = Vec::new();
        let mut prev_was_empty = false;

        for line in lines {
            let is_empty = line.trim().is_empty();

            if !is_empty || !prev_was_empty {
                cleaned_lines.push(line);
            }

            prev_was_empty = is_empty;
        }

        cleaned_lines.join("\n")
    }

    /// Validate the bundled code
    fn validate_bundle(&self, code: &str) -> PaktoResult<()> {
        // Check bundle size
        if code.len() > self.config.bundle.max_size {
            return Err(PaktoError::BundleTooLarge {
                size: code.len(),
                max: self.config.bundle.max_size,
            });
        }

        // Basic syntax validation
        let open_braces = code.matches('{').count();
        let close_braces = code.matches('}').count();
        if open_braces != close_braces {
            return Err(PaktoError::TransformError {
                message: format!("Unbalanced braces in bundle: {} open, {} close", open_braces, close_braces),
                source: None,
            });
        }

        let open_parens = code.matches('(').count();
        let close_parens = code.matches(')').count();
        if open_parens != close_parens {
            return Err(PaktoError::TransformError {
                message: format!("Unbalanced parentheses in bundle: {} open, {} close", open_parens, close_parens),
                source: None,
            });
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::converter::TransformedPackage;

    #[tokio::test]
    async fn test_bundler_creation() {
        let config = Config::default();
        let bundler = Bundler::new(&config);

        let transformed = TransformedPackage {
            files_processed: 1,
            code: "const x = 1;".to_string(),
            source_map: None,
        };

        let result = bundler.bundle(&transformed, &BundleStrategy::Inline, &[]).await;
        assert!(result.is_ok());
    }

    #[test]
    fn test_dependency_extraction() {
        let config = Config::default();
        let bundler = Bundler::new(&config);

        let code = r#"
            const lodash = require('lodash');
            import moment from 'moment';
            const { v4 } = require('uuid');
        "#;

        let deps = bundler.extract_dependencies(code).unwrap();
        assert!(deps.contains(&"lodash".to_string()));
        assert!(deps.contains(&"moment".to_string()));
        assert!(deps.contains(&"uuid".to_string()));
    }

    #[test]
    fn test_dependency_name_conversion() {
        let config = Config::default();
        let bundler = Bundler::new(&config);

        assert_eq!(bundler.dependency_to_variable_name("lodash"), "lodash");
        assert_eq!(bundler.dependency_to_variable_name("@types/node"), "_types_node");
        assert_eq!(bundler.dependency_to_variable_name("./local-file"), "__local_file");

        assert_eq!(bundler.dependency_to_global_name("lodash"), "_");
        assert_eq!(bundler.dependency_to_global_name("jquery"), "$");
        assert_eq!(bundler.dependency_to_global_name("moment"), "moment");
    }

    #[test]
    fn test_should_exclude_dependency() {
        let config = Config::default();
        let bundler = Bundler::new(&config);
        let options = BundleOptions {
            tree_shake: false,
            deduplicate: false,
            inline_small_modules: false,
            max_inline_size: 1000,
            exclude_patterns: vec![],
        };

        assert!(bundler.should_exclude_dependency("fs", &options));
        assert!(bundler.should_exclude_dependency("child_process", &options));
        assert!(!bundler.should_exclude_dependency("lodash", &options));
    }
}