# Fuzzing Tests for Ferrous DI

This directory contains fuzz targets for testing the robustness and security of the ferrous-di dependency injection library.

## Prerequisites

Fuzzing requires nightly Rust and cargo-fuzz:

```bash
# Install nightly toolchain
rustup install nightly
rustup default nightly

# Install cargo-fuzz
cargo install cargo-fuzz
```

## Available Fuzz Targets

### 1. Service Registration (`service_registration.rs`)
Tests various service registration patterns:
- Singleton registration
- Factory registration
- Scoped registration  
- Transient registration
- Multiple registrations (last-wins semantics)
- Trait registration

### 2. Service Resolution (`service_resolution.rs`) 
Tests service resolution behavior:
- Required vs optional resolution
- Trait resolution
- Scoped service isolation
- Singleton consistency across scopes
- Multi-binding resolution

### 3. Dependency Injection (`dependency_injection.rs`)
Tests complex dependency injection scenarios:
- Simple dependency chains
- Multi-level dependency resolution
- Mixed lifetime dependencies
- Trait-based dependency injection

## Running Fuzz Tests

```bash
# Run a specific fuzz target for 60 seconds
cargo fuzz run service_registration -- -max_total_time=60

# Run with specific number of iterations
cargo fuzz run service_resolution -- -runs=10000

# Run dependency injection fuzzing
cargo fuzz run dependency_injection -- -max_total_time=120
```

## Continuous Fuzzing

For continuous integration, you can run fuzzing for a limited time:

```bash
# Quick smoke test (10 seconds each target)
cargo fuzz run service_registration -- -max_total_time=10
cargo fuzz run service_resolution -- -max_total_time=10  
cargo fuzz run dependency_injection -- -max_total_time=10
```

## Interpreting Results

Fuzz testing will:
- Discover crashes, panics, or assertion failures
- Find edge cases in service registration/resolution
- Validate that dependency injection behaves correctly with random inputs
- Ensure thread safety under concurrent access

Any crashes found will be saved to the `artifacts/` directory for reproduction and debugging.

## Coverage

The fuzz targets cover:
- All service lifetime patterns (Singleton, Scoped, Transient)
- Both concrete types and trait objects
- Dependency injection chains
- Error handling paths
- Thread safety scenarios

## Integration with CI

Add fuzzing to your CI pipeline:

```yaml
- name: Run Fuzz Tests
  run: |
    rustup install nightly
    rustup default nightly
    cargo install cargo-fuzz
    cargo fuzz run service_registration -- -max_total_time=30
    cargo fuzz run service_resolution -- -max_total_time=30
    cargo fuzz run dependency_injection -- -max_total_time=30
```