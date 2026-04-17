# Axygen Shot — project commands
# https://just.systems

set windows-shell := ["powershell.exe", "-NoLogo", "-Command"]

# List available recipes
default:
    @just --list

# Fast debug build (no optimizations, fastest compile)
build-fast:
    cargo build

# Release build optimized for minimum exe size
build:
    cargo build --release

# Run all tests (unit + integration)
test:
    cargo test

# Run clippy linter (zero warnings policy)
lint:
    cargo clippy -- -D warnings

# Check formatting without modifying files
fmt-check:
    cargo fmt -- --check

# Format code
fmt:
    cargo fmt

# Full CI check: lint → format → test
ci: lint fmt-check test

# Clean build artifacts
clean:
    cargo clean

# Build release and show binary size
size: build
    Get-Item target\release\shot.exe | Format-Table Name, @{N='Size'; E={"$([math]::Round($_.Length / 1KB)) KB"}} -AutoSize

# Run shot.exe with arguments (e.g., just run -- --list-windows)
run *ARGS:
    cargo run --release -- {{ARGS}}
