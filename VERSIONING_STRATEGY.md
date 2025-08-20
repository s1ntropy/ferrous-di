# Semantic Versioning Strategy

## Overview

Ferrous DI follows [Semantic Versioning 2.0.0](https://semver.org/) with strict adherence to API stability guarantees and clear breaking change policies.

## Version Format

```
MAJOR.MINOR.PATCH[-PRERELEASE][+BUILD]
```

### Version Components

- **MAJOR**: Incompatible API changes
- **MINOR**: Backwards-compatible functionality additions
- **PATCH**: Backwards-compatible bug fixes
- **PRERELEASE**: Optional pre-release identifiers (alpha, beta, rc)
- **BUILD**: Optional build metadata

## Version Increment Rules

### MAJOR (Breaking Changes)
Increment when making incompatible API changes:

- Removing public API functions, methods, or types
- Changing function signatures (parameters, return types)
- Changing trait definitions or implementations
- Modifying public struct fields
- Changing behavior in ways that break existing code
- Bumping MSRV (Minimum Supported Rust Version) beyond compatibility window

**Examples:**
- `1.2.3 → 2.0.0`: Removed `ServiceCollection::deprecated_method()`
- `1.2.3 → 2.0.0`: Changed `get<T>()` to `get<T>() -> Result<T>`
- `1.2.3 → 2.0.0`: MSRV bump from 1.70 to 1.75

### MINOR (Feature Additions)
Increment when adding backwards-compatible functionality:

- Adding new public API functions, methods, or types
- Adding new optional parameters with defaults
- Adding new trait methods with default implementations
- Adding new feature flags
- Performance improvements without API changes
- Internal refactoring that doesn't affect public API

**Examples:**
- `1.2.3 → 1.3.0`: Added `ServiceCollection::add_middleware()`
- `1.2.3 → 1.3.0`: Added new `async-std` feature flag
- `1.2.3 → 1.3.0`: Added `get_optional<T>()` method

### PATCH (Bug Fixes)
Increment when making backwards-compatible bug fixes:

- Fixing incorrect behavior without changing API
- Security patches that don't change API
- Documentation fixes and improvements
- Internal bug fixes
- Dependency updates (patch level only)

**Examples:**
- `1.2.3 → 1.2.4`: Fixed memory leak in scope disposal
- `1.2.3 → 1.2.4`: Fixed deadlock in concurrent resolution
- `1.2.3 → 1.2.4`: Updated documentation examples

## Pre-Release Versioning

### Alpha Releases
- **Format**: `X.Y.Z-alpha.N`
- **Purpose**: Early development, API unstable
- **Stability**: No API guarantees, expect breaking changes
- **Usage**: Internal testing, early feedback

### Beta Releases
- **Format**: `X.Y.Z-beta.N`
- **Purpose**: Feature-complete, API mostly stable
- **Stability**: Minor API changes possible
- **Usage**: Integration testing, community feedback

### Release Candidates
- **Format**: `X.Y.Z-rc.N`
- **Purpose**: Production-ready candidates
- **Stability**: API frozen, only critical bug fixes
- **Usage**: Final validation before stable release

## Feature Flags and Versioning

### Feature Flag Policy
- New features should be behind feature flags when possible
- Feature flags are additive and don't affect base API
- Removing feature flags requires MAJOR version bump
- Feature flag changes follow same versioning rules

### Optional Dependencies
- Adding optional dependencies: MINOR
- Updating optional dependencies: PATCH (compatible) or MINOR (new features)
- Removing optional dependencies: MAJOR

## MSRV (Minimum Supported Rust Version) Policy

### Current MSRV
- **Current**: Rust 1.70.0
- **Policy**: Conservative, 6-month lag behind stable
- **Testing**: CI tests against MSRV and latest stable

### MSRV Updates
- **Minor version**: MSRV updates within same minor Rust version (1.70.0 → 1.70.5)
- **Major version**: MSRV updates to new minor Rust version (1.70 → 1.75)
- **Rationale**: Document why MSRV bump is needed
- **Notice**: Announce MSRV changes prominently in release notes

## API Stability Tiers

### Tier 1: Stable API (Full SemVer Guarantees)
- Core dependency injection traits (`Resolver`, `ResolverCore`)
- Service registration methods (`ServiceCollection`)
- Primary lifecycle management (`ServiceProvider`, `Scope`)
- Public error types (`DiError`, `DiResult`)

### Tier 2: Evolving API (Careful Changes)
- Advanced features (async factories, decorators)
- Performance optimization features
- Extended trait methods with default implementations
- Optional feature flag APIs

### Tier 3: Experimental API (May Change)
- Marked with `#[doc(hidden)]` or explicit warnings
- Behind experimental feature flags
- Internal APIs exposed for advanced use cases
- May change in MINOR versions with clear documentation

## Deprecation Policy

### Deprecation Process
1. **Mark as deprecated**: Add `#[deprecated]` attribute with message
2. **Documentation**: Update docs with migration path
3. **Grace period**: Minimum 2 MINOR versions before removal
4. **Removal**: Only in MAJOR version with migration guide

### Deprecation Messages
```rust
#[deprecated(
    since = "1.3.0",
    note = "Use `new_method()` instead. Will be removed in 2.0.0"
)]
pub fn old_method(&self) -> Result<()> {
    // Implementation or delegation to new method
}
```

## Release Branches and Workflow

### Branch Strategy
- **main**: Development branch, future minor/major releases
- **release/X.Y**: Maintenance branches for patch releases
- **hotfix/X.Y.Z**: Critical fixes for immediate release

### Release Workflow
1. **Feature freeze**: Create release branch from main
2. **Stabilization**: Bug fixes and documentation on release branch
3. **Release candidate**: Tag RC for final testing
4. **Stable release**: Tag stable version after validation
5. **Maintenance**: Cherry-pick critical fixes to release branch

## Compatibility Guidelines

### Source Compatibility
- Code that compiled with version X.Y.Z compiles with X.Y.Z+1
- Code that compiled with version X.Y.Z compiles with X.Y+1.0
- Breaking changes only in MAJOR versions

### Behavioral Compatibility
- Documented behavior remains consistent within MAJOR version
- Performance improvements don't change API contracts
- Bug fixes may change behavior if previous behavior was incorrect

### Dependency Compatibility
- Dependencies follow same versioning principles
- Use compatible version ranges in Cargo.toml
- Document dependency version requirements clearly

## Release Automation

### Version Bumping
- Automated via `cargo-release` or custom scripts
- Validates version increments follow policy
- Updates Cargo.toml, CHANGELOG.md, and documentation

### Changelog Generation
- Generate from conventional commits
- Categorize changes by type (features, fixes, breaking)
- Include migration instructions for breaking changes

### Publishing Pipeline
- Automated testing on multiple Rust versions
- Documentation generation and publishing
- Crate publishing to crates.io
- GitHub release creation with assets

## Examples and Edge Cases

### Complex Scenarios

#### Adding Optional Generic Parameter
```rust
// Before (1.2.3)
impl<T> Service<T> { }

// After (1.3.0) - MINOR: backwards compatible
impl<T, E = DefaultError> Service<T, E> { }
```

#### Changing Internal Implementation
```rust
// Before (1.2.3)
pub fn resolve<T>(&self) -> Result<T> {
    // HashMap-based implementation
}

// After (1.2.4) - PATCH: same API, better performance
pub fn resolve<T>(&self) -> Result<T> {
    // BTreeMap-based implementation - faster lookups
}
```

#### Feature Flag Graduation
```rust
// Version 1.2.0: Behind feature flag
#[cfg(feature = "experimental-async")]
pub async fn async_resolve<T>(&self) -> Result<T> { }

// Version 1.3.0: Promoted to stable (MINOR)
pub async fn async_resolve<T>(&self) -> Result<T> { }
```

## Documentation Requirements

### Release Notes
- **Breaking changes**: Clearly marked with migration path
- **New features**: Examples and use cases
- **Bug fixes**: Description of fixed issues
- **Deprecations**: Timeline and alternatives

### Migration Guides
- Step-by-step instructions for major version upgrades
- Code examples showing before/after
- Common pitfalls and solutions
- Automated migration tools when possible

## Validation and Quality Gates

### Pre-Release Checklist
- [ ] All tests pass on supported Rust versions
- [ ] Documentation builds without warnings
- [ ] Examples compile and run successfully
- [ ] Benchmarks show no performance regressions
- [ ] Breaking changes are documented
- [ ] Migration guide is complete (for major versions)
- [ ] Security review completed (for security-sensitive changes)

### Release Criteria
- **Patch**: All tests pass, no breaking changes
- **Minor**: Feature complete, API stable, documentation updated
- **Major**: Migration guide complete, breaking changes documented, community notice

## Communication Strategy

### Pre-Release Communication
- **Major versions**: Blog post and community announcement
- **Minor versions**: Release notes and social media
- **Patch versions**: Changelog and automated notifications

### Breaking Change Notice
- Minimum 30 days advance notice for major versions
- Clear timeline for deprecation removal
- Community feedback period for significant changes

## Tooling and Automation

### Required Tools
- `cargo-release`: Version management and publishing
- `conventional_changelog`: Automated changelog generation
- `cargo-audit`: Security vulnerability scanning
- `cargo-outdated`: Dependency update tracking

### CI/CD Integration
- Version validation in pull requests
- Automated testing across Rust version matrix
- Documentation deployment on releases
- Security scanning on every commit

This versioning strategy ensures predictable, professional releases while maintaining high quality and clear communication with the community.