//! Disposal traits for resource cleanup.

/// Trait for synchronous resource disposal.
///
/// Implement this trait for services that need structured teardown (e.g., flushing caches,
/// closing connections). Disposal hooks run in LIFO order when `dispose_all()` is called.
///
/// # Examples
///
/// ```
/// use ferrous_di::{Dispose, ServiceCollection, Resolver};
/// use std::sync::Arc;
///
/// struct Cache {
///     name: String,
/// }
///
/// impl Dispose for Cache {
///     fn dispose(&self) {
///         println!("Flushing cache: {}", self.name);
///         // Perform cleanup...
///     }
/// }
///
/// let mut services = ServiceCollection::new();
/// services.add_scoped_factory::<Cache, _>(|resolver| {
///     let cache = Arc::new(Cache { name: "user_cache".to_string() });
///     resolver.register_disposer(cache.clone());
///     Cache { name: "user_cache".to_string() } // Return concrete type
/// });
/// ```
pub trait Dispose: Send + Sync + 'static {
    /// Perform synchronous cleanup of resources.
    fn dispose(&self);
}

/// Trait for asynchronous resource disposal.
///
/// Implement this trait for services that require async teardown (e.g., graceful connection
/// shutdown, async I/O cleanup). Async disposal hooks run before sync hooks in LIFO order.
///
/// # Examples
///
/// ```
/// use ferrous_di::{AsyncDispose, ServiceCollection, Resolver};
/// use async_trait::async_trait;
/// use std::sync::Arc;
///
/// struct DatabaseClient {
///     connection_id: String,
/// }
///
/// #[async_trait]
/// impl AsyncDispose for DatabaseClient {
///     async fn dispose(&self) {
///         println!("Closing database connection: {}", self.connection_id);
///         // Perform async cleanup...
///     }
/// }
///
/// let mut services = ServiceCollection::new();
/// services.add_singleton_factory::<DatabaseClient, _>(|resolver| {
///     let client = Arc::new(DatabaseClient { 
///         connection_id: "conn_123".to_string() 
///     });
///     resolver.register_async_disposer(client.clone());
///     DatabaseClient { connection_id: "conn_123".to_string() } // Return concrete type
/// });
/// ```
#[async_trait::async_trait]
pub trait AsyncDispose: Send + Sync + 'static {
    /// Perform asynchronous cleanup of resources.
    async fn dispose(&self);
}