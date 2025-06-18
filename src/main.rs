use anyhow::Result;
use clap::Parser;
use tracing::{info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod cli;
mod config;
mod converter;
mod analyzer;
mod transformer;
mod bundler;
mod polyfills;
mod npm;
mod output;
mod errors;

use cli::{Cli, Commands};
use config::Config;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    init_tracing()?;

    // Parse CLI arguments
    let cli = Cli::parse();

    // Load configuration
    let config = Config::load(cli.config.as_deref())?;

    info!("Starting Pakto v{}", env!("CARGO_PKG_VERSION"));

    // Handle commands
    match cli.command {
        Commands::Convert {
            package,
            output,
            name,
            namespace,
            minify,
            target,
            include_polyfills,
            exclude_dependencies,
            strategy,
            dry_run
        } => {
            let converter = converter::Converter::new(config).await?;

            if dry_run {
                info!("Dry run mode - analyzing package without conversion");
                let analysis = converter.analyze(&package).await?;
                println!("{}", serde_json::to_string_pretty(&analysis)?);
                return Ok(());
            }

            let options = converter::ConvertOptions {
                output_path: output,
                name,
                namespace,
                minify,
                target_es_version: target,
                include_polyfills,
                exclude_dependencies,
                bundle_strategy: strategy,
            };

            match converter.convert(&package, options).await {
                Ok(result) => {
                    info!("Conversion completed successfully");
                    info!("Output: {}", result.output_path.display());
                    info!("Size: {} bytes", result.size);

                    if !result.warnings.is_empty() {
                        warn!("Warnings during conversion:");
                        for warning in &result.warnings {
                            warn!("  - {}", warning);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("❌ Conversion failed: {}", e);
                    std::process::exit(1);
                }
            }
        }

        Commands::Analyze { package } => {
            let converter = converter::Converter::new(config).await?;
            match converter.analyze(&package).await {
                Ok(analysis) => {
                    println!("{}", serde_json::to_string_pretty(&analysis)?);
                }
                Err(e) => {
                    eprintln!("❌ Analysis failed: {}", e);
                    std::process::exit(1);
                }
            }
        }

        Commands::Init { output_dir } => {
            config::Config::init(&output_dir)?;
            info!("Initialized Pakto configuration in {}", output_dir.display());
        }

        Commands::Completions { shell } => {
            cli::generate_completions(shell);
        }
    }

    Ok(())
}

fn init_tracing() -> Result<()> {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("pakto=info"));

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::fmt::layer()
                .with_target(false)
                .with_thread_ids(false)
                .with_file(false)
                .with_line_number(false)
        )
        .with(filter)
        .init();

    Ok(())
}