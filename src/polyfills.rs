use std::collections::HashMap;

pub struct PolyfillRegistry {
    polyfills: HashMap<String, String>,
}

impl PolyfillRegistry {
    pub fn new() -> Self {
        let mut polyfills = HashMap::new();

        // Add built-in polyfills
        polyfills.insert(
            "crypto".to_string(),
            include_str!("../polyfills/crypto.js").to_string()
        );

        polyfills.insert(
            "buffer".to_string(),
            include_str!("../polyfills/buffer.js").to_string()
        );

        polyfills.insert(
            "events".to_string(),
            include_str!("../polyfills/events.js").to_string()
        );

        polyfills.insert(
            "process".to_string(),
            include_str!("../polyfills/process.js").to_string()
        );

        polyfills.insert(
            "path".to_string(),
            include_str!("../polyfills/path.js").to_string()
        );

        polyfills.insert(
            "util".to_string(),
            include_str!("../polyfills/util.js").to_string()
        );

        Self { polyfills }
    }

    pub fn get_polyfill(&self, api: &str) -> Option<&String> {
        self.polyfills.get(api)
    }

    pub fn add_polyfill(&mut self, api: String, code: String) {
        self.polyfills.insert(api, code);
    }

    pub fn available_polyfills(&self) -> Vec<&String> {
        self.polyfills.keys().collect()
    }

    pub fn has_polyfill(&self, api: &str) -> bool {
        self.polyfills.contains_key(api)
    }

    pub fn get_polyfill_size(&self, api: &str) -> Option<usize> {
        self.polyfills.get(api).map(|code| code.len())
    }

    pub fn get_total_size(&self, apis: &[String]) -> usize {
        apis.iter()
            .filter_map(|api| self.get_polyfill_size(api))
            .sum()
    }

    /// Get polyfills needed for common Node.js APIs
    pub fn get_polyfills_for_apis(&self, apis: &[String]) -> Vec<String> {
        let mut needed = Vec::new();

        for api in apis {
            match api.as_str() {
                "crypto" | "crypto-js" => {
                    if self.has_polyfill("crypto") {
                        needed.push("crypto".to_string());
                    }
                }
                "buffer" | "Buffer" => {
                    if self.has_polyfill("buffer") {
                        needed.push("buffer".to_string());
                    }
                }
                "events" | "EventEmitter" => {
                    if self.has_polyfill("events") {
                        needed.push("events".to_string());
                    }
                }
                "process" => {
                    if self.has_polyfill("process") {
                        needed.push("process".to_string());
                    }
                }
                "path" => {
                    if self.has_polyfill("path") {
                        needed.push("path".to_string());
                    }
                }
                "util" => {
                    if self.has_polyfill("util") {
                        needed.push("util".to_string());
                    }
                }
                _ => {
                    // Check if we have a direct polyfill
                    if self.has_polyfill(api) {
                        needed.push(api.clone());
                    }
                }
            }
        }

        needed.sort();
        needed.dedup();
        needed
    }

    /// Load custom polyfills from a directory
    pub fn load_custom_polyfills(&mut self, dir_path: &std::path::Path) -> Result<usize, std::io::Error> {
        if !dir_path.exists() {
            return Ok(0);
        }

        let mut loaded = 0;

        for entry in std::fs::read_dir(dir_path)? {
            let entry = entry?;
            let path = entry.path();

            if let Some(extension) = path.extension() {
                if extension == "js" {
                    if let Some(name) = path.file_stem().and_then(|s| s.to_str()) {
                        let content = std::fs::read_to_string(&path)?;
                        self.add_polyfill(name.to_string(), content);
                        loaded += 1;
                    }
                }
            }
        }

        Ok(loaded)
    }
}

impl Default for PolyfillRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_polyfill_registry_creation() {
        let registry = PolyfillRegistry::new();
        assert!(registry.has_polyfill("crypto"));
        assert!(registry.has_polyfill("buffer"));
        assert!(registry.has_polyfill("events"));
        assert!(registry.has_polyfill("process"));
        assert!(registry.has_polyfill("path"));
        assert!(registry.has_polyfill("util"));
    }

    #[test]
    fn test_get_polyfill() {
        let registry = PolyfillRegistry::new();

        let crypto_polyfill = registry.get_polyfill("crypto");
        assert!(crypto_polyfill.is_some());
        assert!(crypto_polyfill.unwrap().contains("createHash"));

        let nonexistent = registry.get_polyfill("nonexistent");
        assert!(nonexistent.is_none());
    }

    #[test]
    fn test_available_polyfills() {
        let registry = PolyfillRegistry::new();
        let available = registry.available_polyfills();

        assert!(available.len() >= 6);
        assert!(available.iter().any(|&s| s == "crypto"));
        assert!(available.iter().any(|&s| s == "buffer"));
    }

    #[test]
    fn test_get_polyfills_for_apis() {
        let registry = PolyfillRegistry::new();

        let apis = vec![
            "crypto".to_string(),
            "buffer".to_string(),
            "unknown".to_string(),
        ];

        let needed = registry.get_polyfills_for_apis(&apis);
        assert!(needed.contains(&"crypto".to_string()));
        assert!(needed.contains(&"buffer".to_string()));
        assert!(!needed.contains(&"unknown".to_string()));
    }

    #[test]
    fn test_add_custom_polyfill() {
        let mut registry = PolyfillRegistry::new();

        let custom_code = "window.customPolyfill = { test: true };".to_string();
        registry.add_polyfill("custom".to_string(), custom_code.clone());

        assert!(registry.has_polyfill("custom"));
        assert_eq!(registry.get_polyfill("custom"), Some(&custom_code));
    }

    #[test]
    fn test_polyfill_sizes() {
        let registry = PolyfillRegistry::new();

        let crypto_size = registry.get_polyfill_size("crypto");
        assert!(crypto_size.is_some());
        assert!(crypto_size.unwrap() > 0);

        let apis = vec!["crypto".to_string(), "buffer".to_string()];
        let total_size = registry.get_total_size(&apis);
        assert!(total_size > 0);
    }
}