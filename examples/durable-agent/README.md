# Durable Agent Example

A comprehensive example showcasing ferrous-di's n8n-style workflow capabilities with durable execution, checkpointing, and crash recovery.

## Features Demonstrated

### 🚀 N8N-Style Workflow Primitives
- **Async Factories**: True async service construction 
- **Hierarchical Labeled Scopes**: Nested workflow/run/node contexts
- **Cancellation Tokens**: Hierarchical cancellation propagation
- **Decoration Pipeline**: Cross-cutting concerns (logging, checkpointing)
- **ScopeLocal Context**: Zero-boilerplate workflow context access
- **Observer Correlation**: Run ID tracking across all resolution events
- **Graph Export**: Dependency visualization for UIs

### 💾 Durability & State Management
- **Checkpointing**: Automatic state snapshots after each tool execution
- **Crash Recovery**: Resume workflows from any checkpoint
- **State Isolation**: Per-run context with proper cleanup
- **Pluggable Storage**: Trait-based abstractions for state persistence

### 🔧 Extension Architecture
- **Service Extensions**: Clean registration via extension methods
- **Tool System**: Pluggable tools with standardized interfaces
- **Interceptor Pipeline**: Decoration pattern for cross-cutting concerns

## Running the Example

```bash
# Build the example
cd examples/durable-agent
cargo build

# Show help and run quick demo
cargo run

# Run a complete workflow
cargo run run my-workflow

# Run and crash after step 2
cargo run run my-workflow 2

# Resume from checkpoint
cargo run resume <run_id>

# List checkpoints for a run
cargo run list <run_id>

# Export dependency graph
cargo run graph
```

## Architecture

### Core Components

#### Workflow Context (`RunContext`)
- Unique run ID generation
- Step tracking for resumption
- Metadata storage
- Automatic disposal and cleanup

#### State Management
- `StateStore`: Persistent key-value storage
- `CheckpointService`: Workflow state snapshots
- `SnapshotSerializer`: Stable serialization

#### Tool System
- `Tool`: Base trait for workflow operations
- `ToolContext`: Rich execution context with resolver access
- Built-in tools: File I/O, Math, HTTP requests

#### Decorators & Interceptors
- `CheckpointDecorator`: Automatic checkpointing
- `LoggingDecorator`: Execution tracing
- `WorkflowObserver`: DI resolution correlation

### Service Registration

The example uses extension methods for clean service registration:

```rust
let mut services = ServiceCollection::new();

services
    .add_durable_agent_core()      // Options, observers
    .add_state_services()          // Storage, checkpointing  
    .add_workflow_tools()          // Tool implementations
    .add_workflow_context(run_id, workflow_name); // Per-run context
```

### Workflow Execution

1. **Planning**: Deterministic step planning
2. **Tool Resolution**: Multi-binding tool registry
3. **Execution**: Decorated tool invocation
4. **Checkpointing**: Automatic state persistence
5. **Recovery**: Rehydration from checkpoints

### Crash Recovery Flow

```
Run #1: Execute steps 0,1,2 → Crash → Checkpoint saved
      ↓
Process restart
      ↓
Run #2: Load checkpoint → Resume from step 3 → Complete
```

## Key ferrous-di Features Used

### Async Factories
```rust
// Tools are resolved asynchronously
let tools = resolver.get_all_trait::<dyn Tool>()?;
```

### ScopeLocal Context
```rust
// Zero-boilerplate context access
let run_context = resolver.get_required::<ScopeLocal<RunContext>>();
```

### Observer Correlation
```rust
// Automatic run ID correlation in logs
services.add_observer(Arc::new(WorkflowObserver::new()));
```

### Options Pattern
```rust
// Validated configuration
services.add_options::<EngineOptions>()
    .validate(|opts| { /* validation */ })
    .register();
```

### Multi-trait Resolution
```rust
// All registered tools
let tools = resolver.get_all_trait::<dyn Tool>()?;
```

### Hierarchical Scopes
```rust
// Isolated execution context
let scope = provider.create_scope();
scope.using(|resolver| async move {
    // Workflow execution
}).await
```

## Extension Points

### Adding New Tools
```rust
struct MyTool;

#[async_trait]
impl Tool for MyTool {
    fn name(&self) -> &'static str { "my.tool" }
    // ... implementation
}

// Register via extension
services.add_trait_implementation::<dyn Tool, _>(MyTool, Lifetime::Singleton);
```

### Custom State Storage
```rust
struct PostgresStateStore { /* ... */ }

#[async_trait]
impl StateStore for PostgresStateStore {
    // Implementation
}

// Register via DI
services.add_singleton_trait::<dyn StateStore>(
    Arc::new(PostgresStateStore::new(connection))
);
```

### Adding Decorators
```rust
struct RateLimitDecorator;

impl ServiceDecorator<dyn Tool> for RateLimitDecorator {
    fn decorate(&self, tool: Arc<dyn Tool>, resolver: &dyn ResolverCore) -> Arc<dyn Tool> {
        // Rate limiting logic
    }
}
```

## Testing

The example includes comprehensive tests:

```bash
# Run all tests
cargo test

# Test specific component
cargo test test_workflow_execution
cargo test test_checkpoint_service
```

## Production Considerations

### Performance
- Tools are registered as singletons for efficiency
- Checkpoints use efficient serialization
- Observer correlation has minimal overhead

### Reliability
- Comprehensive error handling
- Automatic cleanup via `Dispose` trait
- Cancellation token propagation

### Observability
- Structured logging with correlation IDs
- Workflow execution tracing
- Dependency graph export for debugging

### Scalability
- Stateless tool design
- Pluggable storage backends
- Concurrent workflow execution

This example demonstrates how ferrous-di enables sophisticated workflow orchestration while maintaining clean separation of concerns and excellent testability.