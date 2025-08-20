# Ferrous DI Architecture

This document describes the architecture, design decisions, and patterns used in Ferrous DI.

## Table of Contents

- [Overview](#overview)
- [Core Concepts](#core-concepts)
- [Module Architecture](#module-architecture)
- [Type System](#type-system)
- [Lifetimes and Scoping](#lifetimes-and-scoping)
- [Performance Design](#performance-design)
- [Error Handling](#error-handling)
- [Thread Safety](#thread-safety)
- [Design Patterns](#design-patterns)
- [Trade-offs and Limitations](#trade-offs-and-limitations)

## Overview

Ferrous DI is designed as a **performance-first**, **type-safe** dependency injection container for Rust. The architecture prioritizes:

1. **Zero-cost abstractions** - No runtime overhead for type resolution
2. **Compile-time safety** - Leverage Rust's type system for correctness
3. **Thread safety** - Safe concurrent access without locks in hot paths
4. **Memory efficiency** - Minimal allocations, Arc-based sharing
5. **Modularity** - Clean separation of concerns

## Core Concepts

### Service Container Pattern

Ferrous DI implements the **Service Container** pattern with three main components:

```rust
ServiceCollection  →  ServiceProvider  →  Scope
     (Config)           (Resolution)      (Lifetime)
```

- **ServiceCollection**: Mutable builder for service registrations
- **ServiceProvider**: Immutable resolver for singleton/transient services  
- **Scope**: Bounded context for scoped services with automatic disposal

### Type Erasure with Safety

Services are stored using **type erasure** but resolved with **compile-time safety**:

```rust
// Internal storage (type-erased)
type AnyArc = Arc<dyn Any + Send + Sync>;

// External API (type-safe)  
pub fn get<T: 'static>(&self) -> DiResult<Arc<T>>
```

This allows heterogeneous service storage while maintaining type safety at the API level.

## Module Architecture

### Layered Architecture

```
┌─────────────────────────────────────┐
│            Public API               │ ← lib.rs
│  ServiceCollection, ServiceProvider │
├─────────────────────────────────────┤
│             Core Types              │ ← error.rs, lifetime.rs  
│        DiError, Lifetime            │   key.rs, descriptors.rs
├─────────────────────────────────────┤
│            Traits Layer             │ ← traits/
│     Resolver, Dispose, etc.         │
├─────────────────────────────────────┤
│          Internal Systems           │ ← internal/
│    Circular Detection, Disposal     │
└─────────────────────────────────────┘
```

### Module Responsibilities

| Module | Responsibility | Lines | Key Types |
|--------|---------------|-------|-----------|
| `lib.rs` | Public API orchestration | 2821 | ServiceCollection, ServiceProvider, Scope |
| `traits/resolver.rs` | Resolution interfaces | 475 | Resolver, ResolverCore |
| `registration.rs` | Service storage | 85 | Registration, Registry |
| `internal/circular.rs` | Dependency cycle detection | 104 | StackGuard, CircularPanic |
| `key.rs` | Service identification | 95 | Key enum |
| `error.rs` | Error types | 45 | DiError, DiResult |

### Dependency Graph

```
lib.rs
├── traits/resolver.rs
├── registration.rs  
├── internal/circular.rs
├── internal/dispose_bag.rs
├── descriptors.rs
├── lifetime.rs
├── key.rs
└── error.rs
```

Clean dependencies with no circular imports.

## Type System

### Service Keys

Services are identified by a `Key` enum that handles both named and unnamed services:

```rust
pub enum Key {
    // Concrete types
    Type(TypeId, &'static str),                    // MyService
    TypeNamed(TypeId, &'static str, &'static str), // MyService("name")
    
    // Trait objects  
    Trait(&'static str),                           // dyn MyTrait
    TraitNamed(&'static str, &'static str),        // dyn MyTrait("name")
    
    // Multi-bindings
    MultiTrait(&'static str, usize),               // dyn MyTrait[0]
    MultiTraitNamed(&'static str, &'static str, usize), // dyn MyTrait("name")[0]
}
```

This design allows:
- **Type-safe resolution** using `TypeId`
- **Named services** for multiple implementations
- **Trait object support** with string-based keys
- **Multi-binding** with index-based disambiguation

### Generic Resolution

The resolver uses **generic methods** for type-safe access:

```rust
impl<T: 'static + Send + Sync> Resolver for ServiceProvider {
    fn get<U>(&self) -> DiResult<Arc<U>>
    where 
        U: 'static + Send + Sync 
    {
        let key = Key::Type(TypeId::of::<U>(), type_name::<U>());
        self.resolve_by_key(key)?.downcast()
    }
}
```

The compiler ensures type safety while allowing runtime service lookup.

## Lifetimes and Scoping

### Lifetime Hierarchy

```
ServiceProvider (Root)
├── Singleton Services    ← Shared across entire application
├── Transient Services    ← New instance per resolution
└── Scope 1, 2, 3...     ← Bounded contexts
    └── Scoped Services  ← Shared within scope, disposed with scope
```

### Scoped Service Implementation

```rust
pub struct Scope {
    provider: Arc<ServiceProvider>,        // Reference to root
    scoped_services: Mutex<HashMap<...>>,  // Scope-local services  
    dispose_bag: Mutex<DisposeBag>,        // LIFO disposal queue
}
```

**Design Decision**: Scopes maintain a reference to the root provider rather than copying all registrations. This reduces memory usage but requires runtime lifetime checks.

### Disposal Management

Services are disposed in **LIFO order** (Last In, First Out):

```rust
// Registration order
scope.resolve::<DatabaseConnection>();  // 1st
scope.resolve::<UserService>();         // 2nd  
scope.resolve::<RequestHandler>();      // 3rd

// Disposal order (reverse)
drop(scope); // Disposes: RequestHandler → UserService → DatabaseConnection
```

This ensures dependencies are disposed before their dependencies.

## Performance Design

### Hot Path Optimization

**Singleton Resolution Hot Path** (~78ns):
1. HashMap lookup by TypeId (1 hash operation)
2. Arc clone (atomic reference count increment)
3. Unsafe downcast (zero-cost type assertion)

```rust
// Optimized singleton resolution
pub fn get_singleton<T>(&self) -> DiResult<Arc<T>> {
    let key = Key::Type(TypeId::of::<T>(), type_name::<T>());
    
    // Fast path: direct registry lookup
    if let Some(registration) = self.registry.single.get(&key) {
        let any_arc = registration.instance?; // Pre-resolved singleton
        Ok(unsafe { Arc::downcast_unchecked(any_arc) }) // Zero-cost cast
    } else {
        Err(DiError::NotFound { ... })
    }
}
```

### Memory Layout

**Service Storage**:
```rust
struct Registration {
    lifetime: Lifetime,                    // 1 byte (enum)
    ctor: Arc<dyn Fn(...) -> DiResult<AnyArc>>, // 16 bytes (fat pointer)
    metadata: Option<Box<dyn Any>>,        // 8 bytes (thin pointer)
    impl_id: Option<TypeId>,               // 9 bytes (Option<u128>)
}
// Total: ~40 bytes per registration
```

**Service Resolution**:
- **Singleton**: Arc clone (atomic increment)
- **Transient**: Factory call + Arc allocation
- **Scoped**: HashMap lookup + Arc clone

### Lock-Free Design

- **ServiceProvider**: Immutable after creation (no locks needed)
- **Singleton instances**: Pre-resolved during build phase
- **Scoped services**: Mutex only for scope-local storage
- **Concurrent access**: Multiple threads can resolve simultaneously

## Error Handling

### Error Architecture

```rust
pub enum DiError {
    NotFound { type_name: &'static str },
    Circular(Vec<&'static str>),
    DepthExceeded(usize),
    ScopeRequired { type_name: &'static str },
    AlreadyBuilt,
    ConstructionFailed { type_name: &'static str, source: Box<dyn Error> },
}
```

**Design Principles**:
1. **Structured errors** with context information
2. **No panics** in normal operation (only for programming errors)
3. **Detailed error paths** for circular dependencies
4. **Source chain** for construction failures

### Error Recovery

```rust
// Graceful degradation example
let primary = provider.get::<PrimaryDatabase>();
let fallback = match primary {
    Ok(db) => db,
    Err(_) => provider.get_required::<FallbackDatabase>(), // Panics if missing
};
```

Errors can be handled gracefully or converted to panics for "must-have" dependencies.

## Thread Safety

### Concurrency Model

**Thread-Safe Components**:
- `ServiceProvider`: Immutable after creation
- `Arc<T>`: Atomic reference counting
- Singleton instances: Pre-resolved and shared

**Thread-Local Components**:
- `Scope`: Not Send/Sync (scope per thread/request)
- Circular dependency detection: Thread-local stack

### Circular Dependency Detection

```rust
thread_local! {
    static RESOLUTION_TLS: RefCell<ResolutionTls> = RefCell::new(ResolutionTls::default());
}

struct ResolutionTls {
    stack: Vec<&'static str>,  // Current resolution path
    frozen: bool,              // Prevent corruption during panic
    depth: usize,              // Stack overflow protection
}
```

**Design Decision**: Thread-local detection avoids coordination overhead but requires careful panic handling to prevent TLS corruption.

## Design Patterns

### Factory Pattern

Services are created using **factory functions** rather than direct construction:

```rust
// Instead of storing instances
services.add_singleton(MyService::new(config));

// Store factory functions  
services.add_singleton_factory::<MyService, _>(|resolver| {
    let config = resolver.get_required::<Config>();
    MyService::new(config) // Dependency injection here
});
```

**Benefits**:
- Lazy initialization
- Dependency injection at creation time
- Better error handling

### Builder Pattern

ServiceCollection uses fluent builder pattern:

```rust
let provider = ServiceCollection::new()
    .add_singleton(Config::default())
    .add_scoped_factory::<Database, _>(|r| Database::connect(r.get_required()))
    .add_transient::<UserService>()
    .build();
```

### Strategy Pattern

Different resolution strategies based on lifetime:

```rust
match registration.lifetime {
    Lifetime::Singleton => self.resolve_singleton(key),
    Lifetime::Transient => self.resolve_transient(key),  
    Lifetime::Scoped => self.resolve_scoped(key),
}
```

## Trade-offs and Limitations

### Performance vs. Flexibility

**Trade-off**: Pre-resolved singletons vs. lazy initialization
- **Choice**: Pre-resolution during build phase
- **Benefit**: Faster runtime resolution (~78ns)
- **Cost**: Longer build time, memory usage

### Type Safety vs. Ergonomics

**Trade-off**: Compile-time vs. runtime type checking
- **Choice**: Generic methods with TypeId lookup
- **Benefit**: Type safety with reasonable ergonomics
- **Cost**: Some boilerplate for trait objects

### Memory vs. CPU

**Trade-off**: Memory usage vs. computation
- **Choice**: HashMap storage with Arc sharing
- **Benefit**: Fast lookups, minimal allocations
- **Cost**: Memory overhead for small numbers of services

### Current Limitations

1. **No constructor injection** - Must use factory functions
2. **String-based trait keys** - Runtime errors possible
3. **Single container** - No hierarchical containers
4. **No conditional registration** - Basic TryAdd methods only

### Future Improvements

1. **Derive macros** for automatic factory generation
2. **Compile-time validation** for trait object keys  
3. **Container hierarchies** for modular applications
4. **Advanced conditional registration** with predicates

---

## Design Philosophy

Ferrous DI follows these core principles:

1. **Performance First** - Optimize for the common case (singleton resolution)
2. **Type Safety** - Leverage Rust's type system, avoid `unsafe` where possible
3. **Explicit Over Implicit** - Clear APIs, no hidden magic
4. **Composability** - Small, focused abstractions that work together
5. **Pragmatic** - Balance theoretical purity with real-world usability

The architecture reflects these principles in every design decision, from the module structure to the performance optimizations.