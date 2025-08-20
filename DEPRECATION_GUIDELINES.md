# Deprecation Guidelines

## Overview

This document establishes clear guidelines for deprecating APIs in ferrous-di, ensuring a smooth transition path for users while maintaining backwards compatibility during deprecation periods.

## Deprecation Policy

### When to Deprecate

#### Required Scenarios
- **API Redesign**: When a better API design is available
- **Security Issues**: When an API has security implications
- **Performance Problems**: When an API has fundamental performance issues
- **Maintenance Burden**: When an API is difficult to maintain or evolve
- **Ecosystem Alignment**: When Rust ecosystem patterns have evolved

#### Optional Scenarios
- **Naming Consistency**: Improving API naming for better consistency
- **Documentation Clarity**: When API names are confusing or misleading
- **Feature Consolidation**: When multiple APIs can be merged into one

### Deprecation Timeline

#### Standard Timeline
1. **Deprecation Release** (Version N): Mark as deprecated with migration path
2. **Warning Period** (Version N+1): Continue deprecation warnings
3. **Removal Release** (Version N+2 or next major): Remove deprecated API

#### Minimum Timeline
- **Minor Versions**: Minimum 2 minor versions before removal
- **Time Period**: Minimum 6 months between deprecation and removal
- **Major Versions**: Can only remove in major version releases

#### Extended Timeline for Core APIs
- **Core APIs**: Minimum 4 minor versions (12+ months)
- **Fundamental Changes**: May require longer deprecation periods
- **Community Impact**: Consider ecosystem-wide impact

### Example Timeline
```
1.3.0: Deprecate old_method(), introduce new_method()
1.4.0: Continue deprecation warnings, improve new_method()
1.5.0: Final deprecation warnings, migration tools available
2.0.0: Remove old_method(), keep new_method() only
```

## Deprecation Implementation

### Using Rust Attributes

#### Basic Deprecation
```rust
#[deprecated(since = "1.3.0", note = "Use `new_method()` instead")]
pub fn old_method(&self) -> Result<String> {
    // Implementation that delegates to new method when possible
    self.new_method()
}
```

#### Detailed Deprecation
```rust
#[deprecated(
    since = "1.3.0",
    note = "Use `get_required()` or `get_optional()` instead. \
            This method will be removed in 2.0.0. \
            See migration guide: https://docs.rs/ferrous-di/latest/ferrous_di/migration/"
)]
pub fn get<T: 'static>(&self) -> Option<Arc<T>> {
    self.get_optional().ok()
}
```

### Documentation Updates

#### Method Documentation
```rust
/// # Deprecated
/// 
/// This method is deprecated since version 1.3.0 and will be removed in 2.0.0.
/// 
/// ## Migration
/// 
/// ```rust
/// // Before (deprecated)
/// let service = provider.old_get::<MyService>();
/// 
/// // After (recommended)
/// let service = provider.get_required::<MyService>();
/// ```
/// 
/// ## Rationale
/// 
/// The old method had unclear error handling semantics. The new methods
/// provide explicit error handling with better diagnostics.
#[deprecated(since = "1.3.0", note = "Use `get_required()` or `get_optional()` instead")]
pub fn old_get<T: 'static>(&self) -> Option<Arc<T>> {
    // Implementation
}
```

#### Module-Level Documentation
```rust
//! # Deprecated Module
//! 
//! This module is deprecated as of version 1.3.0 and will be removed in 2.0.0.
//! 
//! Users should migrate to the new `resolver` module:
//! 
//! ```rust
//! // Before
//! use ferrous_di::old_module::Resolver;
//! 
//! // After  
//! use ferrous_di::resolver::Resolver;
//! ```

#[deprecated(since = "1.3.0", note = "Use `crate::resolver` instead")]
pub mod old_module {
    // Re-export new types for compatibility
    pub use crate::resolver::*;
}
```

## Migration Strategies

### Graceful Delegation
When possible, implement deprecated methods by delegating to new methods:

```rust
#[deprecated(since = "1.3.0", note = "Use `add_singleton_factory()` instead")]
pub fn add_singleton_with_factory<T, F>(&mut self, factory: F) 
where 
    F: Fn() -> T + Send + Sync + 'static,
    T: Send + Sync + 'static,
{
    // Delegate to new method with compatible interface
    self.add_singleton_factory(factory)
}
```

### Compatibility Wrappers
For more complex migrations, provide compatibility wrappers:

```rust
#[deprecated(since = "1.3.0", note = "Use `ServiceProvider::new()` instead")]
pub fn create_provider(collection: ServiceCollection) -> ServiceProvider {
    // Wrapper that maintains old behavior
    ServiceProvider::new(collection)
}
```

### Type Aliases
For renamed types, use type aliases:

```rust
#[deprecated(since = "1.3.0", note = "Use `ServiceProvider` instead")]
pub type Container = ServiceProvider;

#[deprecated(since = "1.3.0", note = "Use `DiError` instead")]  
pub type ContainerError = DiError;
```

## Communication Strategy

### Deprecation Announcements

#### Release Notes Format
```markdown
## Deprecations ⚠️

### Methods
- `ServiceCollection::old_method()` - Use `new_method()` instead
  - **Reason**: Improved error handling and performance
  - **Timeline**: Will be removed in version 2.0.0
  - **Migration**: See [migration guide](link)

### Types
- `OldType` - Use `NewType` instead
  - **Reason**: Better naming consistency with Rust conventions
  - **Timeline**: Will be removed in version 2.0.0
  - **Migration**: Simple rename, no functionality changes
```

#### Commit Message Format
```
feat: deprecate old_method() in favor of new_method()

The old_method() has unclear error semantics and poor performance
characteristics. The new_method() provides:

- Explicit error handling with DiResult<T>
- 40% better performance in benchmarks  
- Clearer documentation and examples

BREAKING CHANGE: old_method() will be removed in 2.0.0

Migration:
- Replace old_method() calls with new_method()
- Handle Result<T> return type appropriately
```

### Migration Documentation

#### Comprehensive Migration Guide
```markdown
# Migration Guide: old_method() → new_method()

## Overview
The `old_method()` is deprecated in favor of `new_method()` starting in version 1.3.0.

## Why the Change?
- **Performance**: new_method() is 40% faster
- **Error Handling**: Explicit error types instead of Option<T>
- **Consistency**: Aligns with other resolver methods

## Step-by-Step Migration

### 1. Simple Cases
```rust
// Before
if let Some(service) = provider.old_method::<MyService>() {
    // use service
}

// After
match provider.new_method::<MyService>() {
    Ok(service) => {
        // use service  
    }
    Err(e) => {
        // handle error
    }
}
```

### 2. Complex Cases
```rust
// Before
let services: Vec<_> = items
    .iter()
    .filter_map(|item| provider.old_method::<MyService>())
    .collect();

// After
let services: Result<Vec<_>, _> = items
    .iter()
    .map(|item| provider.new_method::<MyService>())
    .collect();
```

## Common Pitfalls
- Don't forget to handle the Result<T> return type
- Error handling is now explicit, not silent failures
- Performance sensitive code may see improvements

## Automated Migration
Use the migration tool for automatic conversion:
```bash
cargo install ferrous-di-migrate
ferrous-di-migrate old-method-to-new ./src
```
```

## Tooling and Automation

### Deprecation Detection
```rust
// Custom lint for internal development
#[cfg(test)]
mod deprecation_tests {
    use super::*;
    
    #[test]
    fn test_no_deprecated_api_usage() {
        // Compile-time check that internal code doesn't use deprecated APIs
        // This can be automated in CI
    }
}
```

### Migration Scripts
```bash
#!/bin/bash
# migrate-old-method.sh
# Automated migration script for old_method() → new_method()

find . -name "*.rs" -exec sed -i.bak 's/\.old_method(/\.new_method(/g' {} +
echo "Migration complete. Please review changes and handle Result<T> return types."
```

### IDE Integration
```json
// VSCode settings for deprecation warnings
{
    "rust-analyzer.diagnostics.disabled": [],
    "rust-analyzer.diagnostics.enable": true,
    "rust-analyzer.warnings.deprecated": "warn"
}
```

## Special Cases

### Feature-Gated APIs
```rust
#[cfg(feature = "async")]
#[deprecated(
    since = "1.3.0", 
    note = "Use `async_resolver` module instead. Feature 'async' will be removed in 2.0.0"
)]
pub mod old_async {
    pub use crate::async_resolver::*;
}
```

### Conditional Compilation
```rust
// For APIs that depend on specific Rust versions
#[cfg(not(feature = "modern-rust"))]
#[deprecated(since = "1.3.0", note = "Enable 'modern-rust' feature for better API")]
pub fn legacy_method(&self) -> OldResult {
    // Legacy implementation
}
```

### Generic Parameter Changes
```rust
// When changing generic constraints
#[deprecated(since = "1.3.0", note = "Use `new_generic_method()` with Send + Sync bounds")]
pub fn old_generic_method<T>(&self) -> Result<T> 
where 
    T: 'static,
{
    // Convert to new constraints when possible
    self.new_generic_method::<T>()
}

pub fn new_generic_method<T>(&self) -> Result<T>
where 
    T: Send + Sync + 'static,
{
    // New implementation with stricter bounds
}
```

## Quality Assurance

### Deprecation Checklist
- [ ] Deprecation attribute with clear message
- [ ] Updated documentation with migration path
- [ ] Implementation delegates to new API when possible
- [ ] Migration guide created or updated
- [ ] Release notes include deprecation notice
- [ ] CI tests verify deprecated API still works
- [ ] Performance impact assessed
- [ ] Community notification prepared

### Testing Deprecated APIs
```rust
#[cfg(test)]
mod deprecation_tests {
    use super::*;
    
    #[test]
    #[allow(deprecated)]
    fn deprecated_method_still_works() {
        let provider = create_test_provider();
        let result = provider.old_method::<TestService>();
        assert!(result.is_some());
    }
    
    #[test]
    fn new_method_equivalent_behavior() {
        let provider = create_test_provider();
        
        #[allow(deprecated)]
        let old_result = provider.old_method::<TestService>();
        let new_result = provider.new_method::<TestService>();
        
        // Verify equivalent behavior
        assert_eq!(old_result.is_some(), new_result.is_ok());
    }
}
```

### Documentation Testing
```rust
/// Example demonstrating migration:
/// 
/// ```rust,should_panic
/// # use ferrous_di::*;
/// # let provider = ServiceCollection::new().build();
/// // This will show deprecation warning
/// let service = provider.old_method::<String>();
/// ```
/// 
/// ```rust
/// # use ferrous_di::*;
/// # let provider = ServiceCollection::new().build();
/// // Preferred new approach
/// match provider.new_method::<String>() {
///     Ok(service) => println!("Got service"),
///     Err(e) => println!("Error: {}", e),
/// }
/// ```
#[deprecated(since = "1.3.0", note = "Use new_method() instead")]
pub fn old_method<T>(&self) -> Option<T> {
    self.new_method().ok()
}
```

## Monitoring and Metrics

### Deprecation Usage Tracking
- Monitor crates.io downloads by version to estimate migration progress
- Track GitHub issues related to migration difficulties
- Survey community about deprecation process effectiveness
- Analyze compilation warning patterns in popular dependents

### Success Criteria
- [ ] Smooth migration path exists for all deprecated APIs
- [ ] Community feedback is largely positive
- [ ] Migration tools work effectively
- [ ] Documentation is clear and complete
- [ ] Timeline is respected without rushing users

This deprecation process ensures that API evolution can happen smoothly while maintaining trust and usability for the ferrous-di community.