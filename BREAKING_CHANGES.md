# Breaking Changes Policy

## Overview

This document defines the policy for introducing, communicating, and managing breaking changes in ferrous-di. We are committed to providing a stable, predictable API while allowing for necessary evolution and improvements.

## Definition of Breaking Changes

### API Breaking Changes

#### Public Interface Changes
- **Function/Method Removal**: Removing any public function, method, or associated function
- **Signature Changes**: Modifying parameters, return types, or generic constraints
- **Trait Changes**: Adding required methods, removing methods, or changing method signatures
- **Type Changes**: Removing public types, changing struct fields, or enum variants
- **Module Changes**: Moving or removing public modules, changing module visibility

#### Behavioral Breaking Changes
- **Contract Changes**: Modifying documented behavior or API contracts
- **Error Handling**: Changing error types or error conditions
- **Performance**: Significant performance degradation (>25% regression)
- **Thread Safety**: Removing thread safety guarantees
- **Memory Safety**: Changes that could introduce memory unsafety

#### Dependency Breaking Changes
- **MSRV Bumps**: Increasing Minimum Supported Rust Version beyond policy
- **Feature Changes**: Removing feature flags or changing their behavior
- **Optional Dependencies**: Making optional dependencies required
- **Version Requirements**: Incompatible dependency version bumps

### Examples of Breaking Changes

#### ❌ Breaking: Function Signature Change
```rust
// Before (1.x.x)
pub fn get<T: 'static>(&self) -> Option<Arc<T>>

// After (2.0.0) - BREAKING: Return type changed
pub fn get<T: 'static>(&self) -> DiResult<Arc<T>>
```

#### ❌ Breaking: Trait Method Addition
```rust
// Before (1.x.x)
pub trait Resolver {
    fn get<T>(&self) -> Option<T>;
}

// After (2.0.0) - BREAKING: New required method
pub trait Resolver {
    fn get<T>(&self) -> Option<T>;
    fn get_required<T>(&self) -> T; // New required method
}
```

#### ❌ Breaking: Struct Field Removal
```rust
// Before (1.x.x)
pub struct ServiceDescriptor {
    pub service_type: TypeId,
    pub lifetime: Lifetime,
    pub factory: Box<dyn Factory>, // Removed in 2.0.0
}

// After (2.0.0) - BREAKING: Field removed
pub struct ServiceDescriptor {
    pub service_type: TypeId,
    pub lifetime: Lifetime,
}
```

## Non-Breaking Changes

### Acceptable Additions
- Adding new public functions, methods, or types
- Adding optional parameters with default values
- Adding trait methods with default implementations
- Adding new variants to non-exhaustive enums
- Adding new optional struct fields with defaults

### Acceptable Internal Changes
- Implementation optimizations that don't change API
- Internal refactoring that preserves public interface
- Bug fixes that correct documented behavior
- Documentation improvements

### Examples of Non-Breaking Changes

#### ✅ Non-Breaking: New Method Addition
```rust
// Before (1.2.x)
impl ServiceCollection {
    pub fn add_singleton<T>(&mut self, instance: T) { }
}

// After (1.3.0) - NON-BREAKING: New method added
impl ServiceCollection {
    pub fn add_singleton<T>(&mut self, instance: T) { }
    pub fn add_singleton_factory<T, F>(&mut self, factory: F) { } // New method
}
```

#### ✅ Non-Breaking: Trait Method with Default
```rust
// Before (1.2.x)
pub trait Resolver {
    fn get<T>(&self) -> Option<T>;
}

// After (1.3.0) - NON-BREAKING: Default implementation provided
pub trait Resolver {
    fn get<T>(&self) -> Option<T>;
    
    fn get_or_default<T: Default>(&self) -> T {  // New method with default
        self.get().unwrap_or_default()
    }
}
```

## Breaking Change Process

### 1. Impact Assessment
Before introducing breaking changes:

- **Necessity Evaluation**: Is the breaking change absolutely necessary?
- **Alternative Analysis**: Can the goal be achieved without breaking changes?
- **Ecosystem Impact**: How many downstream crates will be affected?
- **Migration Complexity**: How difficult will it be for users to migrate?

### 2. Community Consultation
For significant breaking changes:

- **RFC Process**: Create detailed RFC for major API changes
- **Community Discussion**: Open GitHub issue for feedback
- **Timeline Communication**: Provide clear timeline and rationale
- **Alternative Proposals**: Consider community suggestions

### 3. Deprecation Period
When possible, use deprecation before removal:

```rust
#[deprecated(
    since = "1.5.0",
    note = "Use `new_get_method()` instead. Will be removed in 2.0.0"
)]
pub fn old_get_method<T>(&self) -> Option<T> {
    // Delegate to new method or provide shim implementation
    self.new_get_method().ok()
}
```

#### Deprecation Requirements
- **Minimum Period**: 2 minor versions (e.g., 1.3.0 → 1.5.0 → 2.0.0)
- **Clear Message**: Explain what to use instead
- **Migration Path**: Provide working alternative
- **Documentation**: Update docs with migration instructions

### 4. Migration Guide Creation
For each breaking change, provide:

- **Clear rationale**: Why the change was necessary
- **Step-by-step migration**: How to update existing code
- **Code examples**: Before and after code samples
- **Common pitfalls**: What to watch out for during migration
- **Automated tools**: Scripts or tools to help migration when possible

## Communication Strategy

### Advance Notice
- **Major Breaking Changes**: 60 days advance notice
- **Minor Breaking Changes**: 30 days advance notice
- **Emergency Fixes**: As soon as possible with explanation

### Communication Channels
- **GitHub Issues**: For discussion and feedback
- **Release Notes**: Prominent breaking change section
- **Blog Posts**: For major version releases
- **Documentation**: Updated migration guides
- **Cargo.toml**: Clear version requirements

### Release Notes Format
```markdown
## Breaking Changes 🚨

### Removed APIs
- `ServiceCollection::old_method()` - Use `new_method()` instead
- `Resolver::deprecated_get()` - Use `get_required()` or `get_optional()`

### Changed APIs
- `get<T>()` now returns `DiResult<T>` instead of `Option<T>`
  - Migration: Change `.get().unwrap()` to `.get_required()`
  - Migration: Change `.get()` to `.get().ok()`

### Behavioral Changes
- Error handling is now more strict for circular dependencies
- Scope disposal order changed to LIFO for consistency
```

## Backwards Compatibility

### Source Compatibility
Code that compiled with version X.Y.Z should compile with:
- Any version X.Y.Z+n (patch updates)
- Any version X.Y+n.0 (minor updates)
- Should NOT compile with version X+1.0.0 without changes (major updates)

### Binary Compatibility
Within the same major version:
- ABI stability is NOT guaranteed (Rust doesn't provide ABI stability)
- Users should recompile dependencies when updating ferrous-di
- Use specific version pinning for production deployments

### Runtime Compatibility
- Behavior changes only in major versions
- Performance improvements allowed in minor versions
- Bug fixes may change behavior if previous behavior was incorrect

## Special Cases and Exceptions

### Security Fixes
Security vulnerabilities may require immediate breaking changes:
- **Patch Release**: If fix can be made backwards compatible
- **Minor Release**: If breaking change is minimal and well-contained
- **Emergency Major**: For severe vulnerabilities requiring API changes

### MSRV (Minimum Supported Rust Version)
MSRV updates follow special rules:
- **Patch**: Bug fixes in Rust toolchain (1.70.0 → 1.70.5)
- **Minor**: Same minor Rust version with new features (1.70.0 → 1.70.8)
- **Major**: New minor Rust version (1.70.x → 1.75.x)

### Performance Regressions
Significant performance regressions (>25%) are considered breaking:
- Require justification and migration advice
- May warrant major version bump if unavoidable
- Should include performance optimization recommendations

## Migration Tools and Automation

### Automated Migration
Where possible, provide tools to automate migration:

```bash
# Example migration script
cargo install ferrous-di-migrate
ferrous-di-migrate --from 1.x --to 2.0 ./src
```

### IDE Support
- Rust Analyzer hints for deprecated APIs
- Quick-fix suggestions for common migrations
- Documentation links to migration guides

### Cargo Integration
```toml
# Cargo.toml with clear version requirements
[dependencies]
ferrous-di = { version = "2.0", features = ["std"] }

# Version with migration window
ferrous-di = { version = ">=1.8, <3.0" }
```

## Breaking Change Categories

### High Impact Changes
Require major version bump and extensive communication:
- Core trait modifications (`Resolver`, `ServiceCollection`)
- Primary API signature changes
- Fundamental behavior changes
- Error handling model changes

### Medium Impact Changes
May be introduced with careful deprecation:
- Secondary API modifications
- Optional feature changes
- Performance characteristic changes
- Advanced feature modifications

### Low Impact Changes
Can often be introduced gradually:
- Internal implementation changes
- Diagnostic improvements
- Optional dependency updates
- Documentation restructuring

## Quality Gates for Breaking Changes

### Pre-Implementation
- [ ] Breaking change necessity justified
- [ ] Alternative non-breaking approaches evaluated
- [ ] Community consultation completed
- [ ] Migration strategy defined

### Implementation
- [ ] Deprecation period implemented (when applicable)
- [ ] Migration guide written
- [ ] Automated migration tools created (when possible)
- [ ] Documentation updated

### Pre-Release
- [ ] Community testing completed
- [ ] Migration guide validated
- [ ] Release notes finalized
- [ ] Support channels prepared

### Post-Release
- [ ] Community support provided
- [ ] Migration issues addressed
- [ ] Feedback incorporated for future releases
- [ ] Lessons learned documented

## Examples of Well-Managed Breaking Changes

### Case Study 1: Error Handling Improvement
**Problem**: Original API used `Option<T>` for all operations, making error diagnosis difficult.

**Solution**:
1. **1.3.0**: Introduced `get_result<T>() -> DiResult<T>` alongside existing `get<T>() -> Option<T>`
2. **1.4.0**: Deprecated `get<T>()` with clear migration message
3. **1.5.0**: Added more comprehensive error types and messages
4. **2.0.0**: Removed `get<T>()`, renamed `get_result<T>()` to `get<T>()`

**Migration**:
```rust
// Before (1.x)
if let Some(service) = provider.get::<MyService>() {
    // use service
}

// After (2.0)
match provider.get::<MyService>() {
    Ok(service) => {
        // use service
    }
    Err(e) => {
        eprintln!("Failed to resolve MyService: {}", e);
    }
}
```

### Case Study 2: Trait Method Addition
**Problem**: Need to add lifecycle hooks to `Resolver` trait.

**Solution**:
1. **1.3.0**: Added `ResolverExt` trait with new methods and blanket implementation
2. **1.4.0**: Moved methods to main `Resolver` trait with default implementations
3. **1.5.0**: Deprecated `ResolverExt`, encouraged direct usage
4. **2.0.0**: Removed `ResolverExt`, kept methods in main trait

This approach allowed gradual adoption without breaking existing implementations.

## Continuous Improvement

### Metrics and Monitoring
- Track breaking change frequency
- Monitor community feedback on breaking changes
- Measure migration guide effectiveness
- Analyze support request patterns

### Policy Updates
This breaking changes policy is itself subject to improvement:
- Annual review of policy effectiveness
- Community feedback on policy clarity
- Adaptation based on ecosystem evolution
- Alignment with Rust community best practices

The goal is to minimize breaking changes while allowing necessary evolution, always with clear communication and comprehensive migration support.