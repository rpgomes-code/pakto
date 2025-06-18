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
    dependency_graph: DependencyGraph,
    module_resolver: ModuleResolver,
}

/// Represents the dependency graph of a package
#[derive(Debug, Default)]
struct DependencyGraph {
    nodes: HashMap<String, DependencyNode>,
    edges: HashMap<String, Vec<String>>,
}

/// Node in the dependency graph
#[derive(Debug, Clone)]
struct DependencyNode {
    name: String,
    version: String,
    path: PathBuf,
    code: String,
    size: usize,
    is_external: bool,
    dependencies: Vec<String>,
}

/// Resolves module paths and handles different module systems
struct ModuleResolver {
    base_path: PathBuf,
    extensions: Vec<String>,
    alias_map: HashMap<String, String>,
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

/// Result of dependency analysis
#[derive(Debug)]
struct DependencyAnalysisResult {
    total_dependencies: usize,
    bundled_dependencies: Vec<String>,
    external_dependencies: Vec<String>,
    circular_dependencies: Vec<Vec<String>>,
    unused_dependencies: Vec<String>,
    estimated_size: usize,
}

impl Bundler {
    pub fn new(config: &Config) -> Self {
        Self {
            config: config.clone(),
            dependency_graph: DependencyGraph::default(),
            module_resolver: ModuleResolver::new(),
        }
    }

    pub async fn bundle(
        &self,
        transformed: &TransformedPackage,
        strategy: &BundleStrategy,
        exclude_dependencies: &[String],
    ) -> PaktoResult<BundledCode> {
        info!("Starting dependency bundling with strategy: {:?}", strategy);

        // Parse and analyze dependencies
        let dependencies = self.analyze_dependencies(&transformed.code).await?;
        debug!("Found {} dependencies", dependencies.total_dependencies);

        // Apply bundling strategy
        let bundle_options = self.create_bundle_options(strategy, exclude_dependencies)?;

        let bundled_code = match strategy {
            BundleStrategy::Inline => {
                self.bundle_inline(&transformed.code, &dependencies, &bundle_options).await?
            }
            BundleStrategy::Selective => {
                self.bundle_selective(&transformed.code, &dependencies, &bundle_options).await?
            }
            BundleStrategy::External => {
                self.bundle_external(&transformed.code, &dependencies, &bundle_options).await?
            }
            BundleStrategy::Hybrid => {
                self.bundle_hybrid(&transformed.code, &dependencies, &bundle_options).await?
            }
        };

        // Optimize the bundled code
        let optimized_code = self.optimize_bundle(&bundled_code, &bundle_options)?;

        Ok(BundledCode {
            code: optimized_code,
            bundled_dependencies: dependencies.bundled_dependencies,
            unminified_size: bundled_code.len(),
        })
    }

    /// Analyze dependencies in the transformed code
    async fn analyze_dependencies(&self, code: &str) -> PaktoResult<DependencyAnalysisResult> {
        let mut dependencies = HashSet::new();
        let mut bundled = Vec::new();
        let mut external = Vec::new();

        // Extract require() calls
        let require_regex = Regex::new(r#"require\s*\(\s*['"`]([^'"`]+)['"`]\s*\)"#)?;
        for cap in require_regex.captures_iter(code) {
            let dep_name = &cap[1];
            dependencies.insert(dep_name.to_string());

            if self.should_bundle_dependency(dep_name) {
                bundled.push(dep_name.to_string());
            } else {
                external.push(dep_name.to_string());
            }
        }

        // Extract import statements
        let import_regex = Regex::new(r#"(?:import|from)\s+['"`]([^'"`]+)['"`]"#)?;
        for cap in import_regex.captures_iter(code) {
            let dep_name = &cap[1];
            dependencies.insert(dep_name.to_string());

            if self.should_bundle_dependency(dep_name) {
                bundled.push(dep_name.to_string());
            } else {
                external.push(dep_name.to_string());
            }
        }

        // Detect circular dependencies
        let circular = self.detect_circular_dependencies(&bundled).await?;

        // Estimate bundle size
        let estimated_size = self.estimate_bundle_size(&bundled).await?;

        Ok(DependencyAnalysisResult {
            total_dependencies: dependencies.len(),
            bundled_dependencies: bundled,
            external_dependencies: external,
            circular_dependencies: circular,
            unused_dependencies: Vec::new(), // TODO: Implement unused detection
            estimated_size,
        })
    }

    /// Bundle all dependencies inline
    async fn bundle_inline(
        &self,
        main_code: &str,
        dependencies: &DependencyAnalysisResult,
        options: &BundleOptions,
    ) -> PaktoResult<String> {
        debug!("Bundling {} dependencies inline", dependencies.bundled_dependencies.len());

        let mut bundled_code = String::new();
        let mut processed_modules = HashSet::new();

        // Add bundled dependencies
        for dep_name in &dependencies.bundled_dependencies {
            if !processed_modules.contains(dep_name) {
                if let Ok(dep_code) = self.resolve_and_load_dependency(dep_name).await {
                    bundled_code.push_str(&format!(
                        "\n  // === Dependency: {} ===\n",
                        dep_name
                    ));
                    bundled_code.push_str(&self.wrap_dependency_code(dep_name, &dep_code)?);
                    bundled_code.push_str("\n");
                    processed_modules.insert(dep_name.clone());
                }
            }
        }

        // Add main code
        bundled_code.push_str("\n  // === Main Module ===\n");
        bundled_code.push_str(main_code);

        Ok(bundled_code)
    }

    /// Bundle only used dependencies (tree-shaking)
    async fn bundle_selective(
        &self,
        main_code: &str,
        dependencies: &DependencyAnalysisResult,
        options: &BundleOptions,
    ) -> PaktoResult<String> {
        debug!("Bundling dependencies selectively");

        // Analyze what's actually used
        let used_exports = self.analyze_used_exports(main_code, &dependencies.bundled_dependencies)?;

        let mut bundled_code = String::new();

        for (dep_name, exports) in used_exports {
            if let Ok(dep_code) = self.resolve_and_load_dependency(&dep_name).await {
                let tree_shaken = self.tree_shake_module(&dep_code, &exports)?;
                bundled_code.push_str(&format!(
                    "\n  // === Dependency: {} (tree-shaken) ===\n",
                    dep_name
                ));
                bundled_code.push_str(&self.wrap_dependency_code(&dep_name, &tree_shaken)?);
                bundled_code.push_str("\n");
            }
        }

        bundled_code.push_str("\n  // === Main Module ===\n");
        bundled_code.push_str(main_code);

        Ok(bundled_code)
    }

    /// Bundle with external dependencies assumed available
    async fn bundle_external(
        &self,
        main_code: &str,
        dependencies: &DependencyAnalysisResult,
        _options: &BundleOptions,
    ) -> PaktoResult<String> {
        debug!("Bundling with external dependencies");

        let mut bundled_code = String::new();

        // Add external dependency checks
        if !dependencies.external_dependencies.is_empty() {
            bundled_code.push_str("\n  // === External Dependencies ===\n");
            for dep_name in &dependencies.external_dependencies {
                bundled_code.push_str(&format!(
                    "  if (typeof {} === 'undefined') {{\n",
                    self.dependency_to_global_name(dep_name)
                ));
                bundled_code.push_str(&format!(
                    "    throw new Error('External dependency not found: {}');\n",
                    dep_name
                ));
                bundled_code.push_str("  }\n");
            }
            bundled_code.push_str("\n");
        }

        bundled_code.push_str("  // === Main Module ===\n");
        bundled_code.push_str(main_code);

        Ok(bundled_code)
    }

    /// Hybrid bundling strategy
    async fn bundle_hybrid(
        &self,
        main_code: &str,
        dependencies: &DependencyAnalysisResult,
        options: &BundleOptions,
    ) -> PaktoResult<String> {
        debug!("Using hybrid bundling strategy");

        let mut bundled_code = String::new();
        let mut inlined_deps = Vec::new();
        let mut external_deps = Vec::new();

        // Categorize dependencies
        for dep_name in &dependencies.bundled_dependencies {
            if self.should_inline_dependency(dep_name, options).await? {
                inlined_deps.push(dep_name);
            } else {
                external_deps.push(dep_name);
            }
        }

        // Bundle inline dependencies
        for dep_name in inlined_deps {
            if let Ok(dep_code) = self.resolve_and_load_dependency(dep_name).await {
                bundled_code.push_str(&format!(
                    "\n  // === Inlined: {} ===\n",
                    dep_name
                ));
                bundled_code.push_str(&self.wrap_dependency_code(dep_name, &dep_code)?);
                bundled_code.push_str("\n");
            }
        }

        // Add external dependency checks
        if !external_deps.is_empty() {
            bundled_code.push_str("\n  // === External Dependencies ===\n");
            for dep_name in external_deps {
                bundled_code.push_str(&format!(
                    "  var {} = global.{} || require('{}');\n",
                    self.dependency_to_variable_name(dep_name),
                    self.dependency_to_global_name(dep_name),
                    dep_name
                ));
            }
            bundled_code.push_str("\n");
        }

        bundled_code.push_str("  // === Main Module ===\n");
        bundled_code.push_str(main_code);

        Ok(bundled_code)
    }

    /// Optimize the bundled code
    fn optimize_bundle(&self, code: &str, options: &BundleOptions) -> PaktoResult<String> {
        let mut optimized = code.to_string();

        if options.deduplicate {
            optimized = self.deduplicate_code(&optimized)?;
        }

        // Remove unnecessary whitespace and comments
        optimized = self.clean_bundle(&optimized)?;

        // Validate the bundled code
        self.validate_bundle(&optimized)?;

        Ok(optimized)
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
            max_inline_size: self.config.bundle.max_size / 10, // 10% of max size per module
            exclude_patterns,
        })
    }

    /// Check if dependency should be bundled
    fn should_bundle_dependency(&self, dep_name: &str) -> bool {
        // Don't bundle Node.js built-ins
        if matches!(dep_name, "fs" | "path" | "crypto" | "http" | "https" | "os" | "child_process") {
            return false;
        }

        // Don't bundle if in exclude list
        if self.config.bundle.exclude_dependencies.contains(&dep_name.to_string()) {
            return false;
        }

        // Bundle if in force inline list
        if self.config.bundle.force_inline.contains(&dep_name.to_string()) {
            return true;
        }

        // Default: bundle relative imports, not node_modules
        dep_name.starts_with('.') || dep_name.starts_with('/')
    }

    /// Resolve and load dependency code
    async fn resolve_and_load_dependency(&self, dep_name: &str) -> Result<String> {
        // For now, return a placeholder
        // In a real implementation, this would:
        // 1. Resolve the module path
        // 2. Load the file from disk or cache
        // 3. Transform if necessary

        Ok(format!(
            "// Placeholder for dependency: {}\nvar {} = {{}};",
            dep_name,
            self.dependency_to_variable_name(dep_name)
        ))
    }

    /// Wrap dependency code in a module wrapper
    fn wrap_dependency_code(&self, dep_name: &str, code: &str) -> PaktoResult<String> {
        let var_name = self.dependency_to_variable_name(dep_name);

        Ok(format!(
            "  var {} = (function() {{\n    var module = {{ exports: {{}} }};\n    var exports = module.exports;\n    \n{}\n    \n    return module.exports;\n  }})();",
            var_name,
            code.lines()
                .map(|line| format!("    {}", line))
                .collect::<Vec<_>>()
                .join("\n")
        ))
    }

    /// Convert dependency name to valid variable name
    fn dependency_to_variable_name(&self, dep_name: &str) -> String {
        dep_name
            .replace(['/', '-', '.', '@'], "_")
            .replace(|c: char| !c.is_alphanumeric() && c != '_', "")
    }

    /// Convert dependency name to global name
    fn dependency_to_global_name(&self, dep_name: &str) -> String {
        // Convert lodash -> _, moment -> moment, etc.
        match dep_name {
            "lodash" => "_".to_string(),
            "jquery" => "$".to_string(),
            _ => dep_name.replace(['/', '-'], "_"),
        }
    }

    /// Analyze which exports are actually used
    fn analyze_used_exports(
        &self,
        code: &str,
        dependencies: &[String],
    ) -> PaktoResult<HashMap<String, Vec<String>>> {
        let mut used_exports = HashMap::new();

        for dep_name in dependencies {
            let var_name = self.dependency_to_variable_name(dep_name);

            // Find usage patterns like: dep.method, dep['property'], etc.
            let usage_regex = Regex::new(&format!(r"{}\.(\w+)", regex::escape(&var_name)))?;
            let mut exports = Vec::new();

            for cap in usage_regex.captures_iter(code) {
                exports.push(cap[1].to_string());
            }

            if !exports.is_empty() {
                exports.sort();
                exports.dedup();
                used_exports.insert(dep_name.clone(), exports);
            }
        }

        Ok(used_exports)
    }

    /// Tree-shake unused exports from a module
    fn tree_shake_module(&self, code: &str, used_exports: &[String]) -> PaktoResult<String> {
        // Simple tree-shaking: keep only used exports
        // In a real implementation, this would use AST analysis

        if used_exports.is_empty() {
            return Ok("// Tree-shaken: no exports used\n".to_string());
        }

        // For now, just add a comment about what's being kept
        Ok(format!(
            "// Tree-shaken module (keeping: {})\n{}",
            used_exports.join(", "),
            code
        ))
    }

    /// Detect circular dependencies
    async fn detect_circular_dependencies(&self, dependencies: &[String]) -> PaktoResult<Vec<Vec<String>>> {
        // Simple placeholder implementation
        // Real implementation would build a dependency graph and detect cycles
        Ok(Vec::new())
    }

    /// Estimate bundle size
    async fn estimate_bundle_size(&self, dependencies: &[String]) -> PaktoResult<usize> {
        // Rough estimation: 5KB per dependency
        Ok(dependencies.len() * 5120)
    }

    /// Check if dependency should be inlined in hybrid mode
    async fn should_inline_dependency(&self, dep_name: &str, options: &BundleOptions) -> PaktoResult<bool> {
        // Check against exclude patterns
        for pattern in &options.exclude_patterns {
            if pattern.is_match(dep_name) {
                return Ok(false);
            }
        }

        // Inline small dependencies
        if options.inline_small_modules {
            // Estimate size (placeholder)
            let estimated_size = dep_name.len() * 100; // Very rough estimate
            return Ok(estimated_size < options.max_inline_size);
        }

        Ok(true)
    }

    /// Remove duplicate code sections
    fn deduplicate_code(&self, code: &str) -> PaktoResult<String> {
        // Simple deduplication: remove duplicate function definitions
        // Real implementation would use more sophisticated analysis

        let lines: Vec<&str> = code.lines().collect();
        let mut seen_functions = HashSet::new();
        let mut deduplicated_lines = Vec::new();

        for line in lines {
            if line.trim().starts_with("function ") {
                let func_signature = line.trim().split('{').next().unwrap_or(line);
                if !seen_functions.contains(func_signature) {
                    seen_functions.insert(func_signature.to_string());
                    deduplicated_lines.push(line);
                }
            } else {
                deduplicated_lines.push(line);
            }
        }

        Ok(deduplicated_lines.join("\n"))
    }

    /// Clean up bundle (remove extra whitespace, etc.)
    fn clean_bundle(&self, code: &str) -> PaktoResult<String> {
        let mut cleaned = code.to_string();

        // Remove empty lines
        let re = Regex::new(r"\n\s*\n\s*\n")?;
        cleaned = re.replace_all(&cleaned, "\n\n").to_string();

        // Remove trailing whitespace
        let lines: Vec<String> = cleaned
            .lines()
            .map(|line| line.trim_end().to_string())
            .collect();

        Ok(lines.join("\n"))
    }

    /// Validate the bundled code
    fn validate_bundle(&self, code: &str) -> PaktoResult<()> {
        // Basic validation checks

        // Check for balanced braces
        let open_braces = code.matches('{').count();
        let close_braces = code.matches('}').count();
        if open_braces != close_braces {
            return Err(PaktoError::BundleTooLarge {
                size: 0,
                max: 0,
            }); // Placeholder error
        }

        // Check for balanced parentheses
        let open_parens = code.matches('(').count();
        let close_parens = code.matches(')').count();
        if open_parens != close_parens {
            return Err(PaktoError::BundleTooLarge {
                size: 0,
                max: 0,
            }); // Placeholder error
        }

        // Check bundle size
        if code.len() > self.config.bundle.max_size {
            return Err(PaktoError::BundleTooLarge {
                size: code.len(),
                max: self.config.bundle.max_size,
            });
        }

        Ok(())
    }
}

impl ModuleResolver {
    fn new() -> Self {
        Self {
            base_path: PathBuf::from("."),
            extensions: vec![
                ".js".to_string(),
                ".ts".to_string(),
                ".jsx".to_string(),
                ".tsx".to_string(),
                ".mjs".to_string(),
                ".cjs".to_string(),
            ],
            alias_map: HashMap::new(),
        }
    }

    /// Resolve a module path to an absolute path
    fn resolve_module(&self, module_path: &str) -> PaktoResult<PathBuf> {
        // Check aliases first
        if let Some(aliased) = self.alias_map.get(module_path) {
            return self.resolve_module(aliased);
        }

        // Handle relative paths
        if module_path.starts_with('.') {
            return Ok(self.base_path.join(module_path));
        }

        // Handle node_modules
        let node_modules_path = self.base_path.join("node_modules").join(module_path);
        if node_modules_path.exists() {
            return Ok(node_modules_path);
        }

        // Fallback to module name as-is
        Ok(PathBuf::from(module_path))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    #[test]
    fn test_bundler_creation() {
        let config = Config::default();
        let bundler = Bundler::new(&config);
        assert!(!bundler.config.bundle.exclude_dependencies.is_empty());
    }

    #[test]
    fn test_dependency_name_conversion() {
        let config = Config::default();
        let bundler = Bundler::new(&config);

        assert_eq!(bundler.dependency_to_variable_name("lodash"), "lodash");
        assert_eq!(bundler.dependency_to_variable_name("@types/node"), "_types_node");
        assert_eq!(bundler.dependency_to_variable_name("./relative"), "__relative");
    }

    #[test]
    fn test_should_bundle_dependency() {
        let config = Config::default();
        let bundler = Bundler::new(&config);

        assert!(!bundler.should_bundle_dependency("fs"));
        assert!(!bundler.should_bundle_dependency("crypto"));
        assert!(bundler.should_bundle_dependency("./local-module"));
        assert!(bundler.should_bundle_dependency("lodash"));
    }

    #[tokio::test]
    async fn test_bundle_options_creation() {
        let config = Config::default();
        let bundler = Bundler::new(&config);

        let options = bundler.create_bundle_options(
            &BundleStrategy::Selective,
            &["test-*".to_string()]
        ).unwrap();

        assert!(options.tree_shake);
        assert!(options.deduplicate);
        assert_eq!(options.exclude_patterns.len(), 1);
    }
}