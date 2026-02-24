# vibe-reachout — Telegram permission hook for Claude Code
# https://github.com/alexsavio/vibe-reachout

# Run the bot
run:
    cargo run -- bot

# Build the project
build:
    cargo build

# Build for release
build-release:
    cargo build --release

# Install vibe-reachout locally
install:
    cargo install --path .

# Run all tests
test:
    cargo test

# Run tests with output
test-verbose:
    cargo test -- --nocapture

# Run specific test
test-one TEST:
    cargo test {{TEST}} -- --nocapture

# Run clippy linter
lint:
    cargo clippy --all-targets --all-features -- -D warnings

# Auto-fix linting issues where possible
lint-fix:
    cargo clippy --all-targets --all-features --fix

# Format code
format:
    cargo fmt
    rumdl fmt *.md

# Check formatting without modifying files
format-check:
    cargo fmt -- --check
    rumdl check *.md

# Run all quality checks
check: format-check lint test

# Generate documentation
doc:
    cargo doc --no-deps --open

# Clean build artifacts
clean:
    cargo clean

# Full clean and rebuild
rebuild: clean build

# Generate coverage report
coverage:
    cargo tarpaulin --out Html --output-dir coverage

# Security audit
audit:
    cargo audit

# Install development tools
dev-tools:
    cargo install cargo-watch
    cargo install cargo-tarpaulin
    cargo install cargo-audit
    cargo install git-cliff
    cargo install cross

# Cross-compile for Linux aarch64
cross-linux-arm:
    cross build --release --target aarch64-unknown-linux-gnu

# Cross-compile for Linux x86_64
cross-linux-x64:
    cross build --release --target x86_64-unknown-linux-gnu

# Cross-compile all targets
cross-all: build-release cross-linux-arm cross-linux-x64

# =============================================================================
# Release Management
# =============================================================================

# Show current version
version:
    @sed -n '/^\[package\]/,/^\[/{s/^version = "\(.*\)"/\1/p;}' Cargo.toml

# Generate/update CHANGELOG.md
changelog:
    git-cliff -o CHANGELOG.md

# Preview changelog for next release (unreleased changes)
changelog-preview:
    git-cliff --unreleased --strip header

# Compute next CalVer version (YYYY.MM.MICRO)
_next-version:
    #!/usr/bin/env bash
    set -euo pipefail
    YEAR=$(date +%Y)
    MONTH=$(date +%-m)
    PREFIX="${YEAR}.${MONTH}"
    CURRENT=$(sed -n '/^\[package\]/,/^\[/{s/^version = "\(.*\)"/\1/p;}' Cargo.toml)
    if [[ "$CURRENT" == ${PREFIX}.* ]]; then
        MICRO=${CURRENT##*.}
        echo "${PREFIX}.$((MICRO + 1))"
    else
        echo "${PREFIX}.0"
    fi

# Create a new release with explicit version
# Usage: just release 2026.2.1
release VERSION:
    #!/usr/bin/env bash
    set -euo pipefail

    echo "Releasing v{{VERSION}} (CalVer)"

    # Update Cargo.toml version (only in [package] section)
    sed -i '' '/^\[package\]/,/^\[/{s/^version = ".*"/version = "{{VERSION}}"/;}' Cargo.toml

    # Ensure it compiles and passes checks
    just check

    # Update CHANGELOG.md
    git-cliff --tag "v{{VERSION}}" -o CHANGELOG.md

    # Update Cargo.lock
    cargo check

    # Commit, tag, and push
    git add Cargo.toml Cargo.lock CHANGELOG.md
    git commit -m "chore: release v{{VERSION}}"
    git tag "v{{VERSION}}"
    git push
    git push origin "v{{VERSION}}"

    echo "Released v{{VERSION}}"

# Create a new release with auto-computed CalVer version
release-next:
    #!/usr/bin/env bash
    set -euo pipefail
    VERSION=$(just _next-version)
    just release "$VERSION"

# =============================================================================
# Help
# =============================================================================

# Show help
help:
    @just --list
