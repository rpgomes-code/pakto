# Contributing to Pakto

Thank you for your interest in contributing to Pakto! This document provides guidelines and information for contributors.

## Code of Conduct

This project adheres to the Rust Code of Conduct. By participating, you are expected to uphold this code.

## Getting Started

1. Fork the repository
2. Clone your fork: `git clone https://github.com/yourusername/pakto.git`
3. Create a new branch: `git checkout -b feature/your-feature-name`
4. Make your changes
5. Run tests: `cargo test`
6. Run clippy: `cargo clippy`
7. Format code: `cargo fmt`
8. Commit your changes: `git commit -m "Add your feature"`
9. Push to your fork: `git push origin feature/your-feature-name`
10. Create a Pull Request

## Development Setup

### Prerequisites
- Rust 1.70 or later
- Node.js (for integration tests)
- Git

### Building
```bash
cargo build
```

### Testing
```bash
# Unit tests
cargo test

# Integration tests
cargo test --test integration

# All tests with coverage
cargo test --all-features
```

### Code Quality
```bash
# Check formatting
cargo fmt --check

# Run clippy
cargo clippy --all-targets --all-features -- -D warnings

# Check for security vulnerabilities
cargo audit
```

## Pull Request Guidelines

- Keep PRs focused on a single feature or fix
- Include tests for new functionality
- Update documentation as needed
- Follow the existing code style
- Write clear commit messages
- Rebase on main before submitting

## Testing Guidelines

- Write unit tests for all new functions
- Include integration tests for major features
- Test error conditions and edge cases
- Use descriptive test names
- Mock external dependencies when appropriate

## Documentation

- Update README.md for user-facing changes
- Add doc comments for public APIs
- Include examples in documentation
- Update CHANGELOG.md

## Performance Considerations

- Profile performance-critical code
- Consider memory usage implications
- Optimize for common use cases
- Document performance characteristics

Thank you for contributing!