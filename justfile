# List available recipes
default:
    @just --list

# Format all code
fmt:
    cargo fmt

# Check formatting without modifying files
fmt-check:
    cargo fmt --check

# Lint with clippy (warnings denied)
lint:
    cargo clippy --all-targets --all-features -- -D warnings

# Lint and auto-fix where possible
lint-fix:
    cargo clippy --fix --allow-dirty --allow-staged --all-targets --all-features

# Run the test suite
test:
    cargo test --all-features

# Report code coverage (no enforced threshold yet)
coverage:
    cargo llvm-cov --all-features

# Build the crate
build:
    cargo build --all-features

# Build documentation
doc:
    cargo doc --no-deps --all-features

# fmt-check + lint + test (CI parity)
check: fmt-check lint test
