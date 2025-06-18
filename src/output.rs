use std::collections::HashMap;
use anyhow::{Context, Result};
use handlebars::{Handlebars, Helper, Output, RenderContext, RenderError};
use serde_json::{json, Value};
use tracing::{debug, info};

use crate::config::Config;
use crate::converter::{BundledCode, ConvertOptions, PackageInfo};
use crate::cli::EsTarget;
use crate::errors::{PaktoError, Result as PaktoResult};

/// Generates final output files using templates
pub struct OutputGenerator {
    config: Config,
    handlebars: Handlebars<'static>,
}

/// Template context for code generation
#[derive(Debug, serde::Serialize)]
struct TemplateContext {
    // Package information
    package_name: String,
    package_version: String,
    package_description: Option<String>,

    // Output configuration
    output_name: String,
    namespace: Option<String>,
    global_name: String,

    // Code content
    bundled_code: String,
    polyfills_code: String,

    // Metadata
    generated_at: String,
    generator_version: String,
    target_es_version: String,

    // Features
    has_polyfills: bool,
    has_namespace: bool,
    is_minified: bool,

    // Statistics
    original_size: usize,
    bundle_size: usize,

    // Custom data
    custom: HashMap<String, Value>,
}

/// Available output templates
#[derive(Debug, Clone)]
enum OutputTemplate {
    /// Universal Module Definition (UMD) pattern
    Umd,
    /// Immediately Invoked Function Expression
    Iife,
    /// CommonJS module
    CommonJs,
    /// ES Module
    EsModule,
    /// OutSystems-specific format
    OutSystems,
}

impl OutputGenerator {
    pub fn new(config: &Config) -> Self {
        let mut handlebars = Handlebars::new();

        // Register built-in templates
        Self::register_templates(&mut handlebars);

        // Register helper functions
        Self::register_helpers(&mut handlebars);

        Self {
            config: config.clone(),
            handlebars,
        }
    }

    pub fn generate(
        &self,
        bundled: &BundledCode,
        options: &ConvertOptions,
        package_info: &PackageInfo,
    ) -> PaktoResult<String> {
        info!("Generating output code");

        // Create template context
        let context = self.create_template_context(bundled, options, package_info)?;

        // Select appropriate template
        let template_name = self.select_template(options);
        debug!("Using template: {}", template_name);

        // Render the template
        let rendered = self.handlebars
            .render(&template_name, &context)
            .context("Failed to render output template")?;

        // Apply post-processing
        let final_code = self.post_process_output(&rendered, options)?;

        Ok(final_code)
    }

    /// Create template context from inputs
    fn create_template_context(
        &self,
        bundled: &BundledCode,
        options: &ConvertOptions,
        package_info: &PackageInfo,
    ) -> PaktoResult<TemplateContext> {
        let output_name = options.name.clone()
            .unwrap_or_else(|| package_info.name.clone());

        let global_name = self.sanitize_global_name(&output_name);

        // Extract polyfills from bundled code
        let (main_code, polyfills_code) = self.extract_polyfills(&bundled.code)?;

        Ok(TemplateContext {
            package_name: package_info.name.clone(),
            package_version: package_info.version.clone(),
            package_description: package_info.description.clone(),

            output_name: output_name.clone(),
            namespace: options.namespace.clone(),
            global_name,

            bundled_code: main_code,
            polyfills_code,

            generated_at: chrono::Utc::now().to_rfc3339(),
            generator_version: env!("CARGO_PKG_VERSION").to_string(),
            target_es_version: format!("{:?}", options.target_es_version),

            has_polyfills: !polyfills_code.trim().is_empty(),
            has_namespace: options.namespace.is_some(),
            is_minified: options.minify,

            original_size: bundled.unminified_size,
            bundle_size: bundled.code.len(),

            custom: HashMap::new(),
        })
    }

    /// Select appropriate template based on options
    fn select_template(&self, options: &ConvertOptions) -> String {
        // For OutSystems, we always use the OutSystems-specific template
        // which is essentially a UMD pattern optimized for OutSystems
        "outsystems".to_string()
    }

    /// Register built-in templates
    fn register_templates(handlebars: &mut Handlebars<'static>) {
        // OutSystems-specific template
        handlebars.register_template_string(
            "outsystems",
            include_str!("../templates/outsystems.hbs")
        ).unwrap_or_else(|_| {
            // Fallback if template file doesn't exist
            handlebars.register_template_string(
                "outsystems",
                Self::default_outsystems_template()
            ).unwrap();
        });

        // UMD template
        handlebars.register_template_string(
            "umd",
            Self::umd_template()
        ).unwrap();

        // IIFE template
        handlebars.register_template_string(
            "iife",
            Self::iife_template()
        ).unwrap();
    }

    /// Register helper functions for templates
    fn register_helpers(handlebars: &mut Handlebars<'static>) {
        // Helper to indent code
        handlebars.register_helper("indent", Box::new(indent_helper));

        // Helper to generate unique variable names
        handlebars.register_helper("var_name", Box::new(var_name_helper));

        // Helper to format comments
        handlebars.register_helper("comment", Box::new(comment_helper));

        // Helper for conditional output
        handlebars.register_helper("if_not_empty", Box::new(if_not_empty_helper));
    }

    /// Default OutSystems template
    fn default_outsystems_template() -> &'static str {
        r#"/**
 * {{package_name}} v{{package_version}} - OutSystems Compatible
 * {{#if package_description}}{{package_description}}{{/if}}
 *
 * Generated by Pakto v{{generator_version}} on {{generated_at}}
 * Target: {{target_es_version}}
 *
 * This bundle is optimized for OutSystems platform
 */
(function(global, factory) {
  'use strict';

  // Universal Module Definition (UMD) pattern for maximum compatibility
  if (typeof module === 'object' && typeof module.exports === 'object') {
    // Node.js environment
    module.exports = factory();
  } else if (typeof define === 'function' && define.amd) {
    // AMD environment
    define(factory);
  } else {
    // Browser globals - OutSystems target
    {{#if has_namespace}}
    global.{{namespace}} = global.{{namespace}} || {};
    global.{{namespace}}.{{global_name}} = factory();
    {{else}}
    global.{{global_name}} = factory();
    {{/if}}
  }
})(typeof window !== 'undefined' ? window : this, function() {
  'use strict';

  {{#if has_polyfills}}
  // ================================================================
  // Polyfills for Browser Compatibility
  // ================================================================
  {{indent polyfills_code 2}}

  {{/if}}
  // ================================================================
  // Main Module Code
  // ================================================================
  {{indent bundled_code 2}}

  // ================================================================
  // Module Exports
  // ================================================================
  {{#if_not_empty bundled_code}}
  return typeof module !== 'undefined' && module.exports ? module.exports : {};
  {{else}}
  return {};
  {{/if_not_empty}}
});

{{comment "Bundle Information"}}
{{comment (concat "Original size: " original_size " bytes")}}
{{comment (concat "Bundle size: " bundle_size " bytes")}}
{{#if is_minified}}
{{comment "Code has been minified"}}
{{/if}}
"#
    }

    /// UMD template
    fn umd_template() -> &'static str {
        r#"(function (global, factory) {
    typeof exports === 'object' && typeof module !== 'undefined' ? factory(exports) :
    typeof define === 'function' && define.amd ? define(['exports'], factory) :
    (global = global || self, factory(global.{{global_name}} = {}));
}(this, (function (exports) { 'use strict';

{{indent bundled_code 4}}

})));
"#
    }

    /// IIFE template
    fn iife_template() -> &'static str {
        r#"(function() {
  'use strict';

  {{indent bundled_code 2}}

  return typeof module !== 'undefined' && module.exports ? module.exports : {};
})();"#
    }

    /// Extract polyfills from bundled code
    fn extract_polyfills(&self, code: &str) -> PaktoResult<(String, String)> {
        let polyfill_start = "// === Polyfills ===";
        let polyfill_end = "// === End Polyfills ===";

        if let Some(start_pos) = code.find(polyfill_start) {
            if let Some(end_pos) = code.find(polyfill_end) {
                let polyfills = code[start_pos + polyfill_start.len()..end_pos]
                    .trim()
                    .to_string();

                let main_code = code[..start_pos].to_string() +
                    &code[end_pos + polyfill_end.len()..];

                return Ok((main_code, polyfills));
            }
        }

        // No polyfills found
        Ok((code.to_string(), String::new()))
    }

    /// Sanitize name for use as global variable
    fn sanitize_global_name(&self, name: &str) -> String {
        let mut sanitized = name
            .chars()
            .map(|c| if c.is_alphanumeric() { c } else { '_' })
            .collect::<String>();

        // Ensure it starts with a letter or underscore
        if let Some(first_char) = sanitized.chars().next() {
            if first_char.is_ascii_digit() {
                sanitized = format!("_{}", sanitized);
            }
        }

        // Convert to PascalCase for global variables
        Self::to_pascal_case(&sanitized)
    }

    /// Convert string to PascalCase
    fn to_pascal_case(s: &str) -> String {
        s.split('_')
            .filter(|part| !part.is_empty())
            .map(|part| {
                let mut chars = part.chars();
                match chars.next() {
                    None => String::new(),
                    Some(first) => first.to_uppercase().collect::<String>() + &chars.as_str().to_lowercase(),
                }
            })
            .collect()
    }

    /// Apply post-processing to the generated output
    fn post_process_output(&self, code: &str, options: &ConvertOptions) -> PaktoResult<String> {
        let mut processed = code.to_string();

        // Minify if requested
        if options.minify {
            processed = self.minify_code(&processed)?;
        }

        // Clean up extra whitespace
        processed = self.clean_whitespace(&processed);

        // Validate syntax
        self.validate_output(&processed)?;

        Ok(processed)
    }

    /// Minify JavaScript code
    fn minify_code(&self, code: &str) -> PaktoResult<String> {
        // Use the minifier crate for basic minification
        match minifier::js::minify(code) {
            Ok(minified) => Ok(minified),
            Err(e) => {
                // If minification fails, return original code with warning
                debug!("Minification failed: {}, using original code", e);
                Ok(code.to_string())
            }
        }
    }

    /// Clean up whitespace in generated code
    fn clean_whitespace(&self, code: &str) -> String {
        // Remove trailing whitespace from each line
        let lines: Vec<String> = code
            .lines()
            .map(|line| line.trim_end().to_string())
            .collect();

        // Remove excessive blank lines
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

    /// Basic validation of generated output
    fn validate_output(&self, code: &str) -> PaktoResult<()> {
        // Check for balanced braces
        let open_braces = code.matches('{').count();
        let close_braces = code.matches('}').count();
        if open_braces != close_braces {
            return Err(PaktoError::TransformError {
                message: format!("Unbalanced braces: {} open, {} close", open_braces, close_braces),
                source: None,
            });
        }

        // Check for balanced parentheses
        let open_parens = code.matches('(').count();
        let close_parens = code.matches(')').count();
        if open_parens != close_parens {
            return Err(PaktoError::TransformError {
                message: format!("Unbalanced parentheses: {} open, {} close", open_parens, close_parens),
                source: None,
            });
        }

        // Check for basic syntax errors
        if code.contains("undefined undefined") || code.contains("null null") {
            return Err(PaktoError::TransformError {
                message: "Possible syntax error detected in output".to_string(),
                source: None,
            });
        }

        Ok(())
    }
}

// Handlebars helper functions

fn indent_helper(
    h: &Helper,
    _: &Handlebars,
    _: &handlebars::Context,
    _: &mut RenderContext,
    out: &mut dyn Output,
) -> Result<(), RenderError> {
    let content = h.param(0)
        .and_then(|v| v.value().as_str())
        .ok_or_else(|| RenderError::new("indent helper requires a string parameter"))?;

    let indent_size = h.param(1)
        .and_then(|v| v.value().as_u64())
        .unwrap_or(2) as usize;

    let indent = " ".repeat(indent_size);
    let indented = content
        .lines()
        .map(|line| if line.trim().is_empty() {
            line.to_string()
        } else {
            format!("{}{}", indent, line)
        })
        .collect::<Vec<_>>()
        .join("\n");

    out.write(&indented)?;
    Ok(())
}

fn var_name_helper(
    h: &Helper,
    _: &Handlebars,
    _: &handlebars::Context,
    _: &mut RenderContext,
    out: &mut dyn Output,
) -> Result<(), RenderError> {
    let name = h.param(0)
        .and_then(|v| v.value().as_str())
        .ok_or_else(|| RenderError::new("var_name helper requires a string parameter"))?;

    let var_name = name
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '_' })
        .collect::<String>();

    out.write(&var_name)?;
    Ok(())
}

fn comment_helper(
    h: &Helper,
    _: &Handlebars,
    _: &handlebars::Context,
    _: &mut RenderContext,
    out: &mut dyn Output,
) -> Result<(), RenderError> {
    let text = h.param(0)
        .and_then(|v| v.value().as_str())
        .ok_or_else(|| RenderError::new("comment helper requires a string parameter"))?;

    let comment = format!("// {}", text);
    out.write(&comment)?;
    Ok(())
}

fn if_not_empty_helper(
    h: &Helper,
    hb: &Handlebars,
    ctx: &handlebars::Context,
    rc: &mut RenderContext,
    out: &mut dyn Output,
) -> Result<(), RenderError> {
    let value = h.param(0)
        .ok_or_else(|| RenderError::new("if_not_empty helper requires a parameter"))?;

    let is_not_empty = match value.value() {
        Value::String(s) => !s.trim().is_empty(),
        Value::Array(arr) => !arr.is_empty(),
        Value::Object(obj) => !obj.is_empty(),
        Value::Null => false,
        _ => true,
    };

    if is_not_empty {
        if let Some(template) = h.template() {
            template.render(hb, ctx, rc, out)?;
        }
    } else if let Some(else_template) = h.inverse() {
        else_template.render(hb, ctx, rc, out)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::cli::EsTarget;

    #[test]
    fn test_output_generator_creation() {
        let config = Config::default();
        let generator = OutputGenerator::new(&config);
        assert!(generator.handlebars.has_template("outsystems"));
    }

    #[test]
    fn test_sanitize_global_name() {
        let config = Config::default();
        let generator = OutputGenerator::new(&config);

        assert_eq!(generator.sanitize_global_name("my-package"), "MyPackage");
        assert_eq!(generator.sanitize_global_name("@types/node"), "TypesNode");
        assert_eq!(generator.sanitize_global_name("123invalid"), "_123Invalid");
    }

    #[test]
    fn test_to_pascal_case() {
        assert_eq!(OutputGenerator::to_pascal_case("hello_world"), "HelloWorld");
        assert_eq!(OutputGenerator::to_pascal_case("test_case"), "TestCase");
        assert_eq!(OutputGenerator::to_pascal_case("single"), "Single");
    }

    #[test]
    fn test_extract_polyfills() {
        let config = Config::default();
        let generator = OutputGenerator::new(&config);

        let code = r#"
some code
// === Polyfills ===
polyfill code
// === End Polyfills ===
more code
"#;

        let (main, polyfills) = generator.extract_polyfills(code).unwrap();
        assert!(main.contains("some code"));
        assert!(main.contains("more code"));
        assert!(!main.contains("polyfill code"));
        assert!(polyfills.contains("polyfill code"));
    }
}