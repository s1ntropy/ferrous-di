# Migration Guide Template

This template provides a standardized format for creating migration guides when introducing breaking changes in ferrous-di. Copy this template and fill in the specific details for each migration scenario.

---

# Migration Guide: [From Version] → [To Version]

## Overview

**Migration Type**: [Patch/Minor/Major]  
**Release Date**: [Date]  
**Migration Difficulty**: [Low/Medium/High]  
**Estimated Time**: [X minutes/hours]

### Quick Summary
[One paragraph summary of what changed and why users need to migrate]

## What Changed?

### Breaking Changes
- [ ] **[Change 1]**: [Brief description]
- [ ] **[Change 2]**: [Brief description]
- [ ] **[Change 3]**: [Brief description]

### New Features
- **[Feature 1]**: [Brief description]
- **[Feature 2]**: [Brief description]

### Improvements
- **[Improvement 1]**: [Brief description]
- **[Improvement 2]**: [Brief description]

## Why These Changes?

### Rationale
[Explain why the breaking changes were necessary]
- **[Reason 1]**: [Detailed explanation]
- **[Reason 2]**: [Detailed explanation]
- **[Reason 3]**: [Detailed explanation]

### Benefits
[Explain what users gain from migrating]
- **Performance**: [Specific improvements]
- **Safety**: [Security/memory safety improvements]
- **Usability**: [API improvements]
- **Features**: [New capabilities]

## Migration Steps

### Step 1: Update Dependencies

#### Cargo.toml Changes
```toml
# Before
[dependencies]
ferrous-di = "X.Y"

# After
[dependencies]
ferrous-di = "X+1.Y"
```

#### Feature Flag Updates
```toml
# Before
ferrous-di = { version = "X.Y", features = ["old-feature"] }

# After
ferrous-di = { version = "X+1.Y", features = ["new-feature"] }
```

### Step 2: Core API Changes

#### [Specific Change Category]

**Before (X.Y):**
```rust
// Old code example
use ferrous_di::*;

let mut services = ServiceCollection::new();
services.old_method(parameter);
let provider = services.build();
let service = provider.old_get::<MyService>();
```

**After (X+1.Y):**
```rust
// New code example
use ferrous_di::*;

let mut services = ServiceCollection::new();
services.new_method(parameter)?;  // Note: now returns Result
let provider = services.build();
let service = provider.get_required::<MyService>()?;  // Note: explicit error handling
```

**Key Changes:**
- **Error Handling**: Methods now return `Result<T, DiError>` instead of `Option<T>`
- **Parameter Types**: [Describe parameter changes]
- **Return Types**: [Describe return type changes]

#### [Another Change Category]

**Before:**
```rust
// Another old pattern
```

**After:**
```rust
// New pattern with explanation
```

### Step 3: Advanced Features

#### [Advanced Feature 1]
[For users using advanced features]

**Migration:**
```rust
// Before
advanced_old_api(params);

// After
advanced_new_api(params).await?;  // Note: now async
```

#### [Advanced Feature 2]
[For users using other advanced features]

### Step 4: Testing Changes

#### Test Code Updates
```rust
// Before - test code
#[test]
fn test_old_behavior() {
    let provider = create_provider();
    assert!(provider.old_method().is_some());
}

// After - updated test code
#[test]
fn test_new_behavior() {
    let provider = create_provider();
    assert!(provider.new_method().is_ok());
}
```

#### New Testing Patterns
[Describe any new testing approaches or utilities]

## Common Migration Patterns

### Pattern 1: Optional to Result Conversion

**Before:**
```rust
if let Some(service) = provider.get::<MyService>() {
    service.do_something();
} else {
    handle_missing_service();
}
```

**After:**
```rust
match provider.get::<MyService>() {
    Ok(service) => service.do_something(),
    Err(DiError::ServiceNotFound(_)) => handle_missing_service(),
    Err(e) => handle_other_error(e),
}
```

### Pattern 2: [Another Common Pattern]

**Before:**
```rust
// Old pattern
```

**After:**
```rust
// New pattern
```

## Automated Migration

### Migration Script
```bash
#!/bin/bash
# automated-migration.sh
# This script helps automate common migration tasks

echo "Starting ferrous-di migration from X.Y to X+1.Y..."

# Update Cargo.toml
sed -i.bak 's/ferrous-di = "X\.Y"/ferrous-di = "X+1.Y"/' Cargo.toml

# Update common method names
find . -name "*.rs" -exec sed -i.bak 's/\.old_method(/\.new_method(/g' {} +

# Add error handling (manual review required)
echo "⚠️  Manual review required for error handling changes"
echo "   Look for .unwrap() calls that may need proper error handling"

echo "✅ Automated migration complete. Please review changes and test thoroughly."
```

### IDE Migration Support
[Instructions for using IDE refactoring tools, if available]

## Manual Review Required

### Areas Requiring Manual Attention
- [ ] **Error Handling**: Review all `unwrap()` calls and add proper error handling
- [ ] **Performance**: Check if new async APIs require runtime changes  
- [ ] **Testing**: Update test assertions for new error types
- [ ] **Dependencies**: Ensure dependent crates are compatible

### Code Review Checklist
- [ ] All compilation errors resolved
- [ ] All tests pass
- [ ] Error handling is comprehensive
- [ ] Performance impact is acceptable
- [ ] Documentation is updated

## Troubleshooting

### Common Issues

#### Issue 1: Compilation Errors
**Error:**
```
error[E0599]: no method named `old_method` found for type `ServiceCollection`
```

**Solution:**
```rust
// Replace old_method with new_method
services.new_method(params)?;
```

#### Issue 2: [Another Common Issue]
**Error:**
```
[Error message]
```

**Solution:**
```rust
// Solution code
```

### Getting Help

#### Community Resources
- **GitHub Issues**: [Link to issues]
- **Discord/Forum**: [Link to community]
- **Stack Overflow**: Tag `ferrous-di`

#### Documentation
- **API Docs**: [Link to docs.rs]
- **Examples**: [Link to examples]
- **Migration FAQ**: [Link to FAQ]

## Performance Impact

### Benchmarks
| Operation | Before (X.Y) | After (X+1.Y) | Change |
|-----------|--------------|---------------|---------|
| Service Resolution | 78ns | 65ns | +17% faster |
| Scope Creation | 1.2μs | 1.0μs | +17% faster |
| Memory Usage | 100KB | 95KB | -5% |

### Performance Notes
- **Improvements**: [List performance improvements]
- **Regressions**: [List any regressions and workarounds]
- **Recommendations**: [Performance optimization tips]

## Testing Your Migration

### Validation Steps
1. **Compile Test**: Ensure all code compiles without warnings
2. **Unit Tests**: Run existing unit tests to verify behavior
3. **Integration Tests**: Test with real-world scenarios
4. **Performance Tests**: Benchmark critical paths
5. **Memory Tests**: Check for memory leaks or excessive usage

### Test Scenarios
```rust
#[cfg(test)]
mod migration_tests {
    use super::*;
    
    #[test]
    fn test_basic_migration() {
        // Test that basic functionality works after migration
        let mut services = ServiceCollection::new();
        services.add_singleton(TestService::new());
        let provider = services.build();
        
        let service = provider.get_required::<TestService>().unwrap();
        assert!(service.is_working());
    }
    
    #[test]
    fn test_error_handling_migration() {
        // Test that error handling works correctly
        let provider = ServiceCollection::new().build();
        
        match provider.get::<MissingService>() {
            Err(DiError::ServiceNotFound(_)) => {
                // Expected behavior
            }
            _ => panic!("Expected ServiceNotFound error"),
        }
    }
}
```

## Real-World Examples

### Example 1: Web Service Migration
```rust
// Before - Web service setup
fn setup_web_services() -> ServiceProvider {
    let mut services = ServiceCollection::new();
    services.add_singleton(DatabaseConnection::new());
    services.add_scoped(UserService::new);
    services.build()
}

// After - Updated web service setup
fn setup_web_services() -> Result<ServiceProvider, DiError> {
    let mut services = ServiceCollection::new();
    services.add_singleton(DatabaseConnection::new())?;
    services.add_scoped_factory::<UserService, _>(|resolver| {
        UserService::new(resolver.get_required::<DatabaseConnection>())
    })?;
    Ok(services.build())
}
```

### Example 2: [Another Real-World Example]
[Provide another concrete example from a common use case]

## FAQ

### Q: Do I need to migrate immediately?
**A:** [Answer about urgency and timeline]

### Q: Will my existing code continue to work?
**A:** [Answer about backwards compatibility]

### Q: What if I encounter issues during migration?
**A:** [Answer about support and troubleshooting]

### Q: How long does migration typically take?
**A:** [Answer with time estimates]

### Q: Are there any breaking changes I should be especially careful about?
**A:** [Highlight the most important breaking changes]

## Feedback and Improvements

### Help Us Improve
We value your feedback on this migration process:
- **Migration Difficulty**: Was this guide helpful?
- **Missing Information**: What could we add?
- **Automation Opportunities**: What could be automated better?

### Contact
- **GitHub Issues**: [Report migration problems]
- **Community Discussion**: [Share feedback]
- **Direct Contact**: [For urgent migration support]

---

## Migration Completion Checklist

- [ ] Dependencies updated in Cargo.toml
- [ ] All compilation errors resolved
- [ ] Error handling patterns updated
- [ ] Tests updated and passing
- [ ] Performance validated
- [ ] Documentation updated
- [ ] Code review completed
- [ ] Production deployment tested

**Migration Complete!** 🎉

---

*This migration guide was created on [Date] for ferrous-di version [Version]. For the most up-to-date information, visit [Documentation Link].*