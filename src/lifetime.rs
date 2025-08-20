//! Service lifetime definitions.

/// Service lifetimes controlling instance caching behavior
///
/// Defines how service instances are created, cached, and shared within
/// the dependency injection container. Each lifetime has different
/// performance and memory characteristics.
///
/// # Lifetime Characteristics
///
/// - **Singleton**: Highest performance (cached), highest memory usage
/// - **Scoped**: Medium performance (scoped cache), medium memory usage  
/// - **Transient**: Lowest performance (always creates), lowest memory usage
///
/// # Examples
///
/// ```rust
/// use ferrous_di::{ServiceCollection, Resolver, Lifetime};
///
/// struct Database { url: String }
/// struct Repository { db_url: String }
/// struct RequestModel { id: u32 }
///
/// let mut services = ServiceCollection::new();
///
/// // Singleton: One instance for entire application
/// services.add_singleton(Database { 
///     url: "postgres://localhost".to_string() 
/// });
///
/// // Scoped: One instance per request/scope
/// services.add_scoped_factory::<Repository, _>(|r| {
///     let db = r.get_required::<Database>();
///     Repository { db_url: db.url.clone() }
/// });
///
/// // Transient: New instance every time
/// services.add_transient_factory::<RequestModel, _>(|_| {
///     RequestModel { id: 12345 } // Fixed value for doc test
/// });
///
/// let provider = services.build();
///
/// // Singleton: Same instance across scopes
/// let db1 = provider.get_required::<Database>();
/// let scope1 = provider.create_scope();
/// let db2 = scope1.get_required::<Database>();
/// assert!(std::ptr::eq(&*db1, &*db2)); // Same instance
///
/// // Scoped: Same within scope, different across scopes
/// let repo1a = scope1.get_required::<Repository>();
/// let repo1b = scope1.get_required::<Repository>();
/// assert!(std::ptr::eq(&*repo1a, &*repo1b)); // Same within scope
///
/// let scope2 = provider.create_scope();
/// let repo2 = scope2.get_required::<Repository>();
/// assert!(!std::ptr::eq(&*repo1a, &*repo2)); // Different across scopes
///
/// // Transient: Always different instances  
/// let model1 = scope1.get_required::<RequestModel>();
/// let model2 = scope1.get_required::<RequestModel>();
/// assert!(!std::ptr::eq(&*model1, &*model2)); // Always different
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lifetime {
    /// Single instance per root provider, cached forever
    ///
    /// Singleton services are created once when first requested and then
    /// cached in the root provider. The same instance is shared across
    /// all scopes and threads. Best for expensive-to-create services
    /// that maintain state across the entire application lifetime.
    Singleton,
    /// Single instance per scope, cached for scope lifetime  
    ///
    /// Scoped services are created once per scope when first requested
    /// within that scope. Multiple requests within the same scope return
    /// the same instance, but different scopes get different instances.
    /// Best for request-scoped services like database connections.
    Scoped,
    /// New instance per resolution, never cached
    ///
    /// Transient services create a fresh instance every time they're
    /// requested, even within the same scope. No caching is performed.
    /// Best for lightweight, stateless services where fresh instances
    /// are preferred over caching overhead.
    Transient,
}