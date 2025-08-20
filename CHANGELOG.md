# Changelog

All notable changes to Ferrous DI will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Comprehensive CI/CD infrastructure with GitHub Actions workflows
  - Multi-platform testing (Ubuntu, Windows, macOS)
  - Multi-Rust version support (stable, beta, nightly)
  - Code coverage reporting with tarpaulin + codecov.io
  - Performance regression detection with automated alerts
  - Security auditing and license compliance checks
  - Automated crates.io publishing on version tags
- Complete documentation suite
  - CONTRIBUTING.md with development guidelines
  - ARCHITECTURE.md with design decisions and patterns
  - SECURITY.md with vulnerability reporting process
  - CODE_OF_CONDUCT.md for community guidelines
  - Comprehensive CLAUDE.md for AI-assisted development

### Changed

- Updated CLAUDE.md with detailed project information and CI/CD workflows

## [0.1.0] - 2025-08-XX - Initial Release

### Added

- **Core Dependency Injection System**
  - Service lifetimes: Singleton, Scoped, Transient
  - Type-safe dependency resolution with `Arc<T>` sharing
  - Factory-based service construction with dependency injection
  - Hierarchical scopes with proper disposal management
- **Advanced Features**
  - Service descriptors with metadata for runtime introspection
  - Named services for multiple implementations of the same type
  - Conditional registration methods (`TryAdd*` family)
  - Trait object support for dynamic dispatch
- **Performance & Reliability**
  - ~78ns singleton resolution baseline
  - Thread-safe concurrent access
  - Circular dependency detection with detailed error paths
  - LIFO disposal order for proper cleanup
  - Zero-allocation hot paths where possible
- **Modular Architecture**
  - Well-organized module structure (9 modules)
  - Clean separation of concerns
  - Type-erased internal storage with compile-time safety
  - Extracted from monolithic 4000+ line codebase
- **Comprehensive Testing**
  - 46 integration tests across 6 test modules
  - 46 documentation tests with realistic examples
  - 6 unit tests for core functionality
  - Property-based testing for circular dependency detection
  - Performance benchmarks with Criterion.rs
- **Optional Performance Features**
  - `parking-lot`: Faster mutex implementations
  - `ahash`: Faster hashing algorithms
  - `smallvec`: Stack-allocated small vectors
  - `once-cell`: Lock-free singleton access (experimental)
- **Error Handling**
  - Comprehensive `DiError` enum with detailed error information
  - `DiResult<T>` type alias for consistent error handling
  - Proper error propagation throughout the system
  - Circular dependency detection with full path information
- **Developer Experience**
  - Extensive documentation with examples
  - Web server scope example demonstrating real-world usage
  - Detailed README with feature explanations
  - Performance characteristics documentation

### Performance Benchmarks

- Singleton resolution: ~78ns baseline
- Factory resolution: ~150ns average
- Scoped resolution: ~200ns average
- Complex dependency graphs: Linear scaling
- Multi-threaded throughput: Excellent scalability

### API Reference

- `ServiceCollection`: Service registration and configuration
- `ServiceProvider`: Root service resolution and scope creation
- `Scope`: Scoped service resolution with automatic disposal
- `DiResult<T>`: Result type for all fallible operations
- `DiError`: Comprehensive error types with context
- `Resolver` trait: Core resolution capabilities
- `Dispose`/`AsyncDispose` traits: Resource cleanup

### Breaking Changes

- N/A (Initial release)

### Dependencies

- Standard library only (no external runtime dependencies)
- Optional performance dependencies available via feature flags
- MSRV: Rust 1.70.0

### Documentation

- Complete API documentation with examples
- Architecture guide explaining design decisions
- Performance guide with optimization recommendations
- Contributing guide for community development

---

## Release Notes Template

When releasing a new version, use this template:

```markdown
## [X.Y.Z] - YYYY-MM-DD

### Added

- New features and capabilities

### Changed

- Changes to existing functionality

### Deprecated

- Features marked for removal in future versions

### Removed

- Features removed in this version

### Fixed

- Bug fixes and corrections

### Security

- Security-related changes and fixes

### Performance

- Performance improvements and optimizations

### Breaking Changes

- Changes that break backward compatibility
```

### Versioning Guidelines

- **MAJOR** (X.0.0): Breaking changes to public API
- **MINOR** (0.X.0): New features, backward compatible
- **PATCH** (0.0.X): Bug fixes, backward compatible

### Links

- [Repository](https://github.com/s1ntropy/ferrous-di)
- [Documentation](https://docs.rs/ferrous-di)
- [Crates.io](https://crates.io/crates/ferrous-di)
- [Issues](https://github.com/s1ntropy/ferrous-di/issues)
