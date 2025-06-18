use std::collections::HashMap;
use std::path::{Path, PathBuf};
use anyhow::{Context, Result};
use tracing::{debug, info, warn};
use swc_core::common::{SourceMap, GLOBALS};
use swc_core::ecma::parser::{lexer::Lexer, Parser, StringInput, Syntax, TsConfig, EsConfig};
use swc_core::ecma::ast::*;
use swc_core::ecma::visit::{FoldWith, VisitMut, VisitMutWith};
use swc_core::ecma::transforms::base::resolver;
use swc_core::ecma::transforms::compat;
use swc_core::ecma::transforms::module::common_js;
use swc_core::ecma::codegen::{text_writer::JsWriter, Emitter};

use crate::config::Config;
use crate::converter::{PackageData, TransformedPackage, ConvertOptions, AnalysisResult};
use crate::cli::EsTarget;
use crate::errors::{PaktoError, Result as PaktoResult};
use crate::polyfills::PolyfillRegistry;

/// Transforms JavaScript/TypeScript code for browser compatibility
pub struct CodeTransformer {
    config: Config,
    polyfills: PolyfillRegistry,
    source_map: std::sync::Arc<SourceMap>,
}

/// Custom AST visitor for OutSystems-specific transformations
struct OutSystemsTransformer {
    polyfills_needed: Vec<String>,
    global_name: Option<String>,
    namespace: Option<String>,
}

/// Module transformation result
#[derive(Debug)]
struct ModuleTransformResult {
    code: String,
    polyfills_used: Vec<String>,
    source_map: Option<String>,
}

/// Polyfill injection strategy
#[derive(Debug, Clone)]
enum PolyfillStrategy {
    Inline,       // Inject polyfill code directly
    Global,       // Assume polyfill is available globally
    Conditional,  // Check if native API exists first
}

impl CodeTransformer {
    pub fn new(config: &Config) -> Self {
        Self {
            config: config.clone(),
            polyfills: PolyfillRegistry::new(),
            source_map: std::sync::Arc::new(SourceMap::default()),
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
            source_map: None, // TODO: Implement source map generation
        })
    }

    /// Transform a single file
    async fn transform_file(
        &self,
        path: &Path,
        content: &str,
        options: &ConvertOptions,
        _analysis: &AnalysisResult,
    ) -> Result<ModuleTransformResult> {
        let syntax = self.detect_syntax(path, content);

        // Parse the file
        let lexer = Lexer::new(
            syntax,
            Default::default(),
            StringInput::new(content, Default::default(), Default::default()),
            None,
        );

        let mut parser = Parser::new_from(lexer);
        let mut module = parser.parse_module()
            .context("Failed to parse JavaScript/TypeScript")?;

        // Apply transformations
        let mut transformer = OutSystemsTransformer::new(
            options.name.clone(),
            options.namespace.clone(),
        );

        GLOBALS.set(&Default::default(), || {
            // Apply SWC transformations
            module = module.fold_with(&mut resolver(unresolved_mark(), top_level_mark(), false));

            // Convert ES modules to CommonJS first
            module = module.fold_with(&mut common_js::common_js(
                unresolved_mark(),
                Default::default(),
            ));

            // Apply compatibility transforms based on target
            module = self.apply_compatibility_transforms(module, &options.target_es_version)?;

            // Apply OutSystems-specific transforms
            module.visit_mut_with(&mut transformer);

            Ok::<(), anyhow::Error>(())
        })?;

        // Generate code
        let mut buf = Vec::new();
        {
            let writer = JsWriter::new(self.source_map.clone(), "\n", &mut buf, None);
            let mut emitter = Emitter {
                cfg: Default::default(),
                cm: self.source_map.clone(),
                comments: None,
                wr: writer,
            };

            emitter.emit_module(&module)
                .context("Failed to generate JavaScript code")?;
        }

        let code = String::from_utf8(buf)
            .context("Generated code is not valid UTF-8")?;

        Ok(ModuleTransformResult {
            code,
            polyfills_used: transformer.polyfills_needed,
            source_map: None,
        })
    }

    /// Detect syntax type for parsing
    fn detect_syntax(&self, path: &Path, content: &str) -> Syntax {
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            match ext.to_lowercase().as_str() {
                "ts" => Syntax::Typescript(TsConfig {
                    tsx: false,
                    decorators: true,
                    dts: false,
                    no_early_errors: true,
                    disallow_ambiguous_jsx_like: false,
                }),
                "tsx" => Syntax::Typescript(TsConfig {
                    tsx: true,
                    decorators: true,
                    dts: false,
                    no_early_errors: true,
                    disallow_ambiguous_jsx_like: false,
                }),
                "jsx" => Syntax::Es(EsConfig {
                    jsx: true,
                    fn_bind: true,
                    decorators: true,
                    decorators_before_export: true,
                    export_default_from: true,
                    import_assertions: true,
                    static_blocks: true,
                    private_in_object: true,
                    allow_super_outside_method: true,
                    allow_return_outside_function: true,
                }),
                _ => Syntax::Es(EsConfig {
                    jsx: content.contains("<") && content.contains("/>"),
                    fn_bind: true,
                    decorators: true,
                    decorators_before_export: true,
                    export_default_from: true,
                    import_assertions: true,
                    static_blocks: true,
                    private_in_object: true,
                    allow_super_outside_method: true,
                    allow_return_outside_function: true,
                }),
            }
        } else {
            Syntax::Es(Default::default())
        }
    }

    /// Apply compatibility transformations based on ES target
    fn apply_compatibility_transforms(&self, mut module: Module, target: &EsTarget) -> Result<Module> {
        let es_version = match target {
            EsTarget::Es5 => swc_ecma_ast::EsVersion::Es5,
            EsTarget::Es2015 => swc_ecma_ast::EsVersion::Es2015,
            EsTarget::Es2017 => swc_ecma_ast::EsVersion::Es2017,
            EsTarget::Es2018 => swc_ecma_ast::EsVersion::Es2018,
            EsTarget::Es2020 => swc_ecma_ast::EsVersion::Es2020,
            EsTarget::EsNext => swc_ecma_ast::EsVersion::EsNext,
        };

        // Apply compatibility transforms
        match target {
            EsTarget::Es5 => {
                module = module.fold_with(&mut compat::es2015::es2015(
                    Default::default(),
                    Default::default(),
                ));
                module = module.fold_with(&mut compat::es3::es3(Default::default()));
            }
            EsTarget::Es2015 => {
                module = module.fold_with(&mut compat::es2016::es2016());
                module = module.fold_with(&mut compat::es2017::es2017(Default::default()));
                module = module.fold_with(&mut compat::es2018::es2018(Default::default()));
            }
            EsTarget::Es2017 => {
                module = module.fold_with(&mut compat::es2018::es2018(Default::default()));
                module = module.fold_with(&mut compat::es2020::es2020(Default::default()));
            }
            _ => {
                // For newer targets, apply minimal transforms
            }
        }

        Ok(module)
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
                "\n// === {} ===\n",
                path.display()
            ));
            bundled_code.push_str(&content);
            bundled_code.push('\n');
        }

        // Generate module footer
        bundled_code.push_str(&self.generate_module_footer(options, analysis)?);

        Ok(bundled_code)
    }

    /// Generate module header (IIFE start, polyfills, etc.)
    fn generate_module_header(
        &self,
        options: &ConvertOptions,
        analysis: &AnalysisResult,
    ) -> PaktoResult<String> {
        let mut header = String::new();

        // Start IIFE
        header.push_str("(function(global, factory) {\n");
        header.push_str("  'use strict';\n\n");

        // UMD pattern for compatibility
        header.push_str("  if (typeof module === 'object' && typeof module.exports === 'object') {\n");
        header.push_str("    // Node.js\n");
        header.push_str("    module.exports = factory();\n");
        header.push_str("  } else if (typeof define === 'function' && define.amd) {\n");
        header.push_str("    // AMD\n");
        header.push_str("    define(factory);\n");
        header.push_str("  } else {\n");
        header.push_str("    // Browser globals (OutSystems)\n");

        let global_name = options.name.as_deref()
            .unwrap_or(&analysis.package_info.name);

        if let Some(ref namespace) = options.namespace {
            header.push_str(&format!("    global.{} = global.{} || {{}};\n", namespace, namespace));
            header.push_str(&format!("    global.{}.{} = factory();\n", namespace, global_name));
        } else {
            header.push_str(&format!("    global.{} = factory();\n", global_name));
        }

        header.push_str("  }\n");
        header.push_str("})(typeof window !== 'undefined' ? window : this, function() {\n");
        header.push_str("  'use strict';\n\n");

        // Add strict mode and common utilities
        header.push_str("  // Common utilities\n");
        header.push_str("  var hasOwnProperty = Object.prototype.hasOwnProperty;\n");
        header.push_str("  var toString = Object.prototype.toString;\n\n");

        Ok(header)
    }

    /// Generate module footer (exports, IIFE end)
    fn generate_module_footer(
        &self,
        _options: &ConvertOptions,
        analysis: &AnalysisResult,
    ) -> PaktoResult<String> {
        let mut footer = String::new();

        // Determine what to export
        footer.push_str("\n  // Module exports\n");

        // Check if there's a main entry point
        if let Some(ref main) = analysis.package_info.main {
            footer.push_str(&format!("  // Main entry point: {}\n", main));
        }

        // For now, export everything that was defined
        footer.push_str("  return typeof module !== 'undefined' && module.exports ? module.exports : {};\n");

        // Close IIFE
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

        // Find injection point (after IIFE start but before main code)
        let lines: Vec<&str> = code.lines().collect();
        let mut injection_point = 0;

        for (i, line) in lines.iter().enumerate() {
            if line.contains("'use strict';") && i > 0 {
                injection_point = i + 1;
                break;
            }
        }

        // Add lines before injection point
        for (i, line) in lines.iter().enumerate() {
            polyfilled_code.push_str(line);
            polyfilled_code.push('\n');

            if i == injection_point {
                // Inject polyfills here
                polyfilled_code.push_str("\n  // === Polyfills ===\n");

                for polyfill_name in polyfills_needed {
                    if let Some(polyfill_code) = self.polyfills.get_polyfill(polyfill_name) {
                        polyfilled_code.push_str(&format!("  // Polyfill: {}\n", polyfill_name));

                        // Indent polyfill code to match IIFE indentation
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

impl OutSystemsTransformer {
    fn new(global_name: Option<String>, namespace: Option<String>) -> Self {
        Self {
            polyfills_needed: Vec::new(),
            global_name,
            namespace,
        }
    }
}

impl VisitMut for OutSystemsTransformer {
    fn visit_mut_call_expr(&mut self, call: &mut CallExpr) {
        // Transform require() calls
        if let Callee::Expr(expr) = &mut call.callee {
            if let Expr::Ident(ident) = expr.as_mut() {
                if ident.sym == "require" && !call.args.is_empty() {
                    if let Expr::Lit(Lit::Str(s)) = call.args[0].expr.as_mut() {
                        let module_name = s.value.to_string();

                        // Transform Node.js API requires to polyfills
                        match module_name.as_str() {
                            "crypto" => {
                                self.polyfills_needed.push("crypto".to_string());
                                s.value = "cryptoPolyfill".into();
                            }
                            "buffer" => {
                                self.polyfills_needed.push("buffer".to_string());
                                // Transform to: require('buffer').Buffer or BufferPolyfill
                                *expr = Box::new(Expr::Ident(Ident::new("BufferPolyfill".into(), Default::default())));
                                // Remove the require call entirely by replacing with direct reference
                                return;
                            }
                            "events" => {
                                self.polyfills_needed.push("events".to_string());
                                s.value = "EventEmitterPolyfill".into();
                            }
                            "process" => {
                                self.polyfills_needed.push("process".to_string());
                                s.value = "processPolyfill".into();
                            }
                            _ => {}
                        }
                    }
                }
            }
        }

        call.visit_mut_children_with(self);
    }

    fn visit_mut_import_decl(&mut self, import: &mut ImportDecl) {
        let source = import.src.value.to_string();

        // Transform Node.js API imports to polyfills
        match source.as_str() {
            "crypto" => {
                self.polyfills_needed.push("crypto".to_string());
                import.src.value = "cryptoPolyfill".into();
            }
            "buffer" => {
                self.polyfills_needed.push("buffer".to_string());
                import.src.value = "BufferPolyfill".into();
            }
            "events" => {
                self.polyfills_needed.push("events".to_string());
                import.src.value = "EventEmitterPolyfill".into();
            }
            "process" => {
                self.polyfills_needed.push("process".to_string());
                import.src.value = "processPolyfill".into();
            }
            _ => {}
        }

        import.visit_mut_children_with(self);
    }

    fn visit_mut_member_expr(&mut self, member: &mut MemberExpr) {
        // Transform process.env access
        if let Expr::Ident(obj) = member.obj.as_ref() {
            if obj.sym == "process" {
                if let MemberProp::Ident(prop) = &member.prop {
                    if prop.sym == "env" {
                        self.polyfills_needed.push("process".to_string());
                        // Transform process.env to processPolyfill.env
                        member.obj = Box::new(Expr::Ident(Ident::new("processPolyfill".into(), Default::default())));
                    }
                }
            }
        }

        member.visit_mut_children_with(self);
    }
}

// Helper functions for SWC
fn unresolved_mark() -> swc_core::common::Mark {
    swc_core::common::Mark::new()
}

fn top_level_mark() -> swc_core::common::Mark {
    swc_core::common::Mark::new()
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
    fn test_syntax_detection() {
        let config = Config::default();
        let transformer = CodeTransformer::new(&config);

        let js_syntax = transformer.detect_syntax(Path::new("test.js"), "const x = 1;");
        assert!(matches!(js_syntax, Syntax::Es(_)));

        let ts_syntax = transformer.detect_syntax(Path::new("test.ts"), "const x: number = 1;");
        assert!(matches!(ts_syntax, Syntax::Typescript(_)));
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
}