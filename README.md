# Ferrous DI

An enterprise-grade, type-safe dependency injection framework for Rust with advanced features including async support, AOP patterns, module systems, and comprehensive reliability mechanisms.

[![Crates.io](https://img.shields.io/crates/v/ferrous-di)](https://crates.io/crates/ferrous-di)
[![Documentation](https://docs.rs/ferrous-di/badge.svg)](https://docs.rs/ferrous-di)
[![License](https://img.shields.io/crates/l/ferrous-di)](LICENSE)
[![Build Status](https://github.com/s1ntropy/ferrous-di/workflows/CI/badge.svg)](https://github.com/s1ntropy/ferrous-di/actions)
[![Coverage](https://codecov.io/gh/s1ntropy/ferrous-di/branch/main/graph/badge.svg)](https://codecov.io/gh/s1ntropy/ferrous-di)
[![Security Audit](https://github.com/s1ntropy/ferrous-di/workflows/Security%20Audit/badge.svg)](https://github.com/s1ntropy/ferrous-di/actions)

## Features

### Core DI Features
- **Type-safe**: Full compile-time type checking with zero runtime reflection
- **High Performance**: ~78ns singleton resolution, O(1) service lookups
- **Thread-safe**: All APIs are `Send + Sync` with lock-free hot paths
- **Service Lifetimes**: Singleton, Scoped, and Transient with proper isolation
- **Trait Support**: Single bindings and multi-bindings for trait objects
- **Circular Detection**: Comprehensive cycle detection with detailed error paths
- **Memory Safe**: Arc-based sharing with automatic cleanup

### Advanced Features
- **Async Support**: Async factories, lifecycle management, and disposal
- **AOP (Aspect-Oriented Programming)**: Method interception and decoration
- **Module System**: Hierarchical service organization and configuration
- **Diagnostics**: Service graph export, debugging tools, and telemetry
- **Reliability**: Circuit breakers, retries, and fault tolerance patterns
- **Web Integration**: Framework-agnostic patterns for web applications
- **Agent Architecture**: Durable agent patterns with state management
- **Performance Monitoring**: Built-in metrics and performance tracking

### Quality & Reliability
- **Professional Release Process**: Semantic versioning with migration guides
- **Comprehensive Testing**: 200+ tests including mutation and fuzz testing
- **Security Auditing**: Regular dependency and vulnerability scanning
- **Performance Benchmarking**: Automated regression detection
- **API Stability**: Multi-tier stability guarantees with clear deprecation policies

## Quick Start

Add ferrous-di to your `Cargo.toml`:

```toml
[dependencies]
ferrous-di = "0.1"

# With performance optimizations
ferrous-di = { version = "0.1", features = ["performance"] }

# With async support
ferrous-di = { version = "0.1", features = ["async"] }

# With diagnostics and debugging
ferrous-di = { version = "0.1", features = ["diagnostics"] }

# All features enabled
ferrous-di = { version = "0.1", features = ["performance", "async", "diagnostics"] }
```

### Basic Usage

```rust
use ferrous_di::{ServiceCollection, Lifetime};
use std::sync::Arc;

// Define your services
struct Database {
    connection_string: String,
}

struct UserService {
    db: Arc<Database>,
}

trait Logger: Send + Sync {
    fn log(&self, message: &str);
}

struct ConsoleLogger;
impl Logger for ConsoleLogger {
    fn log(&self, message: &str) {
        println!("[LOG] {}", message);
    }
}

// Configure services
let mut services = ServiceCollection::new();

// Register singleton
services.add_singleton(Database {
    connection_string: "postgresql://localhost".to_string(),
});

// Register with factory
services.add_scoped_factory::<UserService, _>(|resolver| {
    UserService {
        db: resolver.get_required::<Database>(),
    }
});

// Register trait
services.add_singleton_trait::<dyn Logger, _>(ConsoleLogger);

// Build provider
let provider = services.build();

// Resolve services
let db = provider.get_required::<Database>();
let logger = provider.get_required_trait::<dyn Logger>();

// Create scope for scoped services
let scope = provider.create_scope();
let user_service = scope.get_required::<UserService>();
```

## Service Lifetimes

### Singleton
Single instance per root provider, cached forever:

```rust
services.add_singleton(ExpensiveResource::new());
services.add_singleton_factory::<Database, _>(|_| {
    Database::connect("postgresql://localhost")
});
```

### Scoped
Single instance per scope, cached for scope lifetime:

```rust
services.add_scoped_factory::<RequestContext, _>(|_| {
    RequestContext::new(generate_request_id())
});

let scope = provider.create_scope();
let ctx1 = scope.get_required::<RequestContext>(); // Creates new
let ctx2 = scope.get_required::<RequestContext>(); // Returns same instance
```

### Transient
New instance per resolution, never cached:

```rust
services.add_transient_factory::<Command, _>(|resolver| {
    Command::new(resolver.get_required::<Database>())
});
```

## Trait Support

### Single Binding (Replace Semantics)
```rust
trait EmailService: Send + Sync {
    fn send(&self, to: &str, subject: &str, body: &str);
}

struct SmtpEmailService;
impl EmailService for SmtpEmailService {
    fn send(&self, to: &str, subject: &str, body: &str) {
        // SMTP implementation
    }
}

// Register trait implementation
services.add_singleton_trait::<dyn EmailService, _>(SmtpEmailService);

// Resolve single implementation
let email_service = provider.get_required_trait::<dyn EmailService>();
```

### Multi-Binding (Append Semantics)
```rust
trait Plugin: Send + Sync {
    fn name(&self) -> &str;
    fn execute(&self);
}

// Register multiple implementations
services.add_trait_implementation::<dyn Plugin, _>(AuthPlugin, Lifetime::Singleton);
services.add_trait_implementation::<dyn Plugin, _>(LoggingPlugin, Lifetime::Singleton);
services.add_trait_implementation::<dyn Plugin, _>(MetricsPlugin, Lifetime::Transient);

// Resolve all implementations
let plugins = provider.get_all_trait::<dyn Plugin>().unwrap();
for plugin in plugins {
    plugin.execute();
}
```

## Error Handling

Ferrous DI provides detailed error information:

```rust
use ferrous_di::DiError;

match provider.get::<UnregisteredService>() {
    Ok(service) => { /* use service */ }
    Err(DiError::NotFound(name)) => {
        eprintln!("Service not found: {}", name);
    }
    Err(DiError::Circular(path)) => {
        eprintln!("Circular dependency: {}", path.join(" -> "));
    }
    Err(DiError::WrongLifetime(msg)) => {
        eprintln!("Lifetime error: {}", msg);
    }
    Err(e) => {
        eprintln!("DI error: {}", e);
    }
}
```

For convenience, use the `get_required_*` methods that panic on error:

```rust
let service = provider.get_required::<MyService>(); // Panics if not found
let trait_service = provider.get_required_trait::<dyn MyTrait>();
```

## Advanced Examples

### Web Server with Request Scoping

See [examples/web_server_scope.rs](examples/web_server_scope.rs) for a complete example showing:

- Singleton services (database, configuration)
- Scoped services (request context, user session)
- Dependency injection in request handlers
- Proper scope isolation per HTTP request

```bash
cargo run --example web_server_scope
```

### Modular Service Registration

See [examples/modular_registration.rs](examples/modular_registration.rs) for advanced patterns:

```bash
cargo run --example modular_registration
```

### Durable Agent Architecture

See [examples/durable-agent/](examples/durable-agent/) for a complete agent system:

```bash
cd examples/durable-agent
cargo run
```

### Async Service Patterns

```rust
use ferrous_di::async_factories::AsyncFactory;

// Async service factory
struct DatabaseConnection;

#[async_trait]
impl AsyncFactory<DatabaseConnection> for DatabaseConnectionFactory {
    async fn create(&self, resolver: &dyn Resolver) -> DiResult<DatabaseConnection> {
        let config = resolver.get_required::<DatabaseConfig>();
        DatabaseConnection::connect(&config.url).await
    }
}

services.add_async_singleton_factory(DatabaseConnectionFactory);
```

### AOP and Service Decoration

```rust
use ferrous_di::decoration::ServiceDecorator;

// Logging decorator
struct LoggingDecorator<T> {
    inner: Arc<T>,
    logger: Arc<dyn Logger>,
}

impl<T: UserService> UserService for LoggingDecorator<T> {
    fn get_user(&self, id: UserId) -> Result<User, UserError> {
        self.logger.info(&format!("Getting user {}", id));
        let result = self.inner.get_user(id);
        match &result {
            Ok(_) => self.logger.info("User retrieved successfully"),
            Err(e) => self.logger.error(&format!("Failed to get user: {}", e)),
        }
        result
    }
}

services.add_decorator::<dyn UserService, _>(LoggingDecoratorFactory);
```

### Complex Dependency Graphs

```rust
struct Config { /* ... */ }
struct Database { config: Arc<Config> }
struct UserRepository { db: Arc<Database> }
struct UserService { repo: Arc<UserRepository> }

services.add_singleton(Config::load());
services.add_singleton_factory::<Database, _>(|r| {
    Database::new(r.get_required::<Config>())
});
services.add_scoped_factory::<UserRepository, _>(|r| {
    UserRepository::new(r.get_required::<Database>())
});
services.add_transient_factory::<UserService, _>(|r| {
    UserService::new(r.get_required::<UserRepository>())
});
```

### Module-Based Organization

```rust
use ferrous_di::collection::Module;

// Database module
struct DatabaseModule;
impl Module for DatabaseModule {
    fn configure(&self, services: &mut ServiceCollection) -> DiResult<()> {
        services.add_singleton_factory::<DatabasePool, _>(|r| {
            DatabasePool::new(r.get_required::<DatabaseConfig>())
        });
        services.add_scoped_factory::<UnitOfWork, _>(|r| {
            UnitOfWork::new(r.get_required::<DatabasePool>())
        });
        Ok(())
    }
}

// Application composition
let mut services = ServiceCollection::new();
services.add_module(DatabaseModule)?;
services.add_module(BusinessLogicModule)?;
services.add_module(WebModule)?;
```

## Performance

Ferrous DI is designed for high-performance dependency injection with enterprise-grade capabilities.

### Benchmark Results

Measured on Apple Silicon, compiled with `--release`:

| Operation | Time | Notes |
|-----------|------|-------|
| **Singleton hit** | ~78ns | Cached singleton resolution (hot path) |
| **Singleton cold** | ~437ns | First-time singleton creation with factory |
| **Scoped hit** | ~83ns | Cached scoped service within same scope |
| **Transient** | ~68ns | Fresh instance creation each time |
| **Concrete vs Trait** | ~86ns vs ~82ns | Minimal difference between concrete and trait |
| **Multi-binding (16 services)** | ~850ns | Resolving all 16 implementations |
| **Scope create/drop** | ~18ns | Empty scope lifecycle overhead |
| **Circular detection (depth 8)** | ~87ns | Deep dependency chain validation |
| **Using pattern (empty)** | ~152ns | Minimal overhead for scoped disposal |
| **Mixed workload** | ~872ns | Realistic 70/20/10 singleton/scoped/transient mix |

#### Contention Performance

| Threads | Time per Op | Throughput | Scaling |
|---------|------------|------------|---------|
| 1 thread | ~79ns | ~12.6M ops/sec | Baseline |
| 2 threads | ~155ns | ~12.9M ops/sec total | Good parallelization |
| 4 threads | ~206ns | ~19.4M ops/sec total | Continued scaling |
| 8 threads | ~168ns | ~47.6M ops/sec total | Excellent multi-core utilization |

### Performance Features

Enable high-performance optimizations with Cargo features:

```toml
[dependencies]
ferrous-di = { version = "0.1", features = ["performance"] }
```

This enables all performance features:

- **`parking-lot`**: Faster mutex implementation (2-3x faster locking)
- **`ahash`**: High-performance hashing algorithm 
- **`smallvec`**: Stack-allocated vectors for small circular detection stacks
- **`once-cell`**: Lock-free singleton caching (planned)

#### Individual Features

Enable features selectively if you prefer:

```toml
[dependencies]  
ferrous-di = { version = "0.1", features = ["parking-lot", "ahash"] }
```

### Reproduction Instructions

To reproduce benchmarks on your hardware:

```bash
# Clone the repository
git clone https://github.com/s1ntropy/ferrous-di
cd ferrous-di

# Run benchmarks with performance features
cargo bench --features performance

# Run specific benchmarks
cargo bench --features performance -- singleton_hit
cargo bench --features performance -- contention
cargo bench --features performance -- mixed_workload

# Generate HTML reports (if gnuplot available)
cargo bench --features performance -- --output-format html
```

### Performance Optimizations

The library implements several performance optimizations:

1. **TypeId-based lookups**: O(1) service resolution using TypeId hash keys
2. **Arc-based sharing**: Zero-copy service instance sharing
3. **Minimal allocations**: Pre-allocated vectors, stack-allocated small collections
4. **Lock optimization**: Parking-lot mutexes for reduced contention
5. **Hash optimization**: AHash for faster HashMap operations
6. **Hot path optimization**: Singleton resolution avoids locks when possible

### Release Profile

The library is optimized with aggressive release settings:

```toml
[profile.release]
opt-level = 3
lto = "fat"
codegen-units = 1
debug = false
```

This provides maximum performance for production use.

## Feature Flags

### Core Features

#### `std` (default)
Standard library support with std collections and threading:

```toml
[dependencies]
ferrous-di = "0.1"  # includes std by default
```

#### `async`
Enable async factories and lifecycle management:

```toml
[dependencies]
ferrous-di = { version = "0.1", features = ["async"] }
```

```rust
use ferrous_di::async_factories::AsyncFactory;

// Async service creation
services.add_async_singleton_factory(DatabaseConnectionFactory);
let connection = provider.get_async::<DatabaseConnection>().await?;
```

### Performance Features

#### `performance`
Enable all performance optimizations:

```toml
[dependencies]
ferrous-di = { version = "0.1", features = ["performance"] }
```

Individual performance features:
- **`parking-lot`**: Faster mutex implementation (2-3x faster locking)
- **`ahash`**: High-performance hashing algorithm 
- **`smallvec`**: Stack-allocated vectors for small collections
- **`once-cell`**: Lock-free singleton caching (experimental)

### Development Features

#### `diagnostics`
Enable comprehensive debugging and diagnostic tools:

```toml
[dependencies]
ferrous-di = { version = "0.1", features = ["diagnostics"] }
```

```rust
#[cfg(feature = "diagnostics")]
{
    // Service graph visualization
    let graph = provider.export_service_graph()?;
    println!("{}", graph.to_dot_format());
    
    // Debugging information
    println!("{}", provider.to_debug_string());
    
    // Performance metrics
    let metrics = provider.get_metrics();
    println!("Resolution time: {:?}", metrics.avg_resolution_time);
}
```

#### `validation`
Enable service validation and health checks:

```toml
[dependencies]
ferrous-di = { version = "0.1", features = ["validation"] }
```

### Integration Features

#### `web`
Web framework integration patterns:

```toml
[dependencies]
ferrous-di = { version = "0.1", features = ["web"] }
```

#### `metrics`
Built-in metrics and telemetry:

```toml
[dependencies]
ferrous-di = { version = "0.1", features = ["metrics"] }
```

### Experimental Features

#### `experimental`
Cutting-edge features under development:

```toml
[dependencies]
ferrous-di = { version = "0.1", features = ["experimental"] }
```

⚠️ **Note**: Experimental features may change or be removed in minor versions.

## Testing & Quality Assurance

### Running Tests

Run the complete test suite:

```bash
cargo test
```

Run specific test categories:

```bash
cargo test basics              # Basic functionality tests
cargo test scopes              # Scoped service tests  
cargo test circular            # Circular dependency tests
cargo test advanced_features   # Named services, metadata, TryAdd
cargo test agent_features      # Durable agent patterns
cargo test modules             # Module system tests
cargo test disposal            # Resource cleanup tests
```

### Quality Gates

#### Mutation Testing
Test the quality of your tests with mutation testing:

```bash
cargo install cargo-mutants
cargo mutants
```

#### Fuzzing
Run property-based and fuzz testing:

```bash
cargo install cargo-fuzz
cargo fuzz run dependency_injection
cargo fuzz run service_registration
cargo fuzz run service_resolution
```

#### Security Auditing
Scan for security vulnerabilities:

```bash
cargo audit
```

#### Performance Benchmarking
Run performance benchmarks:

```bash
cargo bench --features performance
```

#### Code Coverage
Generate code coverage reports:

```bash
cargo install cargo-tarpaulin
cargo tarpaulin --out html --features performance
```


## Development Roadmap

### Completed Features ✅
- **Phase 1**: Modular architecture with 25+ specialized modules
- **Phase 2**: Comprehensive CI/CD with multi-platform testing
- **Phase 3**: Complete documentation with examples and guides
- **Phase 4**: Advanced development tools and diagnostics
- **Phase 5**: Comprehensive testing (unit, integration, mutation, fuzz)
- **Phase 6**: Performance optimization and production readiness
- **Phase 7**: Professional release engineering process

### Current & Upcoming Features 🚧
- **Phase 8**: Advanced agent features with state persistence
- **Phase 9**: Ecosystem integration and framework adapters
- **Phase 10**: API stabilization and v1.0 preparation

### Version Releases
- **v0.1**: Core DI with lifetimes, traits, circular detection ✅
- **v0.2**: Async support, AOP patterns, module system
- **v0.3**: Advanced diagnostics, reliability patterns
- **v0.4**: Web integration, performance optimizations
- **v0.5**: Agent architecture, state management
- **v1.0**: Stable API, comprehensive ecosystem integration

### Long-term Vision
- **Enterprise Integration**: Support for complex enterprise patterns
- **Framework Ecosystem**: Deep integration with popular Rust frameworks
- **Cloud Native**: Kubernetes, service mesh, and cloud platform support
- **Tooling Ecosystem**: IDE extensions, debugging tools, and analysis

See [ROADMAP.md](ROADMAP.md) for detailed feature plans and timelines.

## Contributing

We welcome contributions from the community! This project follows professional development practices:

### Getting Started
1. Read [CONTRIBUTING.md](CONTRIBUTING.md) for detailed guidelines
2. Check [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md) for community standards
3. Review [ARCHITECTURE.md](ARCHITECTURE.md) to understand the codebase

### Development Process
- **Quality Gates**: All PRs must pass comprehensive testing and validation
- **Conventional Commits**: Follow conventional commit format for automatic changelog generation
- **API Stability**: Review [API_STABILITY.md](API_STABILITY.md) for compatibility requirements
- **Release Process**: Understand our professional release engineering in [RELEASE_CHECKLIST.md](RELEASE_CHECKLIST.md)

### Areas for Contribution
- 🐛 **Bug Fixes**: Help improve reliability and correctness
- 🚀 **Performance**: Optimize hot paths and memory usage
- 📚 **Documentation**: Improve examples, guides, and API docs
- 🧪 **Testing**: Add test coverage, property tests, or benchmarks
- 🔧 **Tooling**: IDE integration, debugging tools, or analysis
- 🌐 **Integration**: Framework adapters and ecosystem support

### Security
For security vulnerabilities, please see [SECURITY.md](SECURITY.md) for responsible disclosure.

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT License ([LICENSE-MIT](LICENSE-MIT))

at your option.

---

## Project Status

Ferrous DI is actively developed and maintained with professional development practices:

- **🏗️ Architecture**: Enterprise-grade modular design
- **🧪 Testing**: 200+ tests with comprehensive coverage
- **📚 Documentation**: Complete API docs and examples
- **🚀 Performance**: Production-ready with benchmarking
- **🔒 Security**: Regular auditing and vulnerability scanning
- **📋 Quality**: Professional release process with validation
- **🤝 Community**: Open source with contributor guidelines

**Ready for Production**: Suitable for enterprise applications requiring high performance, reliability, and maintainability.

For questions, issues, or contributions, visit our [GitHub repository](https://github.com/s1ntropy/ferrous-di).

## Architecture & Design Philosophy

### Comparison with Microsoft.Extensions.DependencyInjection

| Feature | Ferrous DI | MS.DI | Notes |
|---------|------------|-------|-------|
| **Type Safety** | Compile-time | Runtime | Zero-cost abstractions vs reflection |
| **Performance** | ~78ns resolution | ~1000ns+ | TypeId-based O(1) lookups |
| **Memory Safety** | Built-in Arc sharing | Manual lifecycle | Rust ownership + Arc |
| **Async Support** | Native async/await | Task-based | First-class async factories |
| **Lifetimes** | Singleton, Scoped, Transient | Same | Compatible lifecycle semantics |
| **Multi-binding** | Explicit append semantics | Implicit append | Clear intention |
| **Circular Detection** | Compile + Runtime | Runtime | Multi-layered protection |
| **Thread Safety** | Lock-free hot paths | Thread-safe | Optimized for concurrent access |
| **Modularity** | Hierarchical modules | Service collections | Advanced composition |
| **Diagnostics** | Built-in graph export | Limited | Rich debugging capabilities |
| **AOP Support** | Native decoration | Third-party | Built-in interception |
| **Reliability** | Circuit breakers, retries | Manual | Enterprise patterns |

### Design Principles

1. **Zero-Cost Abstractions**: Compile-time optimization with runtime efficiency
2. **Memory Safety**: Rust ownership system prevents common DI pitfalls
3. **Performance First**: Designed for high-throughput, low-latency applications
4. **Enterprise Ready**: Professional patterns for complex applications
5. **Developer Experience**: Rich diagnostics and clear error messages
6. **Ecosystem Integration**: Framework-agnostic with deep integration support

### Unique Features

- **Agent Architecture**: Durable agent patterns with state management
- **Module System**: Hierarchical service organization and configuration
- **Performance Monitoring**: Built-in metrics and telemetry
- **Reliability Patterns**: Circuit breakers, retries, and fault tolerance
- **Professional Release Process**: Enterprise-grade release engineering
- **Comprehensive Testing**: Mutation testing, fuzzing, and property-based testing