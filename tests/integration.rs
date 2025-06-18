use pakto::{Config, Converter, ConvertOptions};
use tempfile::TempDir;
use std::path::PathBuf;

#[tokio::test]
async fn test_basic_package_conversion() {
    let config = Config::default();
    let converter = Converter::new(config).await.unwrap();

    // Use a simple, stable package for testing
    let package = "is-array"; // Small utility package

    let temp_dir = TempDir::new().unwrap();
    let output_path = temp_dir.path().join("is-array-outsystems.js");

    let options = ConvertOptions {
        output_path: Some(output_path.clone()),
        minify: false,
        ..Default::default()
    };

    // This will fail until we implement the actual conversion logic
    // but it tests the integration structure
    let result = converter.convert(package, options).await;

    // For now, we expect this to fail with a "not implemented" error
    assert!(result.is_err());
}

#[tokio::test]
async fn test_package_analysis() {
    let config = Config::default();
    let converter = Converter::new(config).await.unwrap();

    // Test analysis of a simple package
    let package = "is-array";

    let result = converter.analyze(package).await;

    // This will fail until we implement the actual analysis logic
    assert!(result.is_err());
}
