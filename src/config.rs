use std::path::{Path, PathBuf};
use std::collections::HashMap;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use crate::cli::{BundleStrategy, EsTarget};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// NPM registry configuration
    pub npm: NpmConfig,

    /// Output configuration
    pub output: OutputConfig,

    /// Polyfill configuration
    pub polyfills: PolyfillConfig,

    /// Bundle configuration
    pub bundle: BundleConfig,

    /// Cache configuration
    pub cache: CacheConfig,

    /// Custom templates
    pub templates: TemplateConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NpmConfig {
    /// NPM registry URL
    #[serde(default = "default_npm_registry")]
    pub registry: String,

    /// Request timeout in seconds
    #[serde(default = "default_timeout")]
    pub timeout: u64,

    /// User agent for requests
    #[serde(default = "default_user_agent")]
    pub user_agent: String,

    /// Auth token for private registries
    pub auth_token: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputConfig {
    /// Default output directory
    #[serde(default = "default_output_dir")]
    pub directory: PathBuf,

    /// Default file naming pattern
    #[serde(default = "default_naming_pattern")]
    pub naming_pattern: String,

    /// Default minification setting
    #[serde(default)]
    pub minify: bool,

    /// Default ES target
    #[serde(default)]
    pub target: EsTarget,

    /// Include source maps
    #[serde(default)]
    pub source_maps: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolyfillConfig {
    /// Directory containing custom polyfills
    pub custom_dir: Option<PathBuf>,

    /// Default polyfills to include
    #[serde(default)]
    pub default_includes: Vec<String>,

    /// Polyfills to exclude by default
    #[serde(default)]
    pub default_excludes: Vec<String>,

    /// Custom polyfill mappings
    #[serde(default)]
    pub mappings: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundleConfig {
    /// Default bundle strategy
    #[serde(default)]
    pub strategy: BundleStrategy,

    /// Maximum bundle size in bytes
    #[serde(default = "default_max_size")]
    pub max_size: usize,

    /// Dependencies to always exclude
    #[serde(default)]
    pub exclude_dependencies: Vec<String>,

    /// Dependencies to always inline
    #[serde(default)]
    pub force_inline: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheConfig {
    /// Cache directory
    #[serde(default = "default_cache_dir")]
    pub directory: PathBuf,

    /// Cache TTL in seconds
    #[serde(default = "default_cache_ttl")]
    pub ttl: u64,

    /// Enable cache
    #[serde(default = "default_cache_enabled")]
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateConfig {
    /// Custom template directory
    pub directory: Option<PathBuf>,

    /// Template overrides
    #[serde(default)]
    pub overrides: HashMap<String, String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            npm: NpmConfig::default(),
            output: OutputConfig::default(),
            polyfills: PolyfillConfig::default(),
            bundle: BundleConfig::default(),
            cache: CacheConfig::default(),
            templates: TemplateConfig::default(),
        }
    }
}

impl Default for NpmConfig {
    fn default() -> Self {
        Self {
            registry: default_npm_registry(),
            timeout: default_timeout(),
            user_agent: default_user_agent(),
            auth_token: None,
        }
    }
}

impl Default for OutputConfig {
    fn default() -> Self {
        Self {
            directory: default_output_dir(),
            naming_pattern: default_naming_pattern(),
            minify: false,
            target: EsTarget::Es5,
            source_maps: false,
        }
    }
}

impl Default for PolyfillConfig {
    fn default() -> Self {
        Self {
            custom_dir: None,
            default_includes: vec![
                "buffer".to_string(),
                "crypto".to_string(),
                "events".to_string(),
            ],
            default_excludes: vec![
                "fs".to_string(),
                "child_process".to_string(),
            ],
            mappings: HashMap::new(),
        }
    }
}

impl Default for BundleConfig {
    fn default() -> Self {
        Self {
            strategy: BundleStrategy::Inline,
            max_size: default_max_size(),
            exclude_dependencies: vec![
                "fsevents".to_string(),
                "node-gyp".to_string(),
            ],
            force_inline: Vec::new(),
        }
    }
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            directory: default_cache_dir(),
            ttl: default_cache_ttl(),
            enabled: default_cache_enabled(),
        }
    }
}

impl Default for TemplateConfig {
    fn default() -> Self {
        Self {
            directory: None,
            overrides: HashMap::new(),
        }
    }
}

impl Config {
    /// Load configuration from file or use defaults
    pub fn load(path: Option<&Path>) -> Result<Self> {
        let config_path = match path {
            Some(p) => p.to_path_buf(),
            None => Self::find_config_file()?,
        };

        if config_path.exists() {
            let content = std::fs::read_to_string(&config_path)
                .with_context(|| format!("Failed to read config file: {}", config_path.display()))?;

            let config: Config = toml::from_str(&content)
                .with_context(|| format!("Failed to parse config file: {}", config_path.display()))?;

            Ok(config)
        } else {
            Ok(Config::default())
        }
    }

    /// Initialize configuration file
    pub fn init(output_dir: &Path) -> Result<()> {
        std::fs::create_dir_all(output_dir)
            .with_context(|| format!("Failed to create directory: {}", output_dir.display()))?;

        let config_path = output_dir.join("pakto.toml");
        let config = Config::default();

        let content = toml::to_string_pretty(&config)
            .context("Failed to serialize default configuration")?;

        std::fs::write(&config_path, content)
            .with_context(|| format!("Failed to write config file: {}", config_path.display()))?;

        Ok(())
    }

    /// Find configuration file in standard locations
    fn find_config_file() -> Result<PathBuf> {
        let current_dir = std::env::current_dir()
            .context("Failed to get current directory")?;

        // Look for pakto.toml in current directory and parents
        let mut dir = current_dir.as_path();
        loop {
            let config_path = dir.join("pakto.toml");
            if config_path.exists() {
                return Ok(config_path);
            }

            match dir.parent() {
                Some(parent) => dir = parent,
                None => break,
            }
        }

        // Look in config directory
        if let Some(config_dir) = dirs::config_dir() {
            let config_path = config_dir.join("pakto").join("config.toml");
            if config_path.exists() {
                return Ok(config_path);
            }
        }

        // Return default path (may not exist)
        Ok(current_dir.join("pakto.toml"))
    }
}

// Default value functions
fn default_npm_registry() -> String {
    "https://registry.npmjs.org".to_string()
}

fn default_timeout() -> u64 {
    30
}

fn default_user_agent() -> String {
    format!("pakto/{}", env!("CARGO_PKG_VERSION"))
}

fn default_output_dir() -> PathBuf {
    PathBuf::from("./dist")
}

fn default_naming_pattern() -> String {
    "{name}-outsystems.js".to_string()
}

fn default_max_size() -> usize {
    5 * 1024 * 1024 // 5MB
}

fn default_cache_dir() -> PathBuf {
    dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from(".cache"))
        .join("pakto")
}

fn default_cache_ttl() -> u64 {
    24 * 60 * 60 // 24 hours
}

fn default_cache_enabled() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.npm.registry, "https://registry.npmjs.org");
        assert_eq!(config.npm.timeout, 30);
    }

    #[test]
    fn test_config_serialization() {
        let config = Config::default();
        let serialized = toml::to_string(&config).unwrap();
        let deserialized: Config = toml::from_str(&serialized).unwrap();

        assert_eq!(config.npm.registry, deserialized.npm.registry);
    }

    #[test]
    fn test_config_init() {
        let temp_dir = TempDir::new().unwrap();
        Config::init(temp_dir.path()).unwrap();

        let config_path = temp_dir.path().join("pakto.toml");
        assert!(config_path.exists());
    }
}