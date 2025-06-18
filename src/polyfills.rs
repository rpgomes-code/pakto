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
}