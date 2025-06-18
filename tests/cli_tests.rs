use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;

#[test]
fn test_cli_help() {
    let mut cmd = Command::cargo_bin("pakto").unwrap();
    cmd.arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("Convert NPM packages"));
}

#[test]
fn test_cli_version() {
    let mut cmd = Command::cargo_bin("pakto").unwrap();
    cmd.arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains(env!("CARGO_PKG_VERSION")));
}

#[test]
fn test_init_command() {
    let temp_dir = TempDir::new().unwrap();

    let mut cmd = Command::cargo_bin("pakto").unwrap();
    cmd.arg("init")
        .arg("--output-dir")
        .arg(temp_dir.path())
        .assert()
        .success();

    // Check that config file was created
    let config_path = temp_dir.path().join("pakto.toml");
    assert!(config_path.exists());
}

#[test]
fn test_analyze_nonexistent_package() {
    let mut cmd = Command::cargo_bin("pakto").unwrap();
    cmd.arg("analyze")
        .arg("nonexistent-package-12345")
        .assert()
        .failure()
        .stderr(predicate::str::contains("Package not found"));
}