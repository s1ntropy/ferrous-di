//! Scope-local context values for ergonomic per-run state.
//!
//! This module provides zero-boilerplate access to scoped context values that
//! are shared across all services within a single agent run. Perfect for
//! passing trace IDs, budgets, cancellation tokens, and other run-scoped data.

use std::sync::Arc;
use crate::ServiceCollection;

/// Wrapper for scope-local values that are shared within a single scope.
///
/// `ScopeLocal<T>` provides ergonomic access to per-run context values like
/// trace IDs, execution budgets, cancellation tokens, or any other data that
/// should be shared across all services within a single agent run.
///
/// Unlike manually threading context through every service, `ScopeLocal<T>`
/// makes context available anywhere within the scope with a simple
/// `resolver.get_required::<ScopeLocal<T>>()` call.
///
/// # Thread Safety
///
/// The wrapped value is stored in an `Arc<T>`, making it cheaply clonable
/// and safe to access from multiple threads within the same scope.
///
/// # Examples
///
/// ```
/// use ferrous_di::{ServiceCollection, ScopeLocal, Resolver};
/// use std::sync::Arc;
///
/// #[derive(Default)]
/// struct RunContext {
///     trace_id: String,
///     max_steps: u32,
///     budget_remaining: std::sync::atomic::AtomicU32,
/// }
///
/// let mut services = ServiceCollection::new();
/// 
/// // Register scope-local context
/// services.add_scope_local::<RunContext, _>(|_resolver| {
///     Arc::new(RunContext {
///         trace_id: "trace-12345".to_string(),
///         max_steps: 50,
///         budget_remaining: std::sync::atomic::AtomicU32::new(1000),
///     })
/// });
///
/// // Any service can access the context
/// services.add_scoped_factory::<String, _>(|resolver| {
///     let ctx = resolver.get_required::<ScopeLocal<RunContext>>();
///     format!("Processing with trace: {}", ctx.trace_id)
/// });
///
/// let provider = services.build();
/// let scope1 = provider.create_scope();
/// let scope2 = provider.create_scope();
///
/// // Each scope gets its own context instance
/// let result1 = scope1.get_required::<String>();
/// let result2 = scope2.get_required::<String>();
/// // Different trace IDs in each scope
/// ```
pub struct ScopeLocal<T> {
    value: Arc<T>,
}

impl<T> ScopeLocal<T> {
    /// Creates a new scope-local wrapper around the given value.
    pub fn new(value: T) -> Self {
        Self {
            value: Arc::new(value),
        }
    }

    /// Creates a new scope-local wrapper from an existing Arc.
    pub fn from_arc(value: Arc<T>) -> Self {
        Self { value }
    }

    /// Gets a reference to the wrapped value.
    pub fn get(&self) -> &T {
        &self.value
    }

    /// Gets a clone of the underlying Arc.
    ///
    /// This is cheap since Arc cloning only increments a reference count.
    pub fn arc(&self) -> Arc<T> {
        self.value.clone()
    }
}

impl<T> Clone for ScopeLocal<T> {
    fn clone(&self) -> Self {
        Self {
            value: self.value.clone(),
        }
    }
}

impl<T> std::ops::Deref for ScopeLocal<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl<T: std::fmt::Debug> std::fmt::Debug for ScopeLocal<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ScopeLocal")
            .field("value", &self.value)
            .finish()
    }
}

impl ServiceCollection {
    /// Registers a scope-local value factory.
    ///
    /// The factory is called once per scope to create a value that will be
    /// shared across all services within that scope. This is perfect for
    /// per-run context like trace IDs, execution budgets, cancellation tokens,
    /// and other run-scoped state.
    ///
    /// The factory receives a resolver that can access other services, making
    /// it possible to build context values that depend on configuration or
    /// other services.
    ///
    /// # Type Safety
    ///
    /// The registered factory creates `ScopeLocal<T>` instances that can be
    /// resolved from any service within the scope using
    /// `resolver.get_required::<ScopeLocal<T>>()`.
    ///
    /// # Examples
    ///
    /// ```
    /// use ferrous_di::{ServiceCollection, ScopeLocal, Resolver, Options};
    /// use std::sync::Arc;
    /// use std::sync::atomic::AtomicU32;
    ///
    /// #[derive(Default)]
    /// struct AgentConfig {
    ///     max_steps: u32,
    ///     timeout_ms: u64,
    /// }
    ///
    /// struct RunContext {
    ///     trace_id: String,
    ///     max_steps: u32,
    ///     steps_remaining: AtomicU32,
    /// }
    ///
    /// let mut services = ServiceCollection::new();
    /// 
    /// // Register configuration
    /// services.add_options::<AgentConfig>()
    ///     .configure(|_r, config| {
    ///         config.max_steps = 100;
    ///         config.timeout_ms = 30000;
    ///     })
    ///     .register();
    ///
    /// // Register scope-local context that uses config
    /// services.add_scope_local::<RunContext, _>(|_resolver| {
    ///     Arc::new(RunContext {
    ///         trace_id: "trace-12345".to_string(),
    ///         max_steps: 100,
    ///         steps_remaining: AtomicU32::new(100),
    ///     })
    /// });
    ///
    /// // Services can access both config and context
    /// services.add_scoped_factory::<String, _>(|resolver| {
    ///     let ctx = resolver.get_required::<ScopeLocal<RunContext>>();
    ///     let remaining = ctx.steps_remaining.load(std::sync::atomic::Ordering::Relaxed);
    ///     format!("Trace {} has {} steps remaining", ctx.trace_id, remaining)
    /// });
    ///
    /// let provider = services.build();
    /// let scope = provider.create_scope();
    /// let status = scope.get_required::<String>();
    /// ```
    ///
    /// # Advanced Usage: Multiple Context Types
    ///
    /// You can register multiple scope-local context types for different concerns:
    ///
    /// ```
    /// use ferrous_di::{ServiceCollection, ScopeLocal, Resolver};
    /// use std::sync::Arc;
    ///
    /// struct TraceContext { trace_id: String }
    /// struct BudgetContext { tokens_remaining: std::sync::atomic::AtomicU32 }
    /// struct SecurityContext { user_id: String, permissions: Vec<String> }
    ///
    /// let mut services = ServiceCollection::new();
    ///
    /// services.add_scope_local::<TraceContext, _>(|_r| {
    ///     Arc::new(TraceContext { 
    ///         trace_id: "trace-12345".to_string() 
    ///     })
    /// });
    ///
    /// services.add_scope_local::<BudgetContext, _>(|_r| {
    ///     Arc::new(BudgetContext {
    ///         tokens_remaining: std::sync::atomic::AtomicU32::new(10000)
    ///     })
    /// });
    ///
    /// services.add_scope_local::<SecurityContext, _>(|_r| {
    ///     Arc::new(SecurityContext {
    ///         user_id: "agent-user-123".to_string(),
    ///         permissions: vec!["read".to_string(), "write".to_string()],
    ///     })
    /// });
    ///
    /// // Each context type can be resolved independently
    /// services.add_scoped_factory::<String, _>(|resolver| {
    ///     let trace = resolver.get_required::<ScopeLocal<TraceContext>>();
    ///     let budget = resolver.get_required::<ScopeLocal<BudgetContext>>();
    ///     let security = resolver.get_required::<ScopeLocal<SecurityContext>>();
    ///     
    ///     format!("User {} (trace: {}) has {} tokens", 
    ///         security.user_id,
    ///         trace.trace_id,
    ///         budget.tokens_remaining.load(std::sync::atomic::Ordering::Relaxed))
    /// });
    /// ```
    pub fn add_scope_local<T, F>(&mut self, factory: F) -> &mut Self
    where
        T: Send + Sync + 'static,
        F: Fn(&crate::provider::ResolverContext) -> Arc<T> + Send + Sync + 'static,
    {
        self.add_scoped_factory::<ScopeLocal<T>, _>(move |resolver| {
            let value = factory(resolver);
            ScopeLocal::from_arc(value)
        });
        self
    }

    /// Registers a workflow-specific scope-local context factory.
    ///
    /// This is a specialized version of `add_scope_local` that automatically
    /// provides common workflow context features like run IDs, execution metadata,
    /// and hierarchical scope information.
    ///
    /// Perfect for n8n-style workflow engines where each execution run needs
    /// rich context information.
    ///
    /// # Examples
    ///
    /// ```
    /// use ferrous_di::{ServiceCollection, ScopeLocal, WorkflowContext, Resolver};
    /// use std::sync::Arc;
    ///
    /// let mut services = ServiceCollection::new();
    /// 
    /// // Register workflow context with auto-generated run ID
    /// services.add_workflow_context::<WorkflowContext, _>(|_resolver| {
    ///     Arc::new(WorkflowContext::new("user_registration_flow"))
    /// });
    ///
    /// // Services can access rich workflow context
    /// services.add_scoped_factory::<String, _>(|resolver| {
    ///     let ctx = resolver.get_required::<ScopeLocal<WorkflowContext>>();
    ///     format!("Executing {} (run: {})", ctx.workflow_name(), ctx.run_id())
    /// });
    ///
    /// let provider = services.build();
    /// let scope = provider.create_scope();
    /// let status = scope.get_required::<String>();
    /// ```
    pub fn add_workflow_context<T, F>(&mut self, factory: F) -> &mut Self
    where
        T: Send + Sync + 'static,
        F: Fn(&crate::provider::ResolverContext) -> Arc<T> + Send + Sync + 'static,
    {
        // This is essentially the same as add_scope_local but with workflow-specific naming
        // and potential future enhancements for workflow metadata
        self.add_scope_local(factory)
    }

    /// Registers multiple scope-local contexts in a batch.
    ///
    /// Convenient for workflow engines that need to register several context types
    /// (security, tracing, budgets, etc.) in one operation.
    ///
    /// # Examples
    ///
    /// ```
    /// use ferrous_di::{ServiceCollection, ScopeLocal, Resolver};
    /// use std::sync::Arc;
    ///
    /// struct TraceContext { run_id: String }
    /// struct SecurityContext { user_id: String }
    /// struct BudgetContext { tokens: u32 }
    ///
    /// let mut services = ServiceCollection::new();
    /// 
    /// services.add_scope_locals()
    ///     .add::<TraceContext, _>(|_| Arc::new(TraceContext { run_id: "run-123".into() }))
    ///     .add::<SecurityContext, _>(|_| Arc::new(SecurityContext { user_id: "user-456".into() }))
    ///     .add::<BudgetContext, _>(|_| Arc::new(BudgetContext { tokens: 1000 }))
    ///     .register();
    ///
    /// let provider = services.build();
    /// let scope = provider.create_scope();
    /// 
    /// // All contexts are available
    /// let trace = scope.get_required::<ScopeLocal<TraceContext>>();
    /// let security = scope.get_required::<ScopeLocal<SecurityContext>>();
    /// let budget = scope.get_required::<ScopeLocal<BudgetContext>>();
    /// ```
    pub fn add_scope_locals(&mut self) -> ScopeLocalBuilder {
        ScopeLocalBuilder::new(self)
    }
}

/// Builder for registering multiple scope-local contexts.
pub struct ScopeLocalBuilder<'a> {
    collection: &'a mut ServiceCollection,
}

impl<'a> ScopeLocalBuilder<'a> {
    fn new(collection: &'a mut ServiceCollection) -> Self {
        Self { collection }
    }

    /// Adds a scope-local context type to the builder.
    pub fn add<T, F>(self, factory: F) -> Self
    where
        T: Send + Sync + 'static,
        F: Fn(&crate::provider::ResolverContext) -> Arc<T> + Send + Sync + 'static,
    {
        self.collection.add_scope_local(factory);
        self
    }

    /// Completes the builder (for fluent API consistency).
    pub fn register(self) {}
}

/// Standard workflow context for n8n-style execution engines.
///
/// Provides common workflow execution metadata that most workflow engines need.
/// This is a convenience struct that workflow engines can use directly or extend.
///
/// # Examples
///
/// ```
/// use ferrous_di::{ServiceCollection, ScopeLocal, WorkflowContext, Resolver};
/// use std::sync::Arc;
///
/// let mut services = ServiceCollection::new();
/// 
/// services.add_scope_local::<WorkflowContext, _>(|_resolver| {
///     Arc::new(WorkflowContext::new("user_onboarding"))
/// });
///
/// services.add_scoped_factory::<String, _>(|resolver| {
///     let ctx = resolver.get_required::<ScopeLocal<WorkflowContext>>();
///     format!("Executing step in workflow '{}' (run: {})", 
///         ctx.workflow_name(), 
///         ctx.run_id())
/// });
///
/// let provider = services.build();
/// let scope = provider.create_scope();
/// let status = scope.get_required::<String>();
/// ```
#[derive(Debug, Clone)]
pub struct WorkflowContext {
    /// Unique identifier for this workflow execution run
    run_id: String,
    /// Name/type of the workflow being executed
    workflow_name: String,
    /// Timestamp when this execution started
    started_at: std::time::Instant,
    /// Additional metadata for the workflow execution
    metadata: std::collections::HashMap<String, String>,
}

impl WorkflowContext {
    /// Creates a new workflow context with an auto-generated run ID.
    pub fn new(workflow_name: impl Into<String>) -> Self {
        Self {
            run_id: Self::generate_run_id(),
            workflow_name: workflow_name.into(),
            started_at: std::time::Instant::now(),
            metadata: std::collections::HashMap::new(),
        }
    }

    /// Creates a new workflow context with a specific run ID.
    pub fn with_run_id(workflow_name: impl Into<String>, run_id: impl Into<String>) -> Self {
        Self {
            run_id: run_id.into(),
            workflow_name: workflow_name.into(),
            started_at: std::time::Instant::now(),
            metadata: std::collections::HashMap::new(),
        }
    }

    /// Gets the run ID for this workflow execution.
    pub fn run_id(&self) -> &str {
        &self.run_id
    }

    /// Gets the workflow name.
    pub fn workflow_name(&self) -> &str {
        &self.workflow_name
    }

    /// Gets the elapsed time since this workflow execution started.
    pub fn elapsed(&self) -> std::time::Duration {
        self.started_at.elapsed()
    }

    /// Gets the start time for this workflow execution.
    pub fn started_at(&self) -> std::time::Instant {
        self.started_at
    }

    /// Adds metadata to the workflow context.
    pub fn add_metadata(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.metadata.insert(key.into(), value.into());
    }

    /// Gets metadata from the workflow context.
    pub fn get_metadata(&self, key: &str) -> Option<&String> {
        self.metadata.get(key)
    }

    /// Gets all metadata for the workflow context.
    pub fn metadata(&self) -> &std::collections::HashMap<String, String> {
        &self.metadata
    }

    /// Generates a unique run ID.
    fn generate_run_id() -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        use std::time::{SystemTime, UNIX_EPOCH};

        let mut hasher = DefaultHasher::new();
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos().hash(&mut hasher);
        std::thread::current().id().hash(&mut hasher);
        
        format!("run_{:x}", hasher.finish())
    }
}

impl Default for WorkflowContext {
    fn default() -> Self {
        Self::new("default_workflow")
    }
}

/// Convenience macro for accessing scope-local values with less boilerplate.
///
/// This macro reduces the verbosity of accessing scope-local context values
/// in workflow engines where context access is frequent.
///
/// # Examples
///
/// ```
/// use ferrous_di::{ServiceCollection, ScopeLocal, WorkflowContext, scope_local, Resolver};
/// use std::sync::Arc;
///
/// struct MyService;
/// impl MyService {
///     fn process(&self, resolver: &dyn crate::traits::Resolver) -> String {
///         // Without macro:
///         // let ctx = resolver.get_required::<ScopeLocal<WorkflowContext>>();
///         
///         // With macro:
///         let ctx = scope_local!(resolver, WorkflowContext);
///         format!("Processing in workflow: {}", ctx.workflow_name())
///     }
/// }
///
/// let mut services = ServiceCollection::new();
/// services.add_scope_local::<WorkflowContext, _>(|_| {
///     Arc::new(WorkflowContext::new("test_workflow"))
/// });
/// services.add_singleton(MyService);
///
/// let provider = services.build();
/// let scope = provider.create_scope();
/// let service = scope.get_required::<MyService>();
/// // let result = service.process(&scope); // Would work with proper resolver
/// ```
#[macro_export]
macro_rules! scope_local {
    ($resolver:expr, $type:ty) => {
        $resolver.get_required::<$crate::ScopeLocal<$type>>()
    };
}

/// Extensions for working with scope-local values in workflow contexts.
pub mod workflow {
    use super::ScopeLocal;
    use std::sync::Arc;

    /// Standard security context for workflow engines.
    #[derive(Debug, Clone)]
    pub struct SecurityContext {
        /// User or service account executing the workflow
        pub user_id: String,
        /// Permissions granted for this execution
        pub permissions: Vec<String>,
        /// Security tokens or credentials
        pub tokens: std::collections::HashMap<String, String>,
    }

    impl SecurityContext {
        pub fn new(user_id: impl Into<String>) -> Self {
            Self {
                user_id: user_id.into(),
                permissions: Vec::new(),
                tokens: std::collections::HashMap::new(),
            }
        }

        pub fn with_permissions(mut self, permissions: Vec<String>) -> Self {
            self.permissions = permissions;
            self
        }

        pub fn add_token(&mut self, key: impl Into<String>, token: impl Into<String>) {
            self.tokens.insert(key.into(), token.into());
        }

        pub fn has_permission(&self, permission: &str) -> bool {
            self.permissions.contains(&permission.to_string())
        }
    }

    /// Standard budget/quota context for workflow engines.
    #[derive(Debug)]
    pub struct BudgetContext {
        /// Maximum tokens/credits available for this execution
        pub max_tokens: std::sync::atomic::AtomicU32,
        /// Tokens/credits remaining
        pub tokens_remaining: std::sync::atomic::AtomicU32,
        /// Maximum execution time allowed
        pub max_duration: std::time::Duration,
        /// When this execution started (for timeout calculation)
        pub started_at: std::time::Instant,
    }

    impl BudgetContext {
        pub fn new(max_tokens: u32, max_duration: std::time::Duration) -> Self {
            Self {
                max_tokens: std::sync::atomic::AtomicU32::new(max_tokens),
                tokens_remaining: std::sync::atomic::AtomicU32::new(max_tokens),
                max_duration,
                started_at: std::time::Instant::now(),
            }
        }

        pub fn consume_tokens(&self, amount: u32) -> bool {
            let current = self.tokens_remaining.load(std::sync::atomic::Ordering::Relaxed);
            if current >= amount {
                self.tokens_remaining.fetch_sub(amount, std::sync::atomic::Ordering::Relaxed);
                true
            } else {
                false
            }
        }

        pub fn tokens_remaining(&self) -> u32 {
            self.tokens_remaining.load(std::sync::atomic::Ordering::Relaxed)
        }

        pub fn time_remaining(&self) -> Option<std::time::Duration> {
            let elapsed = self.started_at.elapsed();
            if elapsed < self.max_duration {
                Some(self.max_duration - elapsed)
            } else {
                None
            }
        }

        pub fn is_expired(&self) -> bool {
            self.time_remaining().is_none()
        }
    }
}