# Security Policy

## Supported Versions

We provide security updates for the following versions of Ferrous DI:

| Version | Supported          |
| ------- | ------------------ |
| 0.1.x   | :white_check_mark: |

As the project is in early development, we currently only support the latest version. Once we reach 1.0, we will provide security updates for at least the current major version.

## Reporting a Vulnerability

We take security vulnerabilities seriously. If you discover a security vulnerability in Ferrous DI, please report it privately using one of the following methods:

### Preferred: GitHub Security Advisories
1. Go to the [Security Advisories](https://github.com/s1ntropy/ferrous-di/security/advisories) page
2. Click "Report a vulnerability"
3. Fill out the form with details about the vulnerability
4. Submit the report

### Alternative: Email
Send an email to: **security@s1ntropy.dev** (replace with actual email)

### What to Include

Please include the following information in your report:

- **Description**: Clear description of the vulnerability
- **Impact**: What can an attacker do? What is compromised?
- **Reproduction**: Step-by-step instructions to reproduce the issue
- **Environment**: Rust version, OS, and any relevant configuration
- **Proof of Concept**: Code or commands that demonstrate the vulnerability
- **Suggested Fix**: If you have ideas for how to fix it

### Example Report

```
Subject: Security Vulnerability in Ferrous DI - Memory Safety Issue

Description:
Found a memory safety issue in the service resolution path that could lead to 
use-after-free when using scoped services with circular dependencies.

Impact: 
- Potential memory corruption
- Possible arbitrary code execution in unsafe configurations
- DoS through segmentation fault

Reproduction:
1. Create two scoped services with circular dependencies
2. Dispose the scope while resolution is in progress
3. Access the resolved service after disposal

Environment:
- Rust 1.75.0
- Ubuntu 22.04
- Ferrous DI 0.1.0

Proof of Concept:
[Attach minimal code example]

Suggested Fix:
Add proper lifetime tracking to prevent access after disposal
```

## Response Process

### Timeline

- **24 hours**: Initial acknowledgment of the report
- **72 hours**: Initial assessment and triage
- **7 days**: Detailed response with fix timeline
- **30 days**: Security patch release (target)

### Triage Process

1. **Acknowledge**: We confirm receipt of your report
2. **Validate**: We reproduce and confirm the vulnerability  
3. **Assess**: We determine the severity and impact
4. **Fix**: We develop and test a patch
5. **Coordinate**: We coordinate disclosure and release
6. **Publish**: We release the fix and publish an advisory

### Severity Levels

We use the following severity levels based on impact and exploitability:

#### Critical
- Remote code execution
- Privilege escalation
- Memory corruption with high exploitability
- Data corruption or loss

#### High  
- Local code execution
- Significant data exposure
- Memory corruption with low exploitability
- DoS affecting availability

#### Medium
- Information disclosure
- Local privilege escalation requiring user interaction
- Resource exhaustion
- Logic errors with security implications

#### Low
- Minor information disclosure
- Issues requiring significant user interaction
- Theoretical vulnerabilities with no known exploit

## Security Considerations

### Dependency Injection Security

While dependency injection frameworks are generally safe, there are some security considerations specific to Ferrous DI:

#### Service Construction
- **User-provided factories**: Service factories run user code - ensure they're trustworthy
- **Deserialization**: Be cautious when deserializing data in service constructors
- **Resource limits**: Large object graphs could lead to resource exhaustion

#### Configuration Security  
- **Service metadata**: Don't include sensitive information in service metadata
- **Named services**: Service names are used as keys - avoid exposing sensitive data

#### Thread Safety
- **Shared state**: Services are shared via Arc<T> - ensure thread-safe implementations
- **Disposal**: Improper disposal could lead to resource leaks

### Safe Usage Patterns

```rust
// ✅ Good: Safe service with proper error handling
services.add_singleton_factory::<Database, _>(|resolver| {
    let config = resolver.get_required::<Config>();
    Database::connect(&config.url)
        .map_err(|e| format!("Failed to connect to database: {}", e))
});

// ❌ Risky: Panic in service constructor
services.add_singleton_factory::<Service, _>(|resolver| {
    let config = resolver.get_required::<Config>();
    Service::new(config.secret.unwrap()) // Could panic!
});

// ✅ Good: Proper error propagation  
services.add_singleton_factory::<Service, _>(|resolver| {
    let config = resolver.get_required::<Config>();
    let secret = config.secret
        .ok_or("Missing required secret configuration")?;
    Ok(Service::new(secret))
});
```

### Memory Safety

Ferrous DI is designed to be memory-safe by leveraging Rust's type system:

- **No unsafe code** in normal operation paths
- **Arc-based sharing** prevents use-after-free
- **Scope-based disposal** ensures proper cleanup
- **Circular detection** prevents infinite recursion

However, service implementations themselves must maintain memory safety.

## Security Best Practices

### For Users

1. **Validate service factories** - Ensure user-provided code is trustworthy
2. **Handle errors gracefully** - Don't panic in service constructors  
3. **Use scoped services carefully** - Ensure proper disposal in async contexts
4. **Monitor resource usage** - Large service graphs can consume significant memory
5. **Keep dependencies updated** - Regularly update Ferrous DI and dependencies

### For Contributors

1. **Security review required** - All PRs affecting security must be reviewed
2. **No unsafe code without justification** - Document any unsafe usage
3. **Test error paths** - Ensure error handling doesn't introduce vulnerabilities
4. **Fuzz critical paths** - Use fuzzing to test service resolution and disposal
5. **Static analysis** - Use clippy and other tools to catch potential issues

## Security Testing

### Automated Testing

We use several automated tools to ensure security:

- **Cargo Audit**: Scans dependencies for known vulnerabilities
- **Clippy**: Static analysis for common security pitfalls  
- **Miri**: Detects undefined behavior and memory safety issues
- **Address Sanitizer**: Runtime detection of memory errors

### Manual Testing

Security-focused manual testing includes:

- **Circular dependency edge cases**: Complex circular scenarios
- **Resource exhaustion**: Large service graphs and deep dependency chains
- **Concurrent access**: Multi-threaded service resolution and disposal
- **Error injection**: Simulating failures during service construction

### Fuzzing

We use fuzzing to test critical paths:

```bash
# Install cargo-fuzz
cargo install cargo-fuzz

# Run service resolution fuzzing
cargo fuzz run service_resolution

# Run disposal fuzzing  
cargo fuzz run scope_disposal
```

## Disclosure Policy

### Coordinated Disclosure

We follow responsible disclosure practices:

1. **Private disclosure**: Security issues are kept private until fixed
2. **Coordinated timeline**: We work with reporters to coordinate public disclosure
3. **Credit**: We publicly credit security researchers (if desired)
4. **Advisory publication**: We publish security advisories for confirmed issues

### Public Disclosure Timeline

- **Day 0**: Private report received
- **Day 7**: Initial assessment complete, fix development begins
- **Day 30**: Target date for patch release
- **Day 37**: Public advisory published (7 days after patch)

This timeline may be adjusted based on severity and complexity.

## Security Updates

### Notification

Security updates are announced through:

- **GitHub Security Advisories**: Primary notification method
- **Release notes**: Security fixes highlighted in CHANGELOG.md  
- **Crates.io**: Security releases marked appropriately

### Update Recommendations

- **Critical/High severity**: Update immediately
- **Medium severity**: Update within 30 days
- **Low severity**: Update at next convenient time

## Contact Information

- **Security issues**: security@s1ntropy.dev
- **General questions**: GitHub Issues or Discussions
- **Maintainer contact**: [Maintainer GitHub profiles]

## Acknowledgments

We thank the following security researchers for their contributions:

<!-- This section will be populated as we receive security reports -->

---

**Note**: This security policy is a living document and will be updated as the project evolves. Changes will be announced in release notes and commit messages.