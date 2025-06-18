use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use anyhow::{Context, Result};
use reqwest::header::{HeaderMap, HeaderValue, USER_AGENT, AUTHORIZATION};
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};
use url::Url;

use crate::config::NpmConfig;
use crate::converter::{PackageData, PackageInfo};
use crate::errors::{PaktoError, Result as PaktoResult};

/// NPM registry client for fetching package information and downloads
pub struct NpmClient {
    config: NpmConfig,
    client: reqwest::Client,
    cache_dir: PathBuf,
}

/// NPM package metadata from registry
#[derive(Debug, Deserialize)]
pub struct NpmPackageMetadata {
    pub name: String,
    pub description: Option<String>,
    #[serde(rename = "dist-tags")]
    pub dist_tags: HashMap<String, String>,
    pub versions: HashMap<String, NpmVersionInfo>,
    pub keywords: Option<Vec<String>>,
    pub license: Option<serde_json::Value>,
    pub repository: Option<serde_json::Value>,
    pub homepage: Option<String>,
}

/// Version-specific package information
#[derive(Debug, Deserialize)]
pub struct NpmVersionInfo {
    pub name: String,
    pub version: String,
    pub description: Option<String>,
    pub main: Option<String>,
    pub browser: Option<serde_json::Value>,
    pub module: Option<String>,
    pub dependencies: Option<HashMap<String, String>>,
    #[serde(rename = "devDependencies")]
    pub dev_dependencies: Option<HashMap<String, String>>,
    #[serde(rename = "peerDependencies")]
    pub peer_dependencies: Option<HashMap<String, String>>,
    pub keywords: Option<Vec<String>>,
    pub license: Option<serde_json::Value>,
    pub dist: NpmDistInfo,
    pub scripts: Option<HashMap<String, String>>,
}

/// Distribution/download information
#[derive(Debug, Deserialize)]
pub struct NpmDistInfo {
    pub tarball: String,
    pub shasum: String,
    pub integrity: Option<String>,
    #[serde(rename = "unpackedSize")]
    pub unpacked_size: Option<u64>,
}

/// Cached package information
#[derive(Debug, Serialize, Deserialize)]
struct CachedPackage {
    metadata: NpmPackageMetadata,
    cached_at: u64,
    ttl: u64,
}

impl NpmClient {
    /// Create a new NPM client
    pub async fn new(config: &NpmConfig) -> PaktoResult<Self> {
        let mut headers = HeaderMap::new();
        headers.insert(USER_AGENT, HeaderValue::from_str(&config.user_agent)?);

        if let Some(ref token) = config.auth_token {
            let auth_value = format!("Bearer {}", token);
            headers.insert(AUTHORIZATION, HeaderValue::from_str(&auth_value)?);
        }

        let client = reqwest::Client::builder()
            .default_headers(headers)
            .timeout(std::time::Duration::from_secs(config.timeout))
            .build()
            .context("Failed to create HTTP client")?;

        let cache_dir = dirs::cache_dir()
            .unwrap_or_else(|| PathBuf::from(".cache"))
            .join("pakto")
            .join("npm");

        std::fs::create_dir_all(&cache_dir)
            .context("Failed to create cache directory")?;

        Ok(Self {
            config: config.clone(),
            client,
            cache_dir,
        })
    }

    /// Get package information from NPM registry
    pub async fn get_package_info(&self, package: &str) -> PaktoResult<PackageInfo> {
        info!("Fetching package info for: {}", package);

        let package_name = self.parse_package_name(package)?;
        let metadata = self.get_package_metadata(&package_name.name).await?;

        let version = package_name.version
            .or_else(|| metadata.dist_tags.get("latest").cloned())
            .ok_or_else(|| PaktoError::VersionNotFound {
                package: package_name.name.clone(),
                version: "latest".to_string(),
            })?;

        let version_info = metadata.versions.get(&version)
            .ok_or_else(|| PaktoError::VersionNotFound {
                package: package_name.name.clone(),
                version: version.clone(),
            })?;

        // Determine entry points
        let mut entry_points = Vec::new();

        if let Some(ref main) = version_info.main {
            entry_points.push(main.clone());
        }

        if let Some(ref module) = version_info.module {
            entry_points.push(module.clone());
        }

        // Handle browser field
        if let Some(browser) = &version_info.browser {
            match browser {
                serde_json::Value::String(path) => {
                    entry_points.push(path.clone());
                }
                serde_json::Value::Object(map) => {
                    for (key, value) in map {
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

        // Default entry point if none found
        if entry_points.is_empty() {
            entry_points.push("index.js".to_string());
        }

        let license_string = match &version_info.license {
            Some(serde_json::Value::String(s)) => Some(s.clone()),
            Some(serde_json::Value::Object(obj)) => {
                obj.get("type")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
            }
            _ => None,
        };

        Ok(PackageInfo {
            name: version_info.name.clone(),
            version: version_info.version.clone(),
            description: version_info.description.clone(),
            main: version_info.main.clone(),
            entry_points,
            dependencies: version_info.dependencies.clone().unwrap_or_default(),
            dev_dependencies: version_info.dev_dependencies.clone().unwrap_or_default(),
            keywords: version_info.keywords.clone().unwrap_or_default(),
            license: license_string,
        })
    }

    /// Download package and extract files
    pub async fn download_package(&self, package: &str) -> PaktoResult<PackageData> {
        info!("Downloading package: {}", package);

        let package_name = self.parse_package_name(package)?;
        let metadata = self.get_package_metadata(&package_name.name).await?;

        let version = package_name.version
            .or_else(|| metadata.dist_tags.get("latest").cloned())
            .ok_or_else(|| PaktoError::VersionNotFound {
                package: package_name.name.clone(),
                version: "latest".to_string(),
            })?;

        let version_info = metadata.versions.get(&version)
            .ok_or_else(|| PaktoError::VersionNotFound {
                package: package_name.name.clone(),
                version: version.clone(),
            })?;

        // Check cache first
        let cache_key = format!("{}@{}", package_name.name, version);
        if let Ok(cached_data) = self.get_cached_package_data(&cache_key).await {
            debug!("Using cached package data for {}", cache_key);
            return Ok(cached_data);
        }

        // Download tarball
        debug!("Downloading tarball: {}", version_info.dist.tarball);
        let response = self.client
            .get(&version_info.dist.tarball)
            .send()
            .await
            .context("Failed to download package tarball")?;

        if !response.status().is_success() {
            return Err(PaktoError::NetworkError {
                package: package_name.name,
                source: reqwest::Error::from(response.error_for_status().unwrap_err()),
            });
        }

        let tarball_bytes = response.bytes().await
            .context("Failed to read tarball bytes")?;

        // Extract tarball
        let extracted_files = self.extract_tarball(&tarball_bytes).await?;

        // Create package.json content
        let package_json = serde_json::to_value(version_info)
            .context("Failed to serialize package.json")?;

        let package_data = PackageData {
            total_size: tarball_bytes.len(),
            files: extracted_files,
            package_json,
        };

        // Cache the result
        self.cache_package_data(&cache_key, &package_data).await?;

        Ok(package_data)
    }

    /// Parse package name and version
    fn parse_package_name(&self, package: &str) -> PaktoResult<ParsedPackageName> {
        if package.is_empty() {
            return Err(PaktoError::InvalidPackageName {
                package: package.to_string(),
            });
        }

        // Handle scoped packages (@scope/name@version)
        if package.starts_with('@') {
            let parts: Vec<&str> = package.splitn(2, '/').collect();
            if parts.len() != 2 {
                return Err(PaktoError::InvalidPackageName {
                    package: package.to_string(),
                });
            }

            let scope = parts[0];
            let name_version: Vec<&str> = parts[1].splitn(2, '@').collect();

            let name = format!("{}/{}", scope, name_version[0]);
            let version = if name_version.len() > 1 {
                Some(name_version[1].to_string())
            } else {
                None
            };

            Ok(ParsedPackageName { name, version })
        } else {
            // Handle regular packages (name@version)
            let parts: Vec<&str> = package.splitn(2, '@').collect();
            let name = parts[0].to_string();
            let version = if parts.len() > 1 {
                Some(parts[1].to_string())
            } else {
                None
            };

            Ok(ParsedPackageName { name, version })
        }
    }

    /// Get package metadata from registry
    async fn get_package_metadata(&self, name: &str) -> PaktoResult<NpmPackageMetadata> {
        // Check cache first
        if let Ok(cached) = self.get_cached_metadata(name).await {
            debug!("Using cached metadata for {}", name);
            return Ok(cached.metadata);
        }

        let encoded_name = urlencoding::encode(name);
        let url = format!("{}/{}", self.config.registry, encoded_name);

        debug!("Fetching metadata from: {}", url);

        let response = self.client
            .get(&url)
            .send()
            .await
            .context("Failed to fetch package metadata")?;

        if response.status() == 404 {
            return Err(PaktoError::PackageNotFound {
                package: name.to_string(),
                source: None,
            });
        }

        if !response.status().is_success() {
            return Err(PaktoError::NetworkError {
                package: name.to_string(),
                source: reqwest::Error::from(response.error_for_status().unwrap_err()),
            });
        }

        let metadata: NpmPackageMetadata = response.json().await
            .context("Failed to parse package metadata JSON")?;

        // Cache the metadata
        self.cache_metadata(name, &metadata).await?;

        Ok(metadata)
    }

    /// Extract files from tarball
    async fn extract_tarball(&self, tarball_bytes: &[u8]) -> PaktoResult<HashMap<PathBuf, String>> {
        use flate2::read::GzDecoder;
        use tar::Archive;
        use std::io::Read;

        let decoder = GzDecoder::new(tarball_bytes);
        let mut archive = Archive::new(decoder);

        let mut files = HashMap::new();

        for entry in archive.entries().context("Failed to read tarball entries")? {
            let mut entry = entry.context("Failed to read tarball entry")?;
            let path = entry.path().context("Failed to get entry path")?;

            // Skip directories and non-text files
            if entry.header().entry_type().is_dir() {
                continue;
            }

            let path_str = path.to_string_lossy();

            // Skip common non-source files
            if self.should_skip_file(&path_str) {
                continue;
            }

            // Read file content
            let mut content = String::new();
            if entry.read_to_string(&mut content).is_ok() {
                // Remove package/ prefix that npm adds
                let clean_path = path_str.strip_prefix("package/")
                    .unwrap_or(&path_str);

                files.insert(PathBuf::from(clean_path), content);
            }
        }

        Ok(files)
    }

    /// Check if file should be skipped during extraction
    fn should_skip_file(&self, path: &str) -> bool {
        let path_lower = path.to_lowercase();

        // Skip common non-source files
        path_lower.ends_with(".md") ||
            path_lower.ends_with(".txt") ||
            path_lower.ends_with(".yml") ||
            path_lower.ends_with(".yaml") ||
            path_lower.contains("test") ||
            path_lower.contains("spec") ||
            path_lower.contains("example") ||
            path_lower.contains("demo") ||
            path_lower.contains(".git") ||
            path_lower.starts_with("node_modules/") ||
            path_lower == "license" ||
            path_lower == "readme" ||
            path_lower == "changelog" ||
            path_lower == "contributing"
    }

    /// Get cached metadata
    async fn get_cached_metadata(&self, name: &str) -> Result<CachedPackage> {
        let cache_file = self.cache_dir.join(format!("{}.json",
                                                     name.replace('/', "_").replace('@', "_")));

        if !cache_file.exists() {
            return Err(anyhow::anyhow!("Cache file not found"));
        }

        let content = tokio::fs::read_to_string(&cache_file).await?;
        let cached: CachedPackage = serde_json::from_str(&content)?;

        let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
        if now > cached.cached_at + cached.ttl {
            return Err(anyhow::anyhow!("Cache expired"));
        }

        Ok(cached)
    }

    /// Cache metadata
    async fn cache_metadata(&self, name: &str, metadata: &NpmPackageMetadata) -> Result<()> {
        let cache_file = self.cache_dir.join(format!("{}.json",
                                                     name.replace('/', "_").replace('@', "_")));

        let cached = CachedPackage {
            metadata: metadata.clone(),
            cached_at: SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),
            ttl: 3600, // 1 hour
        };

        let content = serde_json::to_string_pretty(&cached)?;
        tokio::fs::write(&cache_file, content).await?;

        Ok(())
    }

    /// Get cached package data (placeholder for now)
    async fn get_cached_package_data(&self, _cache_key: &str) -> Result<PackageData> {
        // TODO: Implement package data caching
        Err(anyhow::anyhow!("Package data caching not implemented"))
    }

    /// Cache package data (placeholder for now)
    async fn cache_package_data(&self, _cache_key: &str, _data: &PackageData) -> Result<()> {
        // TODO: Implement package data caching
        Ok(())
    }
}

#[derive(Debug)]
struct ParsedPackageName {
    name: String,
    version: Option<String>,
}

impl Clone for NpmPackageMetadata {
    fn clone(&self) -> Self {
        Self {
            name: self.name.clone(),
            description: self.description.clone(),
            dist_tags: self.dist_tags.clone(),
            versions: self.versions.clone(),
            keywords: self.keywords.clone(),
            license: self.license.clone(),
            repository: self.repository.clone(),
            homepage: self.homepage.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::NpmConfig;

    #[tokio::test]
    async fn test_npm_client_creation() {
        let config = NpmConfig::default();
        let client = NpmClient::new(&config).await;
        assert!(client.is_ok());
    }

    #[test]
    fn test_package_name_parsing() {
        let config = NpmConfig::default();
        let client = NpmClient {
            config,
            client: reqwest::Client::new(),
            cache_dir: PathBuf::new(),
        };

        // Regular package
        let parsed = client.parse_package_name("lodash").unwrap();
        assert_eq!(parsed.name, "lodash");
        assert_eq!(parsed.version, None);

        // Package with version
        let parsed = client.parse_package_name("lodash@4.17.21").unwrap();
        assert_eq!(parsed.name, "lodash");
        assert_eq!(parsed.version, Some("4.17.21".to_string()));

        // Scoped package
        let parsed = client.parse_package_name("@types/node").unwrap();
        assert_eq!(parsed.name, "@types/node");
        assert_eq!(parsed.version, None);

        // Scoped package with version
        let parsed = client.parse_package_name("@types/node@18.0.0").unwrap();
        assert_eq!(parsed.name, "@types/node");
        assert_eq!(parsed.version, Some("18.0.0".to_string()));
    }

    #[test]
    fn test_should_skip_file() {
        let config = NpmConfig::default();
        let client = NpmClient {
            config,
            client: reqwest::Client::new(),
            cache_dir: PathBuf::new(),
        };

        assert!(client.should_skip_file("README.md"));
        assert!(client.should_skip_file("test/index.js"));
        assert!(client.should_skip_file("package/test/spec.js"));
        assert!(!client.should_skip_file("src/index.js"));
        assert!(!client.should_skip_file("lib/main.js"));
    }
}