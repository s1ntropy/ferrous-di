//! Diagnostic observers for dependency injection traceability.
//!
//! This module provides hooks for observing DI resolution events, enabling 
//! structured tracing, performance monitoring, and debugging of agent workflows.
//! Enhanced with run_id correlation for n8n-style workflow engines.

use std::sync::Arc;
use std::collections::HashMap;
use crate::Key;

/// Context information for correlated observation of workflow executions.
///
/// Provides run_id and workflow metadata for correlating DI events with
/// n8n-style workflow execution traces.
#[derive(Debug, Clone)]
pub struct ObservationContext {
    /// Unique identifier for the current workflow run
    pub run_id: Option<String>,
    /// Name of the workflow being executed
    pub workflow_name: Option<String>,
    /// Current node/step being executed
    pub node_id: Option<String>,
    /// Additional metadata for the execution context
    pub metadata: HashMap<String, String>,
}

impl ObservationContext {
    /// Creates a new empty observation context.
    pub fn new() -> Self {
        Self {
            run_id: None,
            workflow_name: None,
            node_id: None,
            metadata: HashMap::new(),
        }
    }

    /// Creates an observation context with a run ID.
    pub fn with_run_id(run_id: impl Into<String>) -> Self {
        Self {
            run_id: Some(run_id.into()),
            workflow_name: None,
            node_id: None,
            metadata: HashMap::new(),
        }
    }

    /// Creates a full workflow context.
    pub fn workflow(
        run_id: impl Into<String>,
        workflow_name: impl Into<String>,
        node_id: Option<impl Into<String>>,
    ) -> Self {
        Self {
            run_id: Some(run_id.into()),
            workflow_name: Some(workflow_name.into()),
            node_id: node_id.map(|n| n.into()),
            metadata: HashMap::new(),
        }
    }

    /// Adds metadata to the context.
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }

    /// Gets a correlation string for logging/tracing.
    pub fn correlation_id(&self) -> String {
        match (&self.run_id, &self.workflow_name, &self.node_id) {
            (Some(run_id), Some(workflow), Some(node)) => {
                format!("{}:{}:{}", run_id, workflow, node)
            }
            (Some(run_id), Some(workflow), None) => {
                format!("{}:{}", run_id, workflow)
            }
            (Some(run_id), None, _) => run_id.clone(),
            _ => "no-context".to_string(),
        }
    }
}

impl Default for ObservationContext {
    fn default() -> Self {
        Self::new()
    }
}

/// Observer trait for dependency injection resolution events.
///
/// This trait enables structured tracing and monitoring of the DI container's
/// behavior. Observers can track what services are being resolved, timing
/// information, and failure conditions.
///
/// Enhanced with run_id correlation support for n8n-style workflow engines.
/// This allows correlating DI events with specific workflow execution runs.
///
/// This is particularly valuable for agentic systems where you need to:
/// - Correlate DI events with agent execution steps
/// - Debug resolution chains and performance bottlenecks
/// - Monitor tool usage patterns and failures
/// - Generate post-mortem analysis data
/// - Track workflow execution across distributed systems
///
/// # Performance
///
/// Observer calls are made synchronously during resolution. Keep implementations
/// lightweight to avoid impacting agent performance. For expensive operations,
/// consider queuing events for async processing.
///
/// # Examples
///
/// ```
/// use ferrous_di::{DiObserver, ServiceCollection, Key, ObservationContext};
/// use std::sync::Arc;
/// use std::time::Duration;
///
/// struct TracingObserver {
///     trace_id: String,
/// }
///
/// impl TracingObserver {
///     fn new(trace_id: String) -> Self {
///         Self { trace_id }
///     }
/// }
///
/// impl DiObserver for TracingObserver {
///     fn resolving(&self, key: &Key) {
///         println!("[{}] Resolving: {}", self.trace_id, key.display_name());
///     }
///
///     fn resolved(&self, key: &Key, duration: Duration) {
///         println!("[{}] Resolved: {} in {:?}", 
///             self.trace_id, key.display_name(), duration);
///     }
///
///     fn factory_panic(&self, key: &Key, message: &str) {
///         println!("[{}] PANIC in {}: {}", 
///             self.trace_id, key.display_name(), message);
///     }
///
///     fn resolving_with_context(&self, key: &Key, context: &ObservationContext) {
///         println!("[{}] [{}] Resolving: {}", 
///             self.trace_id, context.correlation_id(), key.display_name());
///     }
///
///     fn resolved_with_context(&self, key: &Key, duration: Duration, context: &ObservationContext) {
///         println!("[{}] [{}] Resolved: {} in {:?}", 
///             self.trace_id, context.correlation_id(), key.display_name(), duration);
///     }
///
///     fn factory_panic_with_context(&self, key: &Key, message: &str, context: &ObservationContext) {
///         println!("[{}] [{}] PANIC in {}: {}", 
///             self.trace_id, context.correlation_id(), key.display_name(), message);
///     }
/// }
///
/// let mut services = ServiceCollection::new();
/// services.add_observer(Arc::new(TracingObserver::new("agent-run-123".to_string())));
///
/// // All subsequent resolutions will be traced
/// let provider = services.build();
/// ```
pub trait DiObserver: Send + Sync {
    /// Called when starting to resolve a service.
    ///
    /// This is called before the factory function is invoked. Use this to
    /// start timing measurements and emit trace events.
    ///
    /// # Arguments
    ///
    /// * `key` - The service key being resolved
    fn resolving(&self, key: &Key);

    /// Called when a service is successfully resolved.
    ///
    /// This is called after the factory function completes successfully.
    /// Use this to record timing data and successful resolution events.
    ///
    /// # Arguments
    ///
    /// * `key` - The service key that was resolved
    /// * `duration` - Time elapsed from `resolving` to `resolved`
    fn resolved(&self, key: &Key, duration: std::time::Duration);

    /// Called when a factory function panics during resolution.
    ///
    /// This captures unhandled panics in factory functions, which is critical
    /// for diagnosing agent failures. The panic will still propagate after
    /// this call.
    ///
    /// # Arguments
    ///
    /// * `key` - The service key being resolved when the panic occurred
    /// * `message` - The panic message if available
    fn factory_panic(&self, key: &Key, message: &str);

    /// Called when starting to resolve a service with workflow context.
    ///
    /// Enhanced version that includes workflow execution context for correlation.
    /// Default implementation calls `resolving()` and logs basic context info.
    ///
    /// # Arguments
    ///
    /// * `key` - The service key being resolved
    /// * `context` - Workflow execution context for correlation
    fn resolving_with_context(&self, key: &Key, context: &ObservationContext) {
        // Call the basic method first
        self.resolving(key);
        
        // Provide basic context logging in default implementation
        if let Some(run_id) = &context.run_id {
            if let Some(workflow_name) = &context.workflow_name {
                println!("[{}] [{}] Starting resolution: {}", 
                    run_id, workflow_name, key.display_name());
            }
        }
    }

    /// Called when a service is successfully resolved with workflow context.
    ///
    /// Enhanced version that includes workflow execution context for correlation.
    /// Default implementation calls `resolved()` and logs basic context info.
    ///
    /// # Arguments
    ///
    /// * `key` - The service key that was resolved
    /// * `duration` - Time elapsed from `resolving` to `resolved`
    /// * `context` - Workflow execution context for correlation
    fn resolved_with_context(&self, key: &Key, duration: std::time::Duration, context: &ObservationContext) {
        // Call the basic method first
        self.resolved(key, duration);
        
        // Provide basic context logging in default implementation
        if let Some(run_id) = &context.run_id {
            if let Some(workflow_name) = &context.workflow_name {
                println!("[{}] [{}] Completed resolution: {} in {:?}", 
                    run_id, workflow_name, key.display_name(), duration);
            }
        }
    }

    /// Called when a factory function panics during resolution with workflow context.
    ///
    /// Enhanced version that includes workflow execution context for correlation.
    /// Default implementation calls `factory_panic()` and logs basic context info.
    ///
    /// # Arguments
    ///
    /// * `key` - The service key being resolved when the panic occurred
    /// * `message` - The panic message if available
    /// * `context` - Workflow execution context for correlation
    fn factory_panic_with_context(&self, key: &Key, message: &str, context: &ObservationContext) {
        // Call the basic method first
        self.factory_panic(key, message);
        
        // Provide basic context logging in default implementation
        if let Some(run_id) = &context.run_id {
            if let Some(workflow_name) = &context.workflow_name {
                eprintln!("[{}] [{}] FACTORY PANIC in {}: {}", 
                    run_id, workflow_name, key.display_name(), message);
            }
        }
    }
}

/// Container for registered observers.
///
/// This struct holds all registered observers and provides methods to notify
/// them of resolution events. It's designed to have minimal overhead when
/// no observers are registered.
#[derive(Default)]
pub(crate) struct Observers {
    observers: Vec<Arc<dyn DiObserver>>,
}

impl Observers {
    /// Creates a new empty observer collection.
    pub(crate) fn new() -> Self {
        Self {
            observers: Vec::new(),
        }
    }

    /// Adds an observer to the collection.
    pub(crate) fn add(&mut self, observer: Arc<dyn DiObserver>) {
        self.observers.push(observer);
    }

    /// Returns true if any observers are registered.
    #[inline]
    pub(crate) fn has_observers(&self) -> bool {
        !self.observers.is_empty()
    }


    /// Notifies all observers that a factory function panicked.
    #[inline]
    #[allow(dead_code)]
    pub(crate) fn factory_panic(&self, key: &Key, message: &str) {
        for observer in &self.observers {
            observer.factory_panic(key, message);
        }
    }

    /// Notifies all observers that resolution is starting with workflow context.
    #[inline]
    pub(crate) fn resolving_with_context(&self, key: &Key, context: &ObservationContext) {
        for observer in &self.observers {
            observer.resolving_with_context(key, context);
        }
    }

    /// Notifies all observers that resolution completed successfully with workflow context.
    #[inline]
    pub(crate) fn resolved_with_context(&self, key: &Key, duration: std::time::Duration, context: &ObservationContext) {
        for observer in &self.observers {
            observer.resolved_with_context(key, duration, context);
        }
    }

    /// Notifies all observers that a factory function panicked with workflow context.
    #[inline]
    #[allow(dead_code)]
    pub(crate) fn factory_panic_with_context(&self, key: &Key, message: &str, context: &ObservationContext) {
        for observer in &self.observers {
            observer.factory_panic_with_context(key, message, context);
        }
    }
}

/// Built-in observer that logs events to stdout.
///
/// This is a simple implementation useful for development and debugging.
/// For production use, consider implementing a custom observer that integrates
/// with your logging/tracing infrastructure.
///
/// # Examples
///
/// ```
/// use ferrous_di::{ServiceCollection, LoggingObserver};
/// use std::sync::Arc;
///
/// let mut services = ServiceCollection::new();
/// services.add_observer(Arc::new(LoggingObserver::new()));
///
/// // All resolutions will be logged to stdout
/// let provider = services.build();
/// ```
pub struct LoggingObserver {
    prefix: String,
}

impl LoggingObserver {
    /// Creates a new logging observer with default prefix.
    pub fn new() -> Self {
        Self {
            prefix: "[ferrous-di]".to_string(),
        }
    }

    /// Creates a new logging observer with a custom prefix.
    pub fn with_prefix(prefix: impl Into<String>) -> Self {
        Self {
            prefix: prefix.into(),
        }
    }
}

impl Default for LoggingObserver {
    fn default() -> Self {
        Self::new()
    }
}

impl DiObserver for LoggingObserver {
    fn resolving(&self, key: &Key) {
        println!("{} Resolving: {}", self.prefix, key.display_name());
    }

    fn resolved(&self, key: &Key, duration: std::time::Duration) {
        println!("{} Resolved: {} in {:?}", 
            self.prefix, key.display_name(), duration);
    }

    fn factory_panic(&self, key: &Key, message: &str) {
        eprintln!("{} FACTORY PANIC in {}: {}", 
            self.prefix, key.display_name(), message);
    }

    fn resolving_with_context(&self, key: &Key, context: &ObservationContext) {
        println!("{} [{}] Resolving: {}", 
            self.prefix, context.correlation_id(), key.display_name());
    }

    fn resolved_with_context(&self, key: &Key, duration: std::time::Duration, context: &ObservationContext) {
        println!("{} [{}] Resolved: {} in {:?}", 
            self.prefix, context.correlation_id(), key.display_name(), duration);
    }

    fn factory_panic_with_context(&self, key: &Key, message: &str, context: &ObservationContext) {
        eprintln!("{} [{}] FACTORY PANIC in {}: {}", 
            self.prefix, context.correlation_id(), key.display_name(), message);
    }
}

/// Workflow-aware observer that focuses on correlation and performance tracking.
///
/// Designed for n8n-style workflow engines where understanding the relationship
/// between service resolution and workflow execution is critical.
///
/// # Examples
///
/// ```
/// use ferrous_di::{ServiceCollection, WorkflowObserver, ObservationContext};
/// use std::sync::Arc;
///
/// let mut services = ServiceCollection::new();
/// services.add_observer(Arc::new(WorkflowObserver::new()));
///
/// let provider = services.build();
/// 
/// // When resolving with workflow context, correlation will be tracked
/// ```
pub struct WorkflowObserver {
    name: String,
    track_performance: bool,
}

impl WorkflowObserver {
    /// Creates a new workflow observer.
    pub fn new() -> Self {
        Self {
            name: "workflow".to_string(),
            track_performance: true,
        }
    }

    /// Creates a workflow observer with a custom name.
    pub fn with_name(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            track_performance: true,
        }
    }

    /// Enables or disables performance tracking.
    pub fn with_performance_tracking(mut self, enabled: bool) -> Self {
        self.track_performance = enabled;
        self
    }
}

impl Default for WorkflowObserver {
    fn default() -> Self {
        Self::new()
    }
}

impl DiObserver for WorkflowObserver {
    fn resolving(&self, key: &Key) {
        // Minimal logging for non-workflow context
        if self.track_performance {
            println!("[{}] Resolving: {}", self.name, key.display_name());
        }
    }

    fn resolved(&self, key: &Key, duration: std::time::Duration) {
        if self.track_performance {
            println!("[{}] Resolved: {} in {:?}", 
                self.name, key.display_name(), duration);
        }
    }

    fn factory_panic(&self, key: &Key, message: &str) {
        eprintln!("[{}] PANIC in {}: {}", 
            self.name, key.display_name(), message);
    }

    fn resolving_with_context(&self, key: &Key, context: &ObservationContext) {
        // Rich logging for workflow context
        match (&context.run_id, &context.workflow_name, &context.node_id) {
            (Some(run_id), Some(workflow), Some(node)) => {
                println!("[{}] [run:{}] [workflow:{}] [node:{}] Resolving: {}", 
                    self.name, run_id, workflow, node, key.display_name());
            }
            (Some(run_id), Some(workflow), None) => {
                println!("[{}] [run:{}] [workflow:{}] Resolving: {}", 
                    self.name, run_id, workflow, key.display_name());
            }
            (Some(run_id), None, _) => {
                println!("[{}] [run:{}] Resolving: {}", 
                    self.name, run_id, key.display_name());
            }
            _ => {
                println!("[{}] [no-context] Resolving: {}", 
                    self.name, key.display_name());
            }
        }
    }

    fn resolved_with_context(&self, key: &Key, duration: std::time::Duration, context: &ObservationContext) {
        if self.track_performance {
            match (&context.run_id, &context.workflow_name, &context.node_id) {
                (Some(run_id), Some(workflow), Some(node)) => {
                    println!("[{}] [run:{}] [workflow:{}] [node:{}] Resolved: {} in {:?}", 
                        self.name, run_id, workflow, node, key.display_name(), duration);
                }
                (Some(run_id), Some(workflow), None) => {
                    println!("[{}] [run:{}] [workflow:{}] Resolved: {} in {:?}", 
                        self.name, run_id, workflow, key.display_name(), duration);
                }
                (Some(run_id), None, _) => {
                    println!("[{}] [run:{}] Resolved: {} in {:?}", 
                        self.name, run_id, key.display_name(), duration);
                }
                _ => {
                    println!("[{}] [no-context] Resolved: {} in {:?}", 
                        self.name, key.display_name(), duration);
                }
            }
        }
    }

    fn factory_panic_with_context(&self, key: &Key, message: &str, context: &ObservationContext) {
        eprintln!("[{}] [{}] PANIC in {}: {}", 
            self.name, context.correlation_id(), key.display_name(), message);
    }
}

/// Helper trait for creating observation context from workflow data.
///
/// This trait allows easy integration with existing workflow engines by
/// providing a standard way to extract correlation data.
pub trait WorkflowContextProvider {
    /// Extracts observation context from the workflow state.
    fn observation_context(&self) -> ObservationContext;
}

/// Helper for integrating with ScopeLocal workflow contexts.
impl WorkflowContextProvider for crate::WorkflowContext {
    fn observation_context(&self) -> ObservationContext {
        ObservationContext::workflow(
            self.run_id(),
            self.workflow_name(),
            None::<String>,
        )
        .with_metadata("started_at", format!("{:?}", self.started_at()))
        .with_metadata("elapsed", format!("{:?}", self.elapsed()))
    }
}

/// Performance-focused observer that tracks detailed metrics.
///
/// Collects timing data, resolution counts, and failure rates for
/// post-workflow analysis and optimization.
pub struct MetricsObserver {
    pub resolution_count: std::sync::atomic::AtomicU64,
    pub total_resolution_time: std::sync::atomic::AtomicU64,
    pub panic_count: std::sync::atomic::AtomicU64,
}

impl MetricsObserver {
    /// Creates a new metrics observer.
    pub fn new() -> Self {
        Self {
            resolution_count: std::sync::atomic::AtomicU64::new(0),
            total_resolution_time: std::sync::atomic::AtomicU64::new(0),
            panic_count: std::sync::atomic::AtomicU64::new(0),
        }
    }

    /// Gets the total number of resolutions observed.
    pub fn resolution_count(&self) -> u64 {
        self.resolution_count.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Gets the average resolution time in nanoseconds.
    pub fn average_resolution_time(&self) -> Option<std::time::Duration> {
        let count = self.resolution_count();
        if count == 0 {
            return None;
        }
        
        let total_ns = self.total_resolution_time.load(std::sync::atomic::Ordering::Relaxed);
        Some(std::time::Duration::from_nanos(total_ns / count))
    }

    /// Gets the total resolution time.
    pub fn total_resolution_time(&self) -> std::time::Duration {
        let total_ns = self.total_resolution_time.load(std::sync::atomic::Ordering::Relaxed);
        std::time::Duration::from_nanos(total_ns)
    }

    /// Gets the number of panics observed.
    pub fn panic_count(&self) -> u64 {
        self.panic_count.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Resets all metrics.
    pub fn reset(&self) {
        self.resolution_count.store(0, std::sync::atomic::Ordering::Relaxed);
        self.total_resolution_time.store(0, std::sync::atomic::Ordering::Relaxed);
        self.panic_count.store(0, std::sync::atomic::Ordering::Relaxed);
    }
}

impl Default for MetricsObserver {
    fn default() -> Self {
        Self::new()
    }
}

impl DiObserver for MetricsObserver {
    fn resolving(&self, _key: &Key) {
        // No action needed on start
    }

    fn resolved(&self, _key: &Key, duration: std::time::Duration) {
        self.resolution_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        self.total_resolution_time.fetch_add(duration.as_nanos() as u64, std::sync::atomic::Ordering::Relaxed);
    }

    fn factory_panic(&self, _key: &Key, _message: &str) {
        self.panic_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }

    fn resolved_with_context(&self, key: &Key, duration: std::time::Duration, _context: &ObservationContext) {
        // Same as resolved() - context doesn't affect metrics collection
        self.resolved(key, duration);
    }

    fn factory_panic_with_context(&self, key: &Key, message: &str, _context: &ObservationContext) {
        // Same as factory_panic() - context doesn't affect metrics collection
        self.factory_panic(key, message);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use std::sync::Arc;

    #[test]
    fn test_observation_context_creation() {
        let context = ObservationContext::new();
        assert!(context.run_id.is_none());
        assert_eq!(context.correlation_id(), "no-context");

        let context = ObservationContext::with_run_id("run-123");
        assert_eq!(context.run_id.as_ref().unwrap(), "run-123");
        assert_eq!(context.correlation_id(), "run-123");

        let context = ObservationContext::workflow("run-456", "user_flow", Some("step_1"));
        assert_eq!(context.correlation_id(), "run-456:user_flow:step_1");
    }

    #[test]
    fn test_observation_context_metadata() {
        let context = ObservationContext::with_run_id("run-123")
            .with_metadata("user_id", "user-456")
            .with_metadata("priority", "high");
        
        assert_eq!(context.metadata.get("user_id").unwrap(), "user-456");
        assert_eq!(context.metadata.get("priority").unwrap(), "high");
    }

    #[test]
    fn test_workflow_observer() {
        let observer = WorkflowObserver::new();
        let key = crate::key_of_type::<String>();
        let context = ObservationContext::workflow("run-123", "test_workflow", Some("node_1"));
        
        // These should not panic
        observer.resolving(&key);
        observer.resolved(&key, Duration::from_millis(1));
        observer.resolving_with_context(&key, &context);
        observer.resolved_with_context(&key, Duration::from_millis(1), &context);
    }

    #[test]
    fn test_metrics_observer() {
        let observer = MetricsObserver::new();
        let key = crate::key_of_type::<String>();
        
        assert_eq!(observer.resolution_count(), 0);
        assert_eq!(observer.panic_count(), 0);
        assert!(observer.average_resolution_time().is_none());
        
        observer.resolved(&key, Duration::from_millis(10));
        observer.resolved(&key, Duration::from_millis(20));
        
        assert_eq!(observer.resolution_count(), 2);
        assert!(observer.average_resolution_time().is_some());
        assert!(observer.total_resolution_time() >= Duration::from_millis(30));
        
        observer.factory_panic(&key, "test panic");
        assert_eq!(observer.panic_count(), 1);
        
        observer.reset();
        assert_eq!(observer.resolution_count(), 0);
        assert_eq!(observer.panic_count(), 0);
    }

    #[test]
    fn test_workflow_context_provider() {
        let workflow_ctx = crate::WorkflowContext::new("test_workflow");
        let obs_ctx = workflow_ctx.observation_context();
        
        assert_eq!(obs_ctx.run_id.as_ref().unwrap(), workflow_ctx.run_id());
        assert_eq!(obs_ctx.workflow_name.as_ref().unwrap(), "test_workflow");
        assert!(obs_ctx.metadata.contains_key("started_at"));
        assert!(obs_ctx.metadata.contains_key("elapsed"));
    }

    #[test]
    fn test_observers_with_context() {
        let mut observers = crate::observer::Observers::new();
        let observer = Arc::new(LoggingObserver::new());
        observers.add(observer);
        
        let key = crate::key_of_type::<String>();
        let context = ObservationContext::workflow("run-123", "test_workflow", None::<String>);
        
        // These should not panic
        observers.resolving_with_context(&key, &context);
        observers.resolved_with_context(&key, Duration::from_millis(1), &context);
        observers.factory_panic_with_context(&key, "test", &context);
    }
}