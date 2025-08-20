# API Stability Guarantees

## Overview

This document defines the API stability guarantees for ferrous-di, providing clear expectations for users about what changes can be expected across different types of releases. These guarantees form the foundation of our semantic versioning policy and release engineering practices.

## Stability Commitment

### Core Promise
ferrous-di commits to providing **predictable API evolution** with **clear migration paths** for necessary changes. Users should be able to upgrade with confidence, knowing exactly what to expect from each release type.

### Rust Ecosystem Alignment
Our stability guarantees align with Rust ecosystem best practices:
- Follow semantic versioning strictly
- Maintain source compatibility within major versions
- Provide deprecation warnings before removal
- Document all breaking changes comprehensively

## API Stability Tiers

### 🔒 Tier 1: Stable API (Ironclad Guarantees)

#### Core Types and Traits
```rust
// These APIs have the strongest stability guarantees
pub trait Resolver { }
pub trait ResolverCore { }
pub struct ServiceCollection { }
pub struct ServiceProvider { }
pub struct Scope { }
pub enum DiError { }
pub type DiResult<T> = Result<T, DiError>;
```

#### Stability Promise
- **No breaking changes** within major version
- **Minimum 12 months** deprecation period before removal
- **Extensive migration support** for major version changes
- **Backwards compatibility** maintained at all costs

#### Change Policy
- **PATCH**: Bug fixes only, no API changes
- **MINOR**: Additive changes only (new methods with defaults)
- **MAJOR**: Breaking changes allowed with comprehensive migration

### 🟡 Tier 2: Evolving API (Strong Guarantees)

#### Advanced Features
```rust
// These APIs have strong but flexible guarantees
pub mod async_factories { }
pub mod decorators { }
pub mod observers { }
pub mod graph_export { }
pub trait AsyncFactory<T> { }
pub trait ServiceDecorator { }
```

#### Stability Promise
- **Careful evolution** within major versions
- **6 months minimum** deprecation period
- **Clear migration paths** for changes
- **Additive changes preferred** over breaking changes

#### Change Policy
- **PATCH**: Bug fixes, minor behavior corrections
- **MINOR**: New features, minor breaking changes if well-justified
- **MAJOR**: Significant API redesign allowed

### 🟠 Tier 3: Experimental API (Limited Guarantees)

#### Experimental Features
```rust
// These APIs may change with less notice
#[cfg(feature = "experimental")]
pub mod experimental { }

#[doc(hidden)]
pub mod internal { }

// APIs marked as unstable
#[unstable(feature = "advanced_features")]
pub fn experimental_method() { }
```

#### Stability Promise
- **May change** in minor versions with clear documentation
- **Minimal deprecation** period (1 minor version)
- **Clear experimental marking** in documentation
- **Opt-in usage** through feature flags or explicit imports

#### Change Policy
- **PATCH**: Bug fixes, small improvements
- **MINOR**: API changes allowed with documentation
- **MAJOR**: Complete redesign or removal allowed

## Compatibility Guarantees

### Source Compatibility

#### Within Major Versions (X.Y.Z → X.Y+n.Z)
```rust
// This code should continue to compile across minor versions
let mut services = ServiceCollection::new();
services.add_singleton(MyService::new());
let provider = services.build();
let service = provider.get_required::<MyService>();
```

**Guarantees:**
- Code that compiles with version X.Y.Z compiles with X.Y+n.Z
- Existing method signatures remain unchanged
- Existing behavior remains consistent (except for bug fixes)

#### Across Major Versions (X.Y.Z → X+1.Y.Z)
**No guarantees** - breaking changes allowed with:
- Comprehensive migration documentation
- Automated migration tools when possible
- Clear timeline and rationale

### Behavioral Compatibility

#### Documented Behavior
```rust
/// Resolves a service of type T.
/// 
/// # Behavior Guarantee
/// This method will always return the same instance for Singleton services
/// within the same ServiceProvider instance.
/// 
/// # Errors
/// Returns DiError::ServiceNotFound if no service of type T is registered.
pub fn get_required<T>(&self) -> DiResult<Arc<T>> { }
```

**Guarantees:**
- Documented behavior remains consistent within major versions
- Error conditions remain the same unless explicitly documented
- Performance characteristics maintained within reasonable bounds

#### Undocumented Behavior
**No guarantees** - implementation details may change in any release:
- Internal data structures
- Memory layout
- Specific error messages (beyond documented types)
- Performance characteristics (unless documented)

### ABI Compatibility

#### Rust ABI Limitations
Rust does not provide stable ABI, therefore:
- **No ABI compatibility** guaranteed across any releases
- **Recompilation required** for all dependency updates
- **Dynamic linking** not supported or recommended

#### Recommendations
- Pin specific versions in production: `ferrous-di = "=1.2.3"`
- Use compatible version ranges: `ferrous-di = "1.2"`
- Test thoroughly when updating versions

## MSRV (Minimum Supported Rust Version) Policy

### Current MSRV: Rust 1.70.0

#### MSRV Update Policy
- **Conservative approach**: 6-month lag behind latest stable
- **Patch releases**: Never increase MSRV
- **Minor releases**: May increase within same Rust minor version (1.70.0 → 1.70.8)
- **Major releases**: May increase to newer Rust minor version (1.70.x → 1.75.x)

#### MSRV Change Communication
```toml
# Clear documentation in Cargo.toml
[package]
rust-version = "1.70.0"  # Enforced MSRV

# Clear documentation in README
## Minimum Supported Rust Version (MSRV)
This crate requires Rust 1.70.0 or newer.
```

### MSRV Testing
- CI tests against MSRV and latest stable
- MSRV bumps require strong justification
- Community notification for MSRV changes

## Feature Flag Stability

### Feature Flag Categories

#### Stable Features
```toml
[features]
default = ["std"]
std = []  # Stable feature, safe for production
async = ["tokio"]  # Stable feature, well-tested
```

**Guarantees:**
- Feature flags themselves won't be removed within major versions
- APIs behind stable features follow Tier 1 or Tier 2 guarantees
- Feature combinations are tested and supported

#### Experimental Features
```toml
[features]
experimental = []  # May change or be removed
unstable-async = []  # No stability guarantees
```

**Guarantees:**
- May be removed in minor versions with clear notice
- APIs may change significantly
- Not recommended for production use

### Feature Flag Evolution
- **Adding features**: Always safe in minor releases
- **Changing feature behavior**: Follows same rules as API changes
- **Removing features**: Only in major releases (stable) or minor releases (experimental)

## Error Handling Stability

### Error Type Guarantees

#### Stable Error Types
```rust
#[derive(Debug, Clone, PartialEq)]
pub enum DiError {
    ServiceNotFound(String),
    CircularDependency(Vec<String>),
    LifetimeViolation(String),
    // Adding new variants is non-breaking
}
```

**Guarantees:**
- Existing error variants won't be removed within major versions
- Error variant data may be enhanced (non-breaking)
- New error variants may be added (non-breaking)
- Error messages may improve (not guaranteed to be stable)

#### Error Compatibility
```rust
// This pattern should continue to work
match provider.get::<MyService>() {
    Ok(service) => { /* use service */ }
    Err(DiError::ServiceNotFound(_)) => { /* handle missing service */ }
    Err(other) => { /* handle other errors */ }
}
```

## Performance Guarantees

### Performance Characteristics

#### Algorithmic Complexity
Current documented complexities that won't regress within major versions:
- **Service resolution**: O(1) for singletons, O(log n) for scoped
- **Registration**: O(1) amortized
- **Scope creation**: O(1)

#### Performance Regression Policy
- **Significant regressions** (>25%) are considered breaking changes
- **Minor optimizations** allowed in any release
- **Benchmark requirements** for performance-sensitive changes

### Benchmark Stability
```rust
// These benchmarks define our performance contracts
#[bench]
fn singleton_resolution_performance() {
    // Must complete in <100ns on reference hardware
}

#[bench]
fn scope_creation_performance() {
    // Must complete in <1μs on reference hardware
}
```

## Documentation Stability

### API Documentation
- **Method signatures** in docs must match actual implementation
- **Examples** must compile and work with current version
- **Behavior descriptions** are part of stability contract

### Migration Documentation
- **Migration guides** maintained for all major version transitions
- **Deprecation notices** provide clear alternatives
- **Breaking change documentation** includes timeline and rationale

## Testing Stability Guarantees

### Public API Testing
```rust
#[test]
fn api_stability_test() {
    // These tests verify API stability promises
    let services = ServiceCollection::new();
    // This API must remain stable within major version
    assert!(services.add_singleton(String::from("test")).is_ok());
}
```

### Integration Testing
- Tests that verify compatibility with common usage patterns
- Tests that ensure migration paths work correctly
- Tests that validate performance characteristics

## Ecosystem Compatibility

### Popular Crate Integration
We maintain compatibility testing with popular crates in the ecosystem:
- **tokio**: Async runtime integration
- **serde**: Serialization support
- **tracing**: Logging and diagnostics

### Framework Integration
Stability guarantees for integration patterns:
- **Web frameworks**: Axum, Actix, Warp integration patterns
- **CLI tools**: Integration with clap and similar
- **Database libraries**: Connection pool management patterns

## Violating Stability Guarantees

### Emergency Exceptions
Stability guarantees may be violated only for:
- **Critical security vulnerabilities** that can't be fixed compatibly
- **Memory safety issues** that pose immediate risk
- **Data corruption bugs** that can't be fixed without breaking changes

### Exception Process
1. **Security review** confirms necessity
2. **Community notification** with emergency timeline
3. **Rapid patch release** with clear documentation
4. **Follow-up** with better long-term solution

### Exception Communication
```markdown
# SECURITY RELEASE 1.2.4

## Emergency Breaking Change

This release contains a security fix that requires a breaking change.
The fix addresses CVE-XXXX-XXXX which could lead to memory safety violations.

### Breaking Change
- Method `unsafe_method()` has been removed immediately
- Use `safe_alternative()` instead

### Migration
// Before (vulnerable)
unsafe_method(data);

// After (secure)
safe_alternative(data)?;
```

## Stability Validation

### Automated Checks
- **API diff validation** in CI prevents accidental breaking changes
- **Semver-check** tool validates version increments
- **Documentation tests** ensure examples remain valid

### Manual Review Process
- **API review board** for significant changes
- **Community feedback** period for major changes
- **Breaking change justification** required

### Tooling
```bash
# Tools used to validate stability
cargo semver-checks
cargo doc --all-features
cargo test --all-features
cargo audit
```

## Community Communication

### Stability Promise Communication
- **Clear documentation** in README and docs
- **Stability badges** indicating API maturity
- **Change notification** through multiple channels

### Feedback Channels
- **GitHub issues** for stability concerns
- **Community forums** for discussion
- **Direct contact** for security issues

## Future Evolution

### Planned Improvements
- **Enhanced performance** guarantees with more detailed metrics
- **ABI stability** exploration for specific use cases
- **Formal verification** of critical stability properties

### Policy Updates
This stability policy itself may evolve:
- Annual review of policy effectiveness
- Community feedback incorporation
- Ecosystem best practice alignment

---

## Quick Reference

### Stability Tiers
- **🔒 Tier 1**: Core APIs - Ironclad stability guarantees
- **🟡 Tier 2**: Advanced APIs - Strong but flexible guarantees  
- **🟠 Tier 3**: Experimental APIs - Limited guarantees

### Version Impact
- **PATCH (X.Y.Z+1)**: Bug fixes only, no API changes
- **MINOR (X.Y+1.0)**: Additive changes, strong backwards compatibility
- **MAJOR (X+1.0.0)**: Breaking changes allowed with migration support

### MSRV Policy
- **Conservative**: 6-month lag behind Rust stable
- **Patch**: Never increases MSRV
- **Minor**: May increase within Rust minor version
- **Major**: May increase to newer Rust minor version

These stability guarantees provide the foundation for reliable, predictable API evolution while enabling necessary improvements and innovations in ferrous-di.