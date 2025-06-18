use std::path::PathBuf;
use thiserror::Error;

/// Main error type for Pakto
#[derive(Error, Debug)]
pub enum PaktoError {
    #[error("Package not found: {package}")]
    PackageNotFound {
        package: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    #[error("Package version not found: {package}@{version}")]
    VersionNotFound {
        package: String,
        version: String
    },

    #[error("Invalid package name: {package}")]
    InvalidPackageName { package: String },

    #[error("Network error while fetching package: {package}")]
    NetworkError {
        package: String,
        #[source]
        source: reqwest::Error,
    },

    #[error("Parse error in file {file}: {message}")]
    ParseError {
        file: PathBuf,
        message: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    #[error("Transformation error: {message}")]
    TransformError {
        message: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    #[error("Incompatible Node.js API detected: {api}")]
    IncompatibleApi {
        api: String,
        suggestion: Option<String>,
        location: Option<CodeLocation>,
    },

    #[error("Circular dependency detected: {cycle:?}")]
    CircularDependency {
        cycle: Vec<String>
    },

    #[error("Bundle size too large: {size} bytes (max: {max} bytes)")]
    BundleTooLarge {
        size: usize,
        max: usize
    },

    #[error("Missing required dependency: {dependency}")]
    MissingDependency {
        dependency: String,
        required_by: String,
    },

    #[error("Template error: {message}")]
    TemplateError {
        message: String,
        template: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    #[error("File system error: {message}")]
    FileSystemError {
        message: String,
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("Configuration error: {message}")]
    ConfigError {
        message: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    #[error("Cache error: {message}")]
    CacheError {
        message: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    #[error("Minification error: {message}")]
    MinificationError {
        message: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    #[error("Multiple errors occurred")]
    Multiple {
        errors: Vec<PaktoError>
    },
}

/// Represents a location in source code
#[derive(Debug, Clone)]
pub struct CodeLocation {
    pub file: PathBuf,
    pub line: Option<usize>,
    pub column: Option<usize>,
}

/// Compatibility issues found during analysis
#[derive(Debug, Clone)]
pub struct CompatibilityIssue {
    pub level: IssueLevel,
    pub message: String,
    pub location: Option<CodeLocation>,
    pub suggestion: Option<String>,
    pub api: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum IssueLevel {
    Error,
    Warning,
    Info,
}

/// Result type alias for convenience
pub type Result<T> = std::result::Result<T, PaktoError>;

/// Warning that doesn't stop conversion but should be reported
#[derive(Debug, Clone)]
pub struct Warning {
    pub message: String,
    pub location: Option<CodeLocation>,
    pub category: WarningCategory,
}

#[derive(Debug, Clone)]
pub enum WarningCategory {
    Performance,
    Compatibility,
    Security,
    Deprecated,
    Size,
}

impl PaktoError {
    pub fn package_not_found(package: impl Into<String>) -> Self {
        Self::PackageNotFound {
            package: package.into(),
            source: None,
        }
    }

    pub fn parse_error(file: impl Into<PathBuf>, message: impl Into<String>) -> Self {
        Self::ParseError {
            file: file.into(),
            message: message.into(),
            source: None,
        }
    }

    pub fn incompatible_api(api: impl Into<String>) -> Self {
        Self::IncompatibleApi {
            api: api.into(),
            suggestion: None,
            location: None,
        }
    }

    pub fn incompatible_api_with_suggestion(
        api: impl Into<String>,
        suggestion: impl Into<String>
    ) -> Self {
        Self::IncompatibleApi {
            api: api.into(),
            suggestion: Some(suggestion.into()),
            location: None,
        }
    }

    pub fn file_system_error(
        message: impl Into<String>,
        path: impl Into<PathBuf>,
        source: std::io::Error
    ) -> Self {
        Self::FileSystemError {
            message: message.into(),
            path: path.into(),
            source,
        }
    }

    /// Check if error is recoverable
    pub fn is_recoverable(&self) -> bool {
        matches!(
            self,
            Self::NetworkError { .. } |
            Self::CacheError { .. } |
            Self::MinificationError { .. }
        )
    }

    /// Get error category for metrics/reporting
    pub fn category(&self) -> ErrorCategory {
        match self {
            Self::PackageNotFound { .. } |
            Self::VersionNotFound { .. } |
            Self::InvalidPackageName { .. } => ErrorCategory::Package,

            Self::NetworkError { .. } => ErrorCategory::Network,

            Self::ParseError { .. } |
            Self::TransformError { .. } => ErrorCategory::Parsing,

            Self::IncompatibleApi { .. } |
            Self::CircularDependency { .. } |
            Self::MissingDependency { .. } => ErrorCategory::Compatibility,

            Self::BundleTooLarge { .. } => ErrorCategory::Bundle,

            Self::TemplateError { .. } => ErrorCategory::Template,

            Self::FileSystemError { .. } => ErrorCategory::FileSystem,

            Self::ConfigError { .. } => ErrorCategory::Configuration,

            Self::CacheError { .. } => ErrorCategory::Cache,

            Self::MinificationError { .. } => ErrorCategory::Minification,

            Self::Multiple { .. } => ErrorCategory::Multiple,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ErrorCategory {
    Package,
    Network,
    Parsing,
    Compatibility,
    Bundle,
    Template,
    FileSystem,
    Configuration,
    Cache,
    Minification,
    Multiple,
}

impl From<reqwest::Error> for PaktoError {
    fn from(err: reqwest::Error) -> Self {
        Self::NetworkError {
            package: "unknown".to_string(),
            source: err,
        }
    }
}

impl From<std::io::Error> for PaktoError {
    fn from(err: std::io::Error) -> Self {
        Self::FileSystemError {
            message: err.to_string(),
            path: PathBuf::new(),
            source: err,
        }
    }
}

impl From<serde_json::Error> for PaktoError {
    fn from(err: serde_json::Error) -> Self {
        Self::ParseError {
            file: PathBuf::from("unknown"),
            message: err.to_string(),
            source: Some(Box::new(err)),
        }
    }
}

impl CodeLocation {
    pub fn new(file: impl Into<PathBuf>) -> Self {
        Self {
            file: file.into(),
            line: None,
            column: None,
        }
    }

    pub fn with_line(mut self, line: usize) -> Self {
        self.line = Some(line);
        self
    }

    pub fn with_column(mut self, column: usize) -> Self {
        self.column = Some(column);
        self
    }
}

impl std::fmt::Display for CodeLocation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.file.display())?;
        if let Some(line) = self.line {
            write!(f, ":{}", line)?;
            if let Some(column) = self.column {
                write!(f, ":{}", column)?;
            }
        }
        Ok(())
    }
}

impl CompatibilityIssue {
    pub fn error(message: impl Into<String>) -> Self {
        Self {
            level: IssueLevel::Error,
            message: message.into(),
            location: None,
            suggestion: None,
            api: None,
        }
    }

    pub fn warning(message: impl Into<String>) -> Self {
        Self {
            level: IssueLevel::Warning,
            message: message.into(),
            location: None,
            suggestion: None,
            api: None,
        }
    }

    pub fn with_suggestion(mut self, suggestion: impl Into<String>) -> Self {
        self.suggestion = Some(suggestion.into());
        self
    }

    pub fn with_api(mut self, api: impl Into<String>) -> Self {
        self.api = Some(api.into());
        self
    }

    pub fn with_location(mut self, location: CodeLocation) -> Self {
        self.location = Some(location);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_creation() {
        let err = PaktoError::package_not_found("test-package");
        assert!(matches!(err, PaktoError::PackageNotFound { .. }));
    }

    #[test]
    fn test_error_categories() {
        let err = PaktoError::package_not_found("test");
        assert_eq!(err.category(), ErrorCategory::Package);

        let err = PaktoError::incompatible_api("fs.readFile");
        assert_eq!(err.category(), ErrorCategory::Compatibility);
    }

    #[test]
    fn test_code_location() {
        let loc = CodeLocation::new("test.js")
            .with_line(10)
            .with_column(5);

        assert_eq!(loc.to_string(), "test.js:10:5");
    }

    #[test]
    fn test_compatibility_issue() {
        let issue = CompatibilityIssue::error("Invalid API usage")
            .with_suggestion("Use Web Crypto API instead")
            .with_api("crypto.createHash");

        assert_eq!(issue.level, IssueLevel::Error);
        assert!(issue.suggestion.is_some());
        assert!(issue.api.is_some());
    }
}