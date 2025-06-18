.PHONY: build test clean install dev-setup fmt clippy audit

# Build the project
build:
	cargo build --release

# Run all tests
test:
	cargo test --all-features

# Run tests with coverage
test-coverage:
	cargo llvm-cov --all-features --workspace --lcov --output-path lcov.info

# Clean build artifacts
clean:
	cargo clean

# Install the binary
install:
	cargo install --path .

# Development setup
dev-setup:
	rustup component add rustfmt clippy
	cargo install cargo-llvm-cov cargo-audit

# Format code
fmt:
	cargo fmt --all

# Run clippy
clippy:
	cargo clippy --all-targets --all-features -- -D warnings

# Security audit
audit:
	cargo audit

# Run all quality checks
check: fmt clippy audit test

# Build Docker image
docker-build:
	docker build -t pakto:latest .

# Run in Docker
docker-run:
	docker run --rm -v $(PWD):/workspace pakto:latest --help

# Generate documentation
docs:
	cargo doc --no-deps --open

# Benchmark (when implemented)
bench:
	cargo bench

# Example conversions
example-jsotp:
	cargo run -- convert jsotp --output ./examples/jsotp-outsystems.js --name JSOTP --minify

example-lodash:
	cargo run -- convert lodash --output ./examples/lodash-outsystems.js --name Lodash --minify

# Development server (for web UI, when implemented)
dev-server:
	@echo "Development server not yet implemented"

# Release preparation
release-prep: check test-coverage
	@echo "Ready for release!"