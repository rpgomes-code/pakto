use std::path::PathBuf;
use clap::{Parser, Subcommand, ValueEnum};
use clap_complete::{generate, Shell};

/// Convert NPM packages to OutSystems-compatible JavaScript bundles
#[derive(Parser)]
#[command(
    name = "pakto",
    version,
    about = "Convert NPM packages to OutSystems-compatible JavaScript bundles",
    long_about = "Pakto converts NPM packages into single-file JavaScript bundles that are compatible with OutSystems platform. It handles module system conversion, polyfills for Node.js APIs, and generates optimized browser-ready code."
)]
pub struct Cli {
    /// Configuration file path
    #[arg(short, long, global = true)]
    pub config: Option<PathBuf>,

    /// Enable verbose logging
    #[arg(short, long, global = true)]
    pub verbose: bool,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Convert an NPM package to OutSystems-compatible JavaScript
    Convert {
        /// NPM package name or local path
        #[arg(value_name = "PACKAGE")]
        package: String,

        /// Output file path
        #[arg(short, long, value_name = "FILE")]
        output: Option<PathBuf>,

        /// Custom name for the generated module
        #[arg(short, long)]
        name: Option<String>,

        /// Namespace for the global variable
        #[arg(long)]
        namespace: Option<String>,

        /// Minify the output
        #[arg(short = 'M', long)]
        minify: bool,

        /// Target ECMAScript version
        #[arg(short, long, default_value = "es5")]
        target: EsTarget,

        /// Include specific polyfills (comma-separated)
        #[arg(long, value_delimiter = ',')]
        include_polyfills: Vec<String>,

        /// Exclude specific dependencies (comma-separated)
        #[arg(long, value_delimiter = ',')]
        exclude_dependencies: Vec<String>,

        /// Bundle strategy
        #[arg(short, long, default_value = "inline")]
        strategy: BundleStrategy,

        /// Perform dry run (analyze only, don't convert)
        #[arg(long)]
        dry_run: bool,
    },

    /// Analyze package compatibility with OutSystems
    Analyze {
        /// NPM package name or local path
        #[arg(value_name = "PACKAGE")]
        package: String,
    },

    /// Initialize Pakto configuration
    Init {
        /// Output directory for configuration
        #[arg(short, long, default_value = ".")]
        output_dir: PathBuf,
    },

    /// Generate shell completions
    Completions {
        /// Shell to generate completions for
        #[arg(value_enum)]
        shell: Shell,
    },
}

#[derive(Clone, ValueEnum, Debug, PartialEq)]
pub enum EsTarget {
    #[value(name = "es5")]
    Es5,
    #[value(name = "es2015")]
    Es2015,
    #[value(name = "es2017")]
    Es2017,
    #[value(name = "es2018")]
    Es2018,
    #[value(name = "es2020")]
    Es2020,
    #[value(name = "esnext")]
    EsNext,
}

#[derive(Clone, ValueEnum, Debug, PartialEq)]
pub enum BundleStrategy {
    /// Include all dependencies inline
    #[value(name = "inline")]
    Inline,

    /// Only include used functions (tree-shaking)
    #[value(name = "selective")]
    Selective,

    /// Assume external CDN libraries
    #[value(name = "external")]
    External,

    /// Mix of inline and external
    #[value(name = "hybrid")]
    Hybrid,
}

impl Default for EsTarget {
    fn default() -> Self {
        Self::Es5
    }
}

impl Default for BundleStrategy {
    fn default() -> Self {
        Self::Inline
    }
}

pub fn generate_completions(shell: Shell) {
    let mut cmd = Cli::command();
    let bin_name = cmd.get_name().to_string();
    generate(shell, &mut cmd, bin_name, &mut std::io::stdout());
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn verify_cli() {
        Cli::command().debug_assert()
    }

    #[test]
    fn test_default_values() {
        assert_eq!(EsTarget::default(), EsTarget::Es5);
        assert_eq!(BundleStrategy::default(), BundleStrategy::Inline);
    }
}