# Contributing to Ferrous DI

Thank you for your interest in contributing to Ferrous DI! This document provides guidelines and information for contributors.

## Code of Conduct

Please read and follow our [Code of Conduct](CODE_OF_CONDUCT.md) to ensure a welcoming environment for all contributors.

## Getting Started

### Prerequisites

- Rust 1.70.0 or later (MSRV)
- Git
- Basic familiarity with dependency injection concepts

### Development Setup

1. **Fork and clone the repository**
   ```bash
   git clone https://github.com/yourusername/ferrous-di.git
   cd ferrous-di
   ```

2. **Install Rust toolchain**
   ```bash
   rustup update
   rustup component add rustfmt clippy
   ```

3. **Run tests to ensure everything works**
   ```bash
   cargo test
   cargo bench --no-run
   ```

## Development Workflow

### Before Making Changes

1. **Check existing issues and PRs** to avoid duplicate work
2. **Create an issue** for major changes to discuss the approach
3. **Create a feature branch** from main:
   ```bash
   git checkout -b feature/your-feature-name
   ```

### Making Changes

1. **Write tests first** (TDD approach recommended)
2. **Implement your changes**
3. **Update documentation** including doc comments and README if needed
4. **Ensure all checks pass**:
   ```bash
   # Format code
   cargo fmt

   # Check for linting issues
   cargo clippy --all-targets --all-features -- -D warnings

   # Run all tests
   cargo test

   # Test with different feature combinations
   cargo test --no-default-features
   cargo test --all-features

   # Check documentation builds
   cargo doc --no-deps --document-private-items
   ```

### Performance Considerations

Ferrous DI prioritizes performance. When making changes:

1. **Run benchmarks** to check for regressions:
   ```bash
   cargo bench --features performance
   ```

2. **Performance requirements**:
   - Singleton resolution: <100ns baseline
   - No unnecessary allocations in hot paths
   - Thread-safe operations with minimal contention

3. **Profile changes** if they affect hot paths:
   ```bash
   # Using perf (Linux)
   perf record --call-graph=dwarf cargo bench
   perf report
   ```

## Testing Guidelines

### Test Categories

1. **Unit tests** (in `src/lib.rs`):
   - Test individual functions and methods
   - Focus on edge cases and error conditions

2. **Integration tests** (in `tests/`):
   - `tests/basics.rs` - Core functionality
   - `tests/circular.rs` - Circular dependency detection
   - `tests/scopes.rs` - Scoped lifetime behavior
   - `tests/advanced_features.rs` - Named services, metadata
   - `tests/disposal.rs` - Resource cleanup
   - `tests/multi.rs` - Multi-binding scenarios

3. **Doc tests**:
   - All public APIs must have working examples
   - Examples should be realistic and demonstrate best practices

### Writing Good Tests

```rust
#[test]
fn test_descriptive_name() {
    // Arrange - Set up test data
    let mut sc = ServiceCollection::new();
    sc.add_singleton(42usize);
    
    // Act - Perform the operation
    let sp = sc.build();
    let result = sp.get::<usize>();
    
    // Assert - Verify the outcome
    assert!(result.is_ok());
    assert_eq!(*result.unwrap(), 42);
}
```

### Test Coverage

- Maintain >90% code coverage
- All public APIs must be tested
- Error paths and edge cases must be covered
- Performance-critical paths should have benchmarks

## Code Style

### Formatting

- Use `cargo fmt` for consistent formatting
- Line length: 100 characters (configured in `rustfmt.toml`)
- Use trailing commas in multi-line constructs

### Naming Conventions

- `snake_case` for functions, variables, modules
- `PascalCase` for types, traits, enums
- `SCREAMING_SNAKE_CASE` for constants
- Descriptive names over abbreviations

### Documentation

- All public APIs must have doc comments
- Include examples for non-trivial APIs:
  ```rust
  /// Resolves a service of type T from the container.
  ///
  /// # Examples
  ///
  /// ```rust
  /// # use ferrous_di::*;
  /// let mut services = ServiceCollection::new();
  /// services.add_singleton(42usize);
  /// 
  /// let provider = services.build();
  /// let number = provider.get_required::<usize>();
  /// assert_eq!(*number, 42);
  /// ```
  pub fn get<T>(&self) -> DiResult<Arc<T>> { ... }
  ```

## Architecture Guidelines

### Module Organization

- Keep modules focused and cohesive
- Prefer smaller, well-defined modules over large ones
- Use `pub(crate)` for internal APIs
- Document module purpose at the top of each file

### Error Handling

- Use `DiResult<T>` for all fallible operations
- Provide meaningful error messages
- Include context in error variants:
  ```rust
  return Err(DiError::NotFound {
      type_name: std::any::type_name::<T>(),
      context: "singleton resolution",
  });
  ```

### Performance

- Minimize allocations in hot paths
- Use `Arc<T>` for shared ownership
- Prefer lock-free operations where possible
- Profile before and after performance changes

## Pull Request Process

### Before Submitting

1. **Ensure all CI checks pass locally**
2. **Update CHANGELOG.md** following [Keep a Changelog](https://keepachangelog.com/)
3. **Update version** in `Cargo.toml` if needed (for maintainers)
4. **Write a clear PR description** explaining:
   - What problem does this solve?
   - How does it solve it?
   - Any breaking changes?
   - Performance impact?

### PR Template

```markdown
## Description
Brief description of changes

## Type of Change
- [ ] Bug fix (non-breaking change that fixes an issue)
- [ ] New feature (non-breaking change that adds functionality)
- [ ] Breaking change (fix or feature that breaks existing functionality)
- [ ] Documentation update

## Testing
- [ ] Added tests for new functionality
- [ ] All existing tests pass
- [ ] Benchmarks run without regression

## Checklist
- [ ] Code follows style guidelines
- [ ] Self-review completed
- [ ] Documentation updated
- [ ] No new clippy warnings
```

### Review Process

1. **Automated checks** must pass (CI, formatting, tests)
2. **Code review** by maintainer(s)
3. **Performance review** for changes affecting hot paths
4. **Documentation review** for public API changes

## Release Process

### Versioning

We follow [Semantic Versioning](https://semver.org/):
- **MAJOR**: Breaking changes
- **MINOR**: New features (backward compatible)
- **PATCH**: Bug fixes (backward compatible)

### Release Checklist

1. Update CHANGELOG.md with release notes
2. Update version in Cargo.toml
3. Create git tag: `git tag v1.2.3`
4. Push tag to trigger automated release: `git push origin v1.2.3`

## Getting Help

### Communication Channels

- **GitHub Issues**: Bug reports, feature requests
- **GitHub Discussions**: General questions, ideas
- **Email**: security@example.com (security issues only)

### Maintainer Response Times

- **Security issues**: Within 24 hours
- **Bug reports**: Within 1 week
- **Feature requests**: Within 2 weeks
- **PRs**: Within 1 week for initial feedback

## Types of Contributions

We welcome all types of contributions:

### Code Contributions
- Bug fixes
- Performance improvements
- New features (with prior discussion for large changes)
- Test coverage improvements

### Documentation
- API documentation improvements
- Examples and tutorials
- Architecture documentation
- Performance guides

### Community
- Answering questions in issues/discussions
- Improving error messages
- Code review feedback
- Blog posts and talks about Ferrous DI

## Recognition

Contributors will be:
- Listed in CHANGELOG.md for significant contributions
- Mentioned in release notes
- Added to a CONTRIBUTORS.md file (if they wish)

Thank you for contributing to Ferrous DI! 🦀