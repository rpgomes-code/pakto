use std::collections::{HashMap, HashSet};
use std::path::Path;
use anyhow::{Context, Result};
use regex::Regex;
use tracing::{debug, warn, info};
use swc_ecma_parser::{lexer::Lexer, Parser, StringInput, Syntax, TsConfig, EsConfig};
use swc_ecma_ast::*;
use swc_ecma_visit::{Visit, VisitWith};

use crate::config::Config;
use crate::converter::{
    PackageData, AnalysisResult, PackageInfo, DependencyAnalysis,
    EstimatedSize, CompatibilityIssue, IssueLevel
};
use crate::errors::{PaktoError, Result as PaktoResult, CodeLocation};

/// Analyzes packages for OutSystems compatibility
pub struct PackageAnalyzer {
    config: Config,
    node_apis: NodeApiRegistry,
}

/// Registry of Node.js APIs and their browser compatibility
struct NodeApiRegistry {
    incompatible_apis: HashSet<String>,
    polyfillable_apis: HashMap<String, String>,
    replaceable_apis: HashMap<String, String>,
}

/// Visitor for analyzing JavaScript/TypeScript AST
struct CompatibilityVisitor {
    issues: Vec<CompatibilityIssue>,
    required_polyfills: HashSet<String>,
    imports: Vec<String>,
    exports: Vec<String>,
    current_file: String,
}

/// Analysis of a single file
#[derive(Debug)]
struct FileAnalysis {
    path: String,
    syntax_type: SyntaxType,
    module_type: ModuleType,
    imports: Vec<ImportInfo>,
    exports: Vec<ExportInfo>,
    node_api_usage: Vec<NodeApiUsage>,
    issues: Vec<CompatibilityIssue>,
    estimated_size: usize,
}

#[derive(Debug, PartialEq)]
enum SyntaxType {
    JavaScript,
    TypeScript,
    Jsx,
    Tsx,
}

#[derive(Debug, PartialEq)]
enum ModuleType {
    CommonJs,
    EsModules,
    Umd,
    Iife,
    Unknown,
}

#[derive(Debug)]
struct ImportInfo {
    source: String,
    specifiers: Vec<String>,
    is_dynamic: bool,
    location: Option<CodeLocation>,
}

#[derive(Debug)]
struct ExportInfo {
    name: Option<String>,
    is_default: bool,
    location: Option<CodeLocation>,
}

#[derive(Debug)]
struct NodeApiUsage {
    api: String,
    usage_type: ApiUsageType,
    location: Option<CodeLocation>,
}

#[derive(Debug, PartialEq)]
enum ApiUsageType {
    DirectCall,
    RequireStatement,
    ImportStatement,
    PropertyAccess,
}

impl PackageAnalyzer {
    pub fn new(config: &Config) -> Self {
        Self {
            config: config.clone(),
            node_apis: NodeApiRegistry::new(),
        }
    }

    pub async fn analyze(&self, package_data: &PackageData) -> PaktoResult<AnalysisResult> {
        info!("Starting package analysis");

        // Parse package.json
        let package_info = self.parse_package_info(&package_data.package_json)?;

        // Analyze all files
        let mut file_analyses = Vec::new();
        let mut all_issues = Vec::new();
        let mut required_polyfills = HashSet::new();

        for (path, content) in &package_data.files {
            if self.should_analyze_file(path) {
                debug!("Analyzing file: {}", path.display());

                match self.analyze_file(path, content).await {
                    Ok(analysis) => {
                        all_issues.extend(analysis.issues.clone());
                        for usage in &analysis.node_api_usage {
                            if let Some(polyfill) = self.node_apis.get_polyfill(&usage.api) {
                                required_polyfills.insert(polyfill);
                            }
                        }
                        file_analyses.push(analysis);
                    }
                    Err(e) => {
                        warn!("Failed to analyze file {}: {}", path.display(), e);
                        all_issues.push(CompatibilityIssue {
                            level: IssueLevel::Warning,
                            message: format!("Failed to parse file: {}", e),
                            location: Some(CodeLocation::new(path)),
                            suggestion: Some("File may contain syntax errors or unsupported features".to_string()),
                            api: None,
                        });
                    }
                }
            }
        }

        // Analyze dependencies
        let dependency_analysis = self.analyze_dependencies(&package_info).await?;

        // Calculate estimated sizes
        let estimated_size = self.calculate_estimated_sizes(&file_analyses, &required_polyfills);

        // Calculate compatibility score
        let compatibility_score = self.calculate_compatibility_score(&all_issues, &file_analyses);

        // Determine if conversion is feasible
        let feasible = self.is_conversion_feasible(&all_issues, &dependency_analysis);

        Ok(AnalysisResult {
            package_info,
            compatibility_issues: all_issues,
            required_polyfills: required_polyfills.into_iter().collect(),
            dependency_analysis,
            estimated_size,
            compatibility_score,
            feasible,
        })
    }

    /// Parse package.json into PackageInfo
    fn parse_package_info(&self, package_json: &serde_json::Value) -> PaktoResult<PackageInfo> {
        let name = package_json.get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| PaktoError::ParseError {
                file: "package.json".into(),
                message: "Missing or invalid 'name' field".to_string(),
                source: None,
            })?
            .to_string();

        let version = package_json.get("version")
            .and_then(|v| v.as_str())
            .unwrap_or("0.0.0")
            .to_string();

        let description = package_json.get("description")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let main = package_json.get("main")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let dependencies = package_json.get("dependencies")
            .and_then(|v| v.as_object())
            .map(|obj| {
                obj.iter()
                    .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                    .collect()
            })
            .unwrap_or_default();

        let dev_dependencies = package_json.get("devDependencies")
            .and_then(|v| v.as_object())
            .map(|obj| {
                obj.iter()
                    .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                    .collect()
            })
            .unwrap_or_default();

        let keywords = package_json.get("keywords")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();

        let license = package_json.get("license")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        // Determine entry points
        let mut entry_points = Vec::new();
        if let Some(ref main_file) = main {
            entry_points.push(main_file.clone());
        }

        if let Some(module) = package_json.get("module").and_then(|v| v.as_str()) {
            entry_points.push(module.to_string());
        }

        if let Some(browser) = package_json.get("browser") {
            match browser {
                serde_json::Value::String(path) => {
                    entry_points.push(path.clone());
                }
                serde_json::Value::Object(obj) => {
                    for (_, value) in obj {
                        if let serde_json::Value::String(path) = value {
                            if !path.is_empty() && path != "false" {
                                entry_points.push(path.clone());
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        if entry_points.is_empty() {
            entry_points.push("index.js".to_string());
        }

        Ok(PackageInfo {
            name,
            version,
            description,
            main,
            entry_points,
            dependencies,
            dev_dependencies,
            keywords,
            license,
        })
    }

    /// Analyze a single file
    async fn analyze_file(&self, path: &Path, content: &str) -> Result<FileAnalysis> {
        let syntax_type = self.detect_syntax_type(path, content);
        let module_type = self.detect_module_type(content);

        // Parse the file
        let mut visitor = CompatibilityVisitor::new(path.to_string_lossy().to_string());

        match self.parse_and_visit(content, &syntax_type, &mut visitor) {
            Ok(_) => {
                let imports = self.extract_imports(&visitor.imports);
                let exports = self.extract_exports(&visitor.exports);
                let node_api_usage = self.analyze_node_api_usage(&visitor.issues);

                Ok(FileAnalysis {
                    path: path.to_string_lossy().to_string(),
                    syntax_type,
                    module_type,
                    imports,
                    exports,
                    node_api_usage,
                    issues: visitor.issues,
                    estimated_size: content.len(),
                })
            }
            Err(e) => {
                // Fallback to regex-based analysis for unparseable files
                warn!("Failed to parse {}, falling back to regex analysis: {}", path.display(), e);
                self.regex_based_analysis(path, content)
            }
        }
    }

    /// Parse JavaScript/TypeScript and visit AST
    fn parse_and_visit(&self, content: &str, syntax_type: &SyntaxType, visitor: &mut CompatibilityVisitor) -> Result<()> {
        let syntax = match syntax_type {
            SyntaxType::TypeScript | SyntaxType::Tsx => {
                Syntax::Typescript(TsConfig {
                    tsx: matches!(syntax_type, SyntaxType::Tsx),
                    decorators: true,
                    dts: false,
                    no_early_errors: true,
                    disallow_ambiguous_jsx_like: false,
                })
            }
            SyntaxType::JavaScript | SyntaxType::Jsx => {
                Syntax::Es(EsConfig {
                    jsx: matches!(syntax_type, SyntaxType::Jsx),
                    fn_bind: true,
                    decorators: true,
                    decorators_before_export: true,
                    export_default_from: true,
                    import_assertions: true,
                    static_blocks: true,
                    private_in_object: true,
                    allow_super_outside_method: true,
                    allow_return_outside_function: true,
                })
            }
        };

        let lexer = Lexer::new(
            syntax,
            Default::default(),
            StringInput::new(content, Default::default(), Default::default()),
            None,
        );

        let mut parser = Parser::new_from(lexer);
        let module = parser.parse_module()
            .context("Failed to parse JavaScript/TypeScript")?;

        module.visit_with(visitor);

        Ok(())
    }

    /// Fallback regex-based analysis for unparseable files
    fn regex_based_analysis(&self, path: &Path, content: &str) -> Result<FileAnalysis> {
        let mut issues = Vec::new();
        let mut imports = Vec::new();
        let mut node_api_usage = Vec::new();

        // Check for require() calls
        let require_regex = Regex::new(r#"require\s*\(\s*['"`]([^'"`]+)['"`]\s*\)"#)?;
        for cap in require_regex.captures_iter(content) {
            let module_name = &cap[1];
            imports.push(module_name.to_string());

            if self.node_apis.is_node_api(module_name) {
                node_api_usage.push(NodeApiUsage {
                    api: module_name.to_string(),
                    usage_type: ApiUsageType::RequireStatement,
                    location: Some(CodeLocation::new(path)),
                });

                if self.node_apis.is_incompatible(module_name) {
                    issues.push(CompatibilityIssue {
                        level: IssueLevel::Error,
                        message: format!("Incompatible Node.js API: {}", module_name),
                        location: Some(CodeLocation::new(path)),
                        suggestion: self.node_apis.get_suggestion(module_name),
                        api: Some(module_name.to_string()),
                    });
                }
            }
        }

        // Check for import statements
        let import_regex = Regex::new(r#"import\s+.*?\s+from\s+['"`]([^'"`]+)['"`]"#)?;
        for cap in import_regex.captures_iter(content) {
            let module_name = &cap[1];
            imports.push(module_name.to_string());
        }

        // Detect module type
        let module_type = if content.contains("module.exports") || content.contains("exports.") {
            ModuleType::CommonJs
        } else if content.contains("import ") || content.contains("export ") {
            ModuleType::EsModules
        } else {
            ModuleType::Unknown
        };

        Ok(FileAnalysis {
            path: path.to_string_lossy().to_string(),
            syntax_type: self.detect_syntax_type(path, content),
            module_type,
            imports: imports.into_iter().map(|source| ImportInfo {
                source,
                specifiers: vec![],
                is_dynamic: false,
                location: Some(CodeLocation::new(path)),
            }).collect(),
            exports: vec![],
            node_api_usage,
            issues,
            estimated_size: content.len(),
        })
    }

    /// Detect syntax type of file
    fn detect_syntax_type(&self, path: &Path, content: &str) -> SyntaxType {
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            match ext.to_lowercase().as_str() {
                "ts" => SyntaxType::TypeScript,
                "tsx" => SyntaxType::Tsx,
                "jsx" => SyntaxType::Jsx,
                "js" | "mjs" | "cjs" => {
                    // Check content for JSX
                    if content.contains("<") && content.contains("/>") {
                        SyntaxType::Jsx
                    } else {
                        SyntaxType::JavaScript
                    }
                }
                _ => SyntaxType::JavaScript,
            }
        } else {
            SyntaxType::JavaScript
        }
    }

    /// Detect module type based on content
    fn detect_module_type(&self, content: &str) -> ModuleType {
        if content.contains("module.exports") || content.contains("exports.") {
            ModuleType::CommonJs
        } else if content.contains("import ") || content.contains("export ") {
            ModuleType::EsModules
        } else if content.contains("(function (global, factory)") {
            ModuleType::Umd
        } else if content.contains("(function()") || content.contains("(function ()") {
            ModuleType::Iife
        } else {
            ModuleType::Unknown
        }
    }

    /// Check if file should be analyzed
    fn should_analyze_file(&self, path: &Path) -> bool {
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            matches!(ext.to_lowercase().as_str(), "js" | "ts" | "jsx" | "tsx" | "mjs" | "cjs")
        } else {
            false
        }
    }

    /// Extract import information
    fn extract_imports(&self, imports: &[String]) -> Vec<ImportInfo> {
        imports.iter().map(|source| ImportInfo {
            source: source.clone(),
            specifiers: vec![],
            is_dynamic: false,
            location: None,
        }).collect()
    }

    /// Extract export information  
    fn extract_exports(&self, exports: &[String]) -> Vec<ExportInfo> {
        exports.iter().map(|name| ExportInfo {
            name: Some(name.clone()),
            is_default: name == "default",
            location: None,
        }).collect()
    }

    /// Analyze Node.js API usage
    fn analyze_node_api_usage(&self, issues: &[CompatibilityIssue]) -> Vec<NodeApiUsage> {
        issues.iter()
            .filter_map(|issue| {
                if let Some(ref api) = issue.api {
                    Some(NodeApiUsage {
                        api: api.clone(),
                        usage_type: ApiUsageType::DirectCall,
                        location: issue.location.clone(),
                    })
                } else {
                    None
                }
            })
            .collect()
    }

    /// Analyze package dependencies
    async fn analyze_dependencies(&self, package_info: &PackageInfo) -> PaktoResult<DependencyAnalysis> {
        let total_dependencies = package_info.dependencies.len();
        let mut problematic_dependencies = Vec::new();
        let mut browser_compatible = Vec::new();
        let mut needs_polyfills = Vec::new();

        for (dep_name, _version) in &package_info.dependencies {
            if self.is_problematic_dependency(dep_name) {
                problematic_dependencies.push(dep_name.clone());
            } else if self.is_browser_compatible(dep_name) {
                browser_compatible.push(dep_name.clone());
            } else if self.needs_polyfills(dep_name) {
                needs_polyfills.push(dep_name.clone());
            }
        }

        // TODO: Implement circular dependency detection
        let circular_dependencies = Vec::new();

        Ok(DependencyAnalysis {
            total_dependencies,
            problematic_dependencies,
            browser_compatible,
            needs_polyfills,
            circular_dependencies,
        })
    }

    /// Calculate estimated bundle sizes
    fn calculate_estimated_sizes(&self, file_analyses: &[FileAnalysis], polyfills: &HashSet<String>) -> EstimatedSize {
        let base_size: usize = file_analyses.iter().map(|f| f.estimated_size).sum();
        let polyfill_size: usize = polyfills.len() * 2048; // Estimate 2KB per polyfill

        EstimatedSize {
            min_size: base_size,
            max_size: base_size + (base_size / 2), // 50% overhead for bundling
            with_polyfills: base_size + polyfill_size,
            minified: (base_size + polyfill_size) / 3, // Estimate 70% compression
        }
    }

    /// Calculate compatibility score (0.0 to 1.0)
    fn calculate_compatibility_score(&self, issues: &[CompatibilityIssue], files: &[FileAnalysis]) -> f32 {
        if files.is_empty() {
            return 0.0;
        }

        let error_count = issues.iter().filter(|i| matches!(i.level, IssueLevel::Error)).count();
        let warning_count = issues.iter().filter(|i| matches!(i.level, IssueLevel::Warning)).count();

        let penalty = (error_count as f32 * 0.1) + (warning_count as f32 * 0.05);
        (1.0 - penalty).max(0.0)
    }

    /// Determine if conversion is feasible
    fn is_conversion_feasible(&self, issues: &[CompatibilityIssue], deps: &DependencyAnalysis) -> bool {
        let critical_errors = issues.iter()
            .filter(|i| matches!(i.level, IssueLevel::Error))
            .count();

        // Consider conversion feasible if there are fewer than 5 critical errors
        // and no more than 50% problematic dependencies
        critical_errors < 5 &&
            (deps.total_dependencies == 0 ||
                deps.problematic_dependencies.len() * 2 <= deps.total_dependencies)
    }

    /// Check if dependency is known to be problematic
    fn is_problematic_dependency(&self, name: &str) -> bool {
        matches!(name, 
            "fsevents" | "node-gyp" | "sqlite3" | "canvas" | 
            "sharp" | "puppeteer" | "electron" | "native-*"
        ) || name.contains("native") || name.contains("gyp")
    }

    /// Check if dependency is browser compatible
    fn is_browser_compatible(&self, name: &str) -> bool {
        matches!(name,
            "lodash" | "moment" | "axios" | "uuid" | "ramda" |
            "bluebird" | "rxjs" | "immutable" | "classnames"
        )
    }

    /// Check if dependency needs polyfills
    fn needs_polyfills(&self, name: &str) -> bool {
        name.starts_with("crypto") ||
            name.starts_with("buffer") ||
            name.contains("stream") ||
            name.contains("util")
    }
}

impl NodeApiRegistry {
    fn new() -> Self {
        let mut incompatible_apis = HashSet::new();
        let mut polyfillable_apis = HashMap::new();
        let mut replaceable_apis = HashMap::new();

        // Incompatible APIs (cannot be polyfilled)
        for api in &["fs", "child_process", "cluster", "worker_threads", "os", "net", "http", "https"] {
            incompatible_apis.insert(api.to_string());
        }

        // Polyfillable APIs
        polyfillable_apis.insert("crypto".to_string(), "crypto".to_string());
        polyfillable_apis.insert("buffer".to_string(), "buffer".to_string());
        polyfillable_apis.insert("events".to_string(), "events".to_string());
        polyfillable_apis.insert("process".to_string(), "process".to_string());
        polyfillable_apis.insert("util".to_string(), "util".to_string());
        polyfillable_apis.insert("path".to_string(), "path".to_string());

        // Replaceable APIs
        replaceable_apis.insert("crypto".to_string(), "Use Web Crypto API".to_string());
        replaceable_apis.insert("fs".to_string(), "File system operations not available in browser".to_string());

        Self {
            incompatible_apis,
            polyfillable_apis,
            replaceable_apis,
        }
    }

    fn is_node_api(&self, api: &str) -> bool {
        self.incompatible_apis.contains(api) || self.polyfillable_apis.contains_key(api)
    }

    fn is_incompatible(&self, api: &str) -> bool {
        self.incompatible_apis.contains(api)
    }

    fn get_polyfill(&self, api: &str) -> Option<String> {
        self.polyfillable_apis.get(api).cloned()
    }

    fn get_suggestion(&self, api: &str) -> Option<String> {
        self.replaceable_apis.get(api).cloned()
    }
}

impl CompatibilityVisitor {
    fn new(file_path: String) -> Self {
        Self {
            issues: Vec::new(),
            required_polyfills: HashSet::new(),
            imports: Vec::new(),
            exports: Vec::new(),
            current_file: file_path,
        }
    }
}

impl Visit for CompatibilityVisitor {
    fn visit_call_expr(&mut self, call: &CallExpr) {
        // Check for require() calls
        if let Callee::Expr(expr) = &call.callee {
            if let Expr::Ident(ident) = expr.as_ref() {
                if ident.sym == "require" && !call.args.is_empty() {
                    if let Expr::Lit(Lit::Str(s)) = call.args[0].expr.as_ref() {
                        let module_name = s.value.to_string();
                        self.imports.push(module_name.clone());

                        // Check if it's a Node.js API
                        if matches!(module_name.as_str(), "fs" | "crypto" | "child_process" | "os") {
                            self.issues.push(CompatibilityIssue {
                                level: if matches!(module_name.as_str(), "fs" | "child_process") {
                                    IssueLevel::Error
                                } else {
                                    IssueLevel::Warning
                                },
                                message: format!("Node.js API usage: {}", module_name),
                                location: Some(CodeLocation::new(&self.current_file)),
                                suggestion: Some("Consider using browser-compatible alternatives".to_string()),
                                api: Some(module_name),
                            });
                        }
                    }
                }
            }
        }

        call.visit_children_with(self);
    }

    fn visit_import_decl(&mut self, import: &ImportDecl) {
        let source = import.src.value.to_string();
        self.imports.push(source.clone());

        // Check for Node.js API imports
        if matches!(source.as_str(), "fs" | "crypto" | "child_process" | "os") {
            self.issues.push(CompatibilityIssue {
                level: if matches!(source.as_str(), "fs" | "child_process") {
                    IssueLevel::Error
                } else {
                    IssueLevel::Warning
                },
                message: format!("Node.js API import: {}", source),
                location: Some(CodeLocation::new(&self.current_file)),
                suggestion: Some("Consider using browser-compatible alternatives".to_string()),
                api: Some(source),
            });
        }

        import.visit_children_with(self);
    }

    fn visit_export_decl(&mut self, export: &ExportDecl) {
        match &export.decl {
            Decl::Fn(fn_decl) => {
                self.exports.push(fn_decl.ident.sym.to_string());
            }
            Decl::Var(var_decl) => {
                for decl in &var_decl.decls {
                    if let Pat::Ident(ident) = &decl.name {
                        self.exports.push(ident.id.sym.to_string());
                    }
                }
            }
            _ => {}
        }

        export.visit_children_with(self);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    #[test]
    fn test_syntax_type_detection() {
        let config = Config::default();
        let analyzer = PackageAnalyzer::new(&config);

        assert_eq!(analyzer.detect_syntax_type(Path::new("test.js"), ""), SyntaxType::JavaScript);
        assert_eq!(analyzer.detect_syntax_type(Path::new("test.ts"), ""), SyntaxType::TypeScript);
        assert_eq!(analyzer.detect_syntax_type(Path::new("test.jsx"), ""), SyntaxType::Jsx);
        assert_eq!(analyzer.detect_syntax_type(Path::new("test.tsx"), ""), SyntaxType::Tsx);
    }

    #[test]
    fn test_module_type_detection() {
        let config = Config::default();
        let analyzer = PackageAnalyzer::new(&config);

        assert_eq!(analyzer.detect_module_type("module.exports = {}"), ModuleType::CommonJs);
        assert_eq!(analyzer.detect_module_type("export default {}"), ModuleType::EsModules);
        assert_eq!(analyzer.detect_module_type("import foo from 'bar'"), ModuleType::EsModules);
        assert_eq!(analyzer.detect_module_type("(function (global, factory)"), ModuleType::Umd);
    }

    #[test]
    fn test_node_api_registry() {
        let registry = NodeApiRegistry::new();

        assert!(registry.is_node_api("fs"));
        assert!(registry.is_node_api("crypto"));
        assert!(registry.is_incompatible("fs"));
        assert!(!registry.is_incompatible("crypto"));
        assert!(registry.get_polyfill("crypto").is_some());
        assert!(registry.get_polyfill("fs").is_none());
    }
}