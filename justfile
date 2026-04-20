# Default recipe: list available commands
default:
    @just --list

# Format all code (Rust + Nix)
fmt:
    treefmt

# Check formatting (Rust + Nix)
fmt-check:
    treefmt --fail-on-change --no-cache

# Run clippy lints
lint:
    cargo clippy --all-targets --all-features -- -D warnings

# Run all tests
test:
    cargo test --all-features

# Run tests with verbose output
test-verbose:
    cargo test --all-features -- --nocapture

# Build release
build:
    cargo build --release --all-features

demo:
    cargo run -p promptui-examples --bin chat

# Generate documentation
doc *args='':
    cargo doc --no-deps --all-features {{args}}

# Run CI checks locally
ci: fmt-check lint test doc build
    @echo "All CI checks passed!"

# Watch for changes and run tests
watch:
    cargo watch -x test

# Clean build artifacts
clean:
    cargo clean
