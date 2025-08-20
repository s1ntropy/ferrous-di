# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

**Ferrous DI** is a comprehensive, enterprise-grade dependency injection library for Rust featuring:

### Core Features
- **Service Lifetimes**: Singleton, Scoped, Transient with proper isolation
- **Type Safety**: Full compile-time type checking with Arc<T> sharing
- **Performance**: ~78ns singleton resolution, optimized for production
- **Trait Support**: Single bindings and multi-bindings for trait objects
- **Circular Detection**: Comprehensive cycle detection with detailed error paths
- **Thread Safety**: All APIs are Send + Sync with safe concurrent access

### Advanced Features
- **Async Support**: Async factories, async disposal, async lifecycle management
- **AOP (Aspect-Oriented Programming)**: Method interception and decoration patterns
- **Module System**: Hierarchical service organization and configuration
- **Diagnostics**: Comprehensive service graph export and debugging tools
- **Performance Monitoring**: Built-in metrics and telemetry
- **Web Integration**: Framework-agnostic patterns for web applications
- **Reliability**: Circuit breakers, retries, and fault tolerance
- **Agent Architecture**: Durable agent patterns with state management

## Development Commands

### Building and Testing
```bash
# Build the project
cargo build

# Run all tests (46 integration + 46 doc + 6 unit tests)
cargo test

# Run specific test categories
cargo test --test basics      # Basic functionality tests
cargo test --test circular    # Circular dependency tests
cargo test --test scopes      # Scoped lifetime tests
cargo test --test advanced_features  # Named services, metadata, TryAdd

# Run doc tests
cargo test --doc

# Format code
cargo fmt

# Run clippy lints
cargo clippy --all-targets --all-features -- -D warnings
```

### Performance
```bash
# Run benchmarks
cargo bench

# Run benchmarks with performance features
cargo bench --features performance

# Performance features available:
# - parking-lot: Faster mutexes
# - ahash: Faster hashing
# - smallvec: Stack-allocated small vectors
# - once-cell: Lock-free singletons (experimental)
```

### Examples
```bash
# Run the web server scope example
cargo run --example web_server_scope

# Run the modular registration example
cargo run --example modular_registration

# Run the durable agent example
cd examples/durable-agent && cargo run
```

### Release Engineering
```bash
# Generate changelog from conventional commits
./scripts/generate-changelog.sh update v1.2.3

# Create automated release (with validation)
./scripts/release.sh release auto

# Publish to crates.io (with safety checks)
./scripts/publish.sh publish

# Dry run any release process
./scripts/release.sh dry-run minor
./scripts/publish.sh dry-run
```

### Quality Assurance
```bash
# Run mutation testing
cargo mutants

# Run fuzzing tests
cargo fuzz run dependency_injection
cargo fuzz run service_registration
cargo fuzz run service_resolution

# Security audit
cargo audit

# Check for unused dependencies
cargo +nightly udeps
```

## Architecture

### Module Structure
```
src/
├── lib.rs                  # Main API (2821 lines, down from 4000+)
├── error.rs                # DiError and DiResult types
├── lifetime.rs             # Lifetime enum
├── key.rs                  # Key enum with methods
├── descriptors.rs          # ServiceDescriptor for introspection
├── registration.rs         # Registration + Registry structs
├── async_di.rs             # Async dependency injection patterns
├── async_factories.rs      # Async service factories
├── async_lifecycle.rs      # Async lifecycle management
├── aop.rs                  # Aspect-oriented programming support
├── capabilities.rs         # Service capability definitions
├── cancellation.rs         # Cancellation token support
├── config.rs               # Configuration binding and validation
├── debug.rs                # Debugging and diagnostics
├── decoration.rs           # Service decoration patterns
├── fast_singletons.rs      # Optimized singleton implementations
├── graph_export.rs         # Service graph visualization
├── labeled_scopes.rs       # Named scope management
├── metrics.rs              # Performance metrics and telemetry
├── observer.rs             # Observer pattern implementation
├── performance.rs          # Performance optimizations
├── prewarm.rs              # Service prewarming strategies
├── reliability.rs          # Circuit breakers and fault tolerance
├── scope_local.rs          # Scope-local storage
├── validation.rs           # Service validation framework
├── web_integration.rs      # Web framework integration patterns
├── collection/
│   ├── mod.rs              # Collection abstractions
│   └── module_system.rs    # Hierarchical module system
├── provider/
│   ├── mod.rs              # Provider abstractions
│   ├── context.rs          # Resolution context management
│   └── scope.rs            # Scope implementation
├── traits/
│   ├── mod.rs              # Trait exports
│   ├── dispose.rs          # Disposal traits (Dispose, AsyncDispose)
│   └── resolver.rs         # Resolver traits (475 lines)
└── internal/
    ├── mod.rs              # Internal utilities
    ├── circular.rs         # Circular dependency detection
    └── dispose_bag.rs      # LIFO disposal management
```

### Key Components
- **ServiceCollection**: Service registration and configuration with module support
- **ServiceProvider**: Root service resolution and scope creation
- **Scope**: Scoped service resolution with automatic disposal
- **ResolverContext**: Thread-local resolution context with circular detection
- **AsyncFactory**: Async service creation with lifecycle management
- **ServiceDecorator**: AOP-style service decoration and interception
- **Module**: Hierarchical service organization and configuration
- **Observer**: Event-driven service lifecycle monitoring
- **MetricsCollector**: Performance monitoring and telemetry
- **ReliabilityManager**: Circuit breakers and fault tolerance
- **ValidationFramework**: Service validation and health checks

## Release Engineering & Quality Assurance

### Professional Release Process
- **Semantic Versioning**: Strict SemVer 2.0.0 compliance with API stability tiers
- **Conventional Commits**: Automated changelog generation and version bumping
- **Breaking Change Policy**: Comprehensive deprecation and migration processes
- **API Stability Guarantees**: Multi-tier stability promises (Tier 1: Ironclad, Tier 2: Strong, Tier 3: Experimental)
- **Release Checklist**: 50+ step validation process for quality assurance
- **Migration Guides**: Detailed documentation for major version transitions

### Automation Scripts
- **scripts/generate-changelog.sh**: Automated changelog from conventional commits
- **scripts/release.sh**: Complete release automation with validation
- **scripts/publish.sh**: Safe crates.io publishing with verification and rollback

### Quality Gates
- **Mutation Testing**: cargo-mutants for test quality validation
- **Fuzzing**: Property-based testing with cargo-fuzz
- **Security Auditing**: Dependency vulnerability scanning
- **Performance Regression**: Automated benchmark validation
- **Code Coverage**: Comprehensive test coverage reporting
- **License Compliance**: Automated license compatibility checking

## Development Guidelines

### Testing Strategy
- **Unit Tests**: Embedded tests in source modules (50+ tests)
- **Integration Tests**: Comprehensive test suites by feature area:
  - `tests/basics.rs`: Core functionality validation
  - `tests/scopes.rs`: Scoped lifetime management
  - `tests/circular.rs`: Circular dependency detection
  - `tests/advanced_features.rs`: Named services, metadata, TryAdd
  - `tests/agent_features.rs`: Durable agent patterns
  - `tests/modules.rs`: Module system testing
  - `tests/disposal.rs`: Resource cleanup validation
- **Property Tests**: Fuzz testing with proptest and cargo-fuzz
- **Doc Tests**: Extensive documentation examples (100+ tests)
- **Benchmark Tests**: Performance regression detection
- **Mutation Tests**: Test quality validation with cargo-mutants

### Performance Requirements
- Singleton resolution must stay under 100ns
- Memory-efficient service storage with Arc sharing
- Lock-free hot paths where possible
- Comprehensive benchmarking with regression detection

### Code Quality Standards
- Zero clippy warnings on all targets
- Formatted with `cargo fmt`
- All public APIs documented with examples
- Module files should stay under 500 lines where practical

## Troubleshooting

### Common Commands
```bash
# Full clean rebuild
cargo clean && cargo build

# Test with verbose output
cargo test -- --nocapture

# Check for unused dependencies
cargo +nightly udeps

# Audit dependencies
cargo audit
```

### Feature Flags
- **Default Features**: Core DI functionality with std support
- **Performance Features**: `parking-lot`, `ahash`, `smallvec` for optimization
- **Async Features**: `tokio` integration for async patterns
- **Diagnostics Features**: Debug tools and service graph export
- **Web Features**: Framework integration patterns
- **Experimental Features**: Cutting-edge functionality under development

### Known Issues & Limitations
- Performance features are optional and must be enabled explicitly
- Async features require tokio runtime for full functionality
- MSRV: Rust 1.70.0+ (enforced in Cargo.toml)
- Some advanced features are gated behind experimental flags

## Project Status

**Current Phase**: Phase 7 Complete - Professional Release Engineering  
**Completed Phases**:
- ✅ **Phase 1**: Code Organization & Refactoring (modular architecture)
- ✅ **Phase 2**: CI/CD Infrastructure (comprehensive workflows)
- ✅ **Phase 3**: Documentation Enhancement (comprehensive docs)
- ✅ **Phase 4**: Development Tools (debugging, diagnostics, validation)
- ✅ **Phase 5**: Testing Improvements (mutation testing, fuzzing, property tests)
- ✅ **Phase 6**: Performance & Production Readiness (optimization, reliability)
- ✅ **Phase 7**: Release Engineering (professional release process)

**Upcoming Phases**:
- 🔄 **Phase 8**: Advanced Agent Features (state management, persistence)
- 📋 **Phase 9**: Ecosystem Integration (framework adapters, tooling)
- 🎯 **Phase 10**: Stabilization & v1.0 Preparation (API finalization)

### Project Maturity
- **Architecture**: Enterprise-grade modular design with 25+ specialized modules
- **Testing**: 200+ tests across unit, integration, property, and mutation testing
- **Documentation**: Comprehensive API docs, examples, migration guides
- **Quality**: Professional release process with automated validation
- **Performance**: Production-ready with extensive benchmarking
- **Reliability**: Circuit breakers, fault tolerance, comprehensive error handling

See ROADMAP.md for the complete development plan and future roadmap.