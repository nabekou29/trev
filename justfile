# Default recipe
default: build

# Build the binary
build:
    cargo build --all-features

# Run tests
test:
    cargo test --all-features

# Run lints
lint:
    cargo clippy --all-targets --all-features

# Run lints and fix them
lint-fix:
    cargo clippy --all-targets --all-features --fix --allow-dirty --allow-staged

# Format the code
format:
    cargo fmt --all

# Format check
format-check:
    cargo fmt --all -- --check

# Run all benchmarks
bench:
    cargo bench

# Regenerate config JSON Schema
schema:
    cargo run --features dev -- schema > config.schema.json

# Regenerate keybinding docs
docs:
    cargo run --features dev -- docs > docs/keybindings.md

# Run tests with coverage
coverage:
    cargo llvm-cov --all-features

# Run tests with coverage and generate HTML report
coverage-html:
    cargo llvm-cov --all-features --html --open

# Run tests with coverage for CI (JSON output)
coverage-ci:
    cargo llvm-cov --all-features --codecov --output-path codecov.json

# Install the binary locally
install:
    cargo install --path . --force

# Preview release (dry-run)
release level="patch":
    cargo release {{ level }}

# Execute release
release-execute level="patch":
    cargo release --execute {{ level }}

# Preview CHANGELOG generation
changelog:
    git-cliff --unreleased

# Generate demo GIF and video
demo:
    ./demo/setup.sh
    vhs demo/demo.tape
