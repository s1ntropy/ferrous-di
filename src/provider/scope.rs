//! Scoped service resolution and lifecycle management.
//!
//! This module contains the Scope and ScopedResolver types for managing
//! request-scoped services and their automatic disposal.

#[cfg(not(feature = "once-cell"))]
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::future::Future;

#[cfg(feature = "once-cell")]
use once_cell::sync::OnceCell;

use crate::{DiResult, DiError, Key, Lifetime};
use crate::registration::AnyArc;
use super::ResolverContext;
use crate::internal::{DisposeBag, BoxFutureUnit, with_circular_catch};
use crate::traits::{Resolver, ResolverCore, Dispose, AsyncDispose};
use super::ServiceProvider;

/// Scoped service container for request-scoped dependency resolution.
///
/// A `Scope` provides isolated dependency resolution for scoped services while
/// still accessing singleton services from the root provider. This is ideal for
/// web applications where you want request-scoped services (like database connections,
/// user contexts, etc.) that are shared within a single request but isolated
/// between requests.
///
/// # Lifetime Behavior
///
/// - **Singleton**: Resolved and cached in the root provider (shared across all scopes)
/// - **Scoped**: Resolved and cached within this specific scope
/// - **Transient**: Created fresh on every resolution (no caching)
///
/// # Examples
///
/// ```
/// use ferrous_di::{ServiceCollection, Resolver};
/// use std::sync::{Arc, Mutex};
///
/// #[derive(Debug)]
/// struct DatabaseConnection(String);
///
/// #[derive(Debug)]
/// struct UserService {
///     db: Arc<DatabaseConnection>,
/// }
///
/// let mut collection = ServiceCollection::new();
///
/// // Scoped database connection per request
/// collection.add_scoped_factory::<DatabaseConnection, _>(|_| {
///     DatabaseConnection("connection-123".to_string())
/// });
///
/// // Transient user service that uses scoped DB connection
/// collection.add_transient_factory::<UserService, _>(|resolver| {
///     UserService {
///         db: resolver.get_required::<DatabaseConnection>(),
///     }
/// });
///
/// let provider = collection.build();
/// let scope = provider.create_scope();
///
/// // Multiple services in the same scope share the same DB connection
/// let user1 = scope.get_required::<UserService>();
/// let user2 = scope.get_required::<UserService>();
/// assert!(Arc::ptr_eq(&user1.db, &user2.db));
/// ```
pub struct Scope {
    pub(crate) root: ServiceProvider,
    // Slot-based scoped storage for O(1) access
    #[cfg(feature = "once-cell")]
    pub(crate) scoped_cells: Box<[OnceCell<AnyArc>]>,
    #[cfg(not(feature = "once-cell"))]
    pub(crate) scoped: Mutex<HashMap<Key, AnyArc>>,
    pub(crate) scoped_disposers: Mutex<DisposeBag>,
}

impl Clone for Scope {
    fn clone(&self) -> Self {
        // Create a new scope with the same root but fresh scoped state
        #[cfg(feature = "once-cell")]
        {
            let scoped_count = self.root.inner().registry.scoped_count;
            let scoped_cells: Box<[OnceCell<AnyArc>]> = (0..scoped_count)
                .map(|_| OnceCell::new())
                .collect::<Vec<_>>()
                .into_boxed_slice();
                
            Self {
                root: self.root.clone(),
                scoped_cells,
                scoped_disposers: Mutex::new(DisposeBag::default()),
            }
        }
        
        #[cfg(not(feature = "once-cell"))]
        {
            Self {
                root: self.root.clone(),
                scoped: Mutex::new(HashMap::new()),
                scoped_disposers: Mutex::new(DisposeBag::default()),
            }
        }
    }
}

impl ResolverCore for Scope {
    fn resolve_any(&self, key: &Key) -> DiResult<AnyArc> {
        let name = key.display_name();
        with_circular_catch(name, || self.resolve_any_impl(key))
    }
    
    fn resolve_many(&self, key: &Key) -> DiResult<Vec<AnyArc>> {
        if let Key::Trait(_trait_name) = key {
            let name = key.display_name();
            with_circular_catch(name, || self.resolve_many_impl(key))
        } else {
            Ok(Vec::new())
        }
    }

    fn push_sync_disposer(&self, f: Box<dyn FnOnce() + Send>) {
        self.scoped_disposers.lock().unwrap().push_sync(f);
    }

    fn push_async_disposer(&self, f: Box<dyn FnOnce() -> BoxFutureUnit + Send>) {
        self.scoped_disposers.lock().unwrap().push_async(move || (f)());
    }
}

impl Scope {
    /// Ultra-optimized scoped resolution using slot-based Vec storage
    #[inline(always)]
    fn resolve_scoped(&self, reg: &crate::registration::Registration, _key: &Key) -> DiResult<AnyArc> {
        #[cfg(feature = "once-cell")]
        {
            if let Some(slot) = reg.scoped_slot {
                let cell = &self.scoped_cells[slot];
                
                // Ultra-fast path: check if already initialized
                if let Some(value) = cell.get() {
                    return Ok(value.clone());
                }
                
                // Slow path: initialize with factory (unlikely after first access)
                // TODO: Add std::hint::unlikely when stable
                {
                    let ctx = ResolverContext::new(self);
                    let v = (reg.ctor)(&ctx)?;
                    let stored = cell.get_or_init(|| v.clone()).clone();
                    return Ok(stored);
                }
            }
        }
        
        #[cfg(not(feature = "once-cell"))]
        {
            // Use HashMap for scoped caching when once-cell is not available
            let key = _key.clone();
            
            // Check if already cached
            {
                let guard = self.scoped.lock().unwrap();
                if let Some(cached) = guard.get(&key) {
                    return Ok(cached.clone());
                }
            }
            
            // Create and cache the value
            let ctx = ResolverContext::new(self);
            let value = (reg.ctor)(&ctx)?;
            
            // Cache the value
            {
                let mut guard = self.scoped.lock().unwrap();
                guard.insert(key, value.clone());
            }
            
            Ok(value)
        }
        
        #[cfg(feature = "once-cell")]
        {
            // Fallback if no slot assigned (shouldn't happen with once-cell)
            let ctx = ResolverContext::new(self);
            (reg.ctor)(&ctx)
        }
    }

    fn resolve_any_impl(&self, key: &Key) -> DiResult<AnyArc> {
        let name = key.display_name();
        
        if let Some(reg) = self.root.inner().registry.get(key) {
            match reg.lifetime {
                Lifetime::Singleton => {
                    // Delegate to root provider's optimized singleton resolution
                    self.root.resolve_singleton(reg, key)
                }
                Lifetime::Scoped => {
                    // Use optimized slot-based scoped resolution
                    self.resolve_scoped(reg, key)
                }
                Lifetime::Transient => {
                    let ctx = ResolverContext::new(self);
                    (reg.ctor)(&ctx)  // CRITICAL FIX: pass self (scope) as resolver
                }
            }
        } else if let Key::Trait(trait_name) = key {
            // Fallback: if trait has multi-bindings, return last as single
            if let Some(regs) = self.root.inner().registry.many.get(trait_name) {
                if let Some(last) = regs.last() {
                    let ctx = ResolverContext::new(self);
                    (last.ctor)(&ctx)  // CRITICAL FIX: pass self (scope) as resolver
                } else {
                    Err(DiError::NotFound(name))
                }
            } else {
                Err(DiError::NotFound(name))
            }
        } else {
            Err(DiError::NotFound(name))
        }
    }
    
    fn resolve_many_impl(&self, key: &Key) -> DiResult<Vec<AnyArc>> {
        if let Key::Trait(trait_name) = key {
            
            if let Some(regs) = self.root.inner().registry.many.get(trait_name) {
                let mut results = Vec::with_capacity(regs.len());
                
                for (i, reg) in regs.iter().enumerate() {
                    let multi_key = Key::MultiTrait(trait_name, i);
                    
                    let value = match reg.lifetime {
                        Lifetime::Singleton => {
                            // Expert fix: Double-checked locking for singletons
                            {
                                let cache = self.root.inner().singletons.lock().unwrap();
                                if let Some(cached) = cache.get(&multi_key) {
                                    results.push(cached.clone());
                                    continue;
                                }
                            } // Lock released here
                            
                            // Create without holding lock
                            let ctx = ResolverContext::new(self);
                            let value = (reg.ctor)(&ctx)?;
                            
                            // Double-checked insert
                            {
                                let mut cache = self.root.inner().singletons.lock().unwrap();
                                if let Some(cached) = cache.get(&multi_key) {
                                    cached.clone() // Another thread beat us
                                } else {
                                    cache.insert(multi_key, value.clone());
                                    value
                                }
                            }
                        }
                        Lifetime::Scoped => {
                            // Use slot-based scoped resolution for multi-bindings
                            #[allow(unused_variables)]
                            if let Some(slot) = reg.scoped_slot {
                                #[cfg(feature = "once-cell")]
                                {
                                    let cell = &self.scoped_cells[slot];
                                    
                                    // Ultra-fast path: check if already initialized
                                    if let Some(value) = cell.get() {
                                        value.clone()
                                    } else {
                                        // Slow path: initialize with factory
                                        let ctx = ResolverContext::new(self);
                                        let v = (reg.ctor)(&ctx)?;
                                        cell.get_or_init(|| v.clone()).clone()
                                    }
                                }
                                #[cfg(not(feature = "once-cell"))]
                                {
                                    // Use HashMap caching for scoped multi-bindings when once-cell is not available
                                    let multi_key = Key::MultiTrait(trait_name, i);
                                    
                                    // Check if already cached
                                    {
                                        let guard = self.scoped.lock().unwrap();
                                        if let Some(cached) = guard.get(&multi_key) {
                                            cached.clone()
                                        } else {
                                            drop(guard); // Release lock before creating
                                            
                                            // Create and cache the value
                                            let ctx = ResolverContext::new(self);
                                            let value = (reg.ctor)(&ctx)?;
                                            
                                            let mut guard = self.scoped.lock().unwrap();
                                            guard.insert(multi_key, value.clone());
                                            value
                                        }
                                    }
                                }
                            } else {
                                // No slot assigned - fallback to transient behavior
                                let ctx = ResolverContext::new(self);
                                (reg.ctor)(&ctx)?
                            }
                        }
                        Lifetime::Transient => {
                            let ctx = ResolverContext::new(self);
                            (reg.ctor)(&ctx)?
                        }
                    };
                    
                    results.push(value);
                }
                
                Ok(results)
            } else {
                Ok(Vec::new())
            }
        } else {
            Ok(Vec::new())
        }
    }

    /// Disposes all scoped disposal hooks in LIFO order.
    ///
    /// This method runs all asynchronous disposal hooks first (in reverse order),
    /// followed by all synchronous disposal hooks (in reverse order). This ensures
    /// proper cleanup of scoped services.
    ///
    /// # Examples
    ///
    /// ```
    /// use ferrous_di::{ServiceCollection, Dispose, Resolver};
    /// use std::sync::Arc;
    ///
    /// struct ScopedCache {
    ///     name: String,
    /// }
    ///
    /// impl Dispose for ScopedCache {
    ///     fn dispose(&self) {
    ///         println!("Disposing scoped cache: {}", self.name);
    ///     }
    /// }
    ///
    /// # async fn example() {
    /// let mut services = ServiceCollection::new();
    /// services.add_scoped_factory::<ScopedCache, _>(|r| {
    ///     let cache = Arc::new(ScopedCache { name: "request_cache".to_string() });
    ///     r.register_disposer(cache.clone());
    ///     ScopedCache { name: "request_cache".to_string() } // Return concrete type
    /// });
    ///
    /// let provider = services.build();
    /// let scope = provider.create_scope();
    /// // ... use scoped services ...
    /// scope.dispose_all().await; // Only disposes scoped resources
    /// # }
    /// ```
    pub async fn dispose_all(&self) {
        // First run async disposers in reverse order
        self.scoped_disposers.lock().unwrap().run_all_async_reverse().await;
        // Then run sync disposers in reverse order  
        self.scoped_disposers.lock().unwrap().run_all_sync_reverse();
    }

    /// Executes an async block with automatic disposal of services resolved via `*_disposable` methods.
    ///
    /// This method provides a "using" pattern where services resolved with the disposable
    /// variants (`get_disposable`, `get_async_disposable`, etc.) are automatically disposed
    /// when the block exits, regardless of whether it succeeds or fails.
    ///
    /// # Disposal Order
    ///
    /// Services are disposed in LIFO order (last resolved, first disposed):
    /// 1. Async disposers run first (in reverse order)
    /// 2. Sync disposers run second (in reverse order)
    ///
    /// # Error Handling
    ///
    /// The block's result is preserved even if disposal occurs. Disposal happens
    /// regardless of success or failure of the user block.
    ///
    /// # Examples
    ///
    /// ```
    /// use ferrous_di::{ServiceCollection, Dispose, AsyncDispose, DiError};
    /// use async_trait::async_trait;
    /// use std::sync::Arc;
    ///
    /// struct DatabaseConnection;
    /// impl Dispose for DatabaseConnection {
    ///     fn dispose(&self) {
    ///         println!("Closing database connection");
    ///     }
    /// }
    ///
    /// struct ApiClient;
    /// #[async_trait]
    /// impl AsyncDispose for ApiClient {
    ///     async fn dispose(&self) {
    ///         println!("Shutting down API client");
    ///     }
    /// }
    ///
    /// # async fn example() -> Result<(), DiError> {
    /// let mut services = ServiceCollection::new();
    /// services.add_scoped_factory::<DatabaseConnection, _>(|_| DatabaseConnection);
    /// services.add_scoped_factory::<ApiClient, _>(|_| ApiClient);
    ///
    /// let provider = services.build();
    /// let scope = provider.create_scope();
    ///
    /// let result = scope.using(|resolver| async move {
    ///     let db = resolver.get_disposable::<DatabaseConnection>()?;
    ///     let api = resolver.get_async_disposable::<ApiClient>()?;
    ///     
    ///     // Use the services...
    ///     Ok::<String, DiError>("Operation completed".to_string())
    /// }).await?;
    ///
    /// // Both services are automatically disposed here in LIFO order:
    /// // 1. ApiClient.dispose() (async)  
    /// // 2. DatabaseConnection.dispose() (sync)
    ///
    /// assert_eq!(result, "Operation completed");
    /// # Ok(())
    /// # }
    /// ```
    pub async fn using<F, Fut, R, E>(&self, f: F) -> Result<R, E>
    where
        F: FnOnce(ScopedResolver) -> Fut,
        Fut: Future<Output = Result<R, E>>,
        E: From<DiError>,
    {
        let resolver = ScopedResolver::new(self);
        let bag_handle = resolver.bag.clone();

        // Run user code
        let result = f(resolver).await;

        // Always dispose (even on error): async then sync, LIFO
        let mut bag = std::mem::take(&mut *bag_handle.lock().unwrap());
        bag.run_all_async_reverse().await;
        bag.run_all_sync_reverse();

        result
    }

    /// Executes a synchronous block with automatic disposal of services resolved via `*_disposable` methods.
    ///
    /// This is the synchronous variant of [`using`](Self::using) for blocks that don't need async.
    /// Only synchronous disposers are supported - async disposers will be ignored in this method.
    ///
    /// # Examples
    ///
    /// ```
    /// use ferrous_di::{ServiceCollection, Dispose, DiError};
    /// use std::sync::Arc;
    ///
    /// struct FileHandle;
    /// impl Dispose for FileHandle {
    ///     fn dispose(&self) {
    ///         println!("Closing file");
    ///     }
    /// }
    ///
    /// # fn example() -> Result<(), DiError> {
    /// let mut services = ServiceCollection::new();
    /// services.add_scoped_factory::<FileHandle, _>(|_| FileHandle);
    ///
    /// let provider = services.build();
    /// let scope = provider.create_scope();
    ///
    /// let result = scope.using_sync(|resolver| {
    ///     let file = resolver.get_disposable::<FileHandle>()?;
    ///     // Use the file...
    ///     Ok::<String, DiError>("File processed".to_string())
    /// })?;
    ///
    /// // FileHandle is automatically disposed here
    /// assert_eq!(result, "File processed");
    /// # Ok(())
    /// # }
    /// ```
    pub fn using_sync<F, R, E>(&self, f: F) -> Result<R, E>
    where
        F: FnOnce(ScopedResolver) -> Result<R, E>,
        E: From<DiError>,
    {
        let resolver = ScopedResolver::new(self);
        let bag_handle = resolver.bag.clone();

        let result = f(resolver);

        let mut bag = std::mem::take(&mut *bag_handle.lock().unwrap());
        bag.run_all_sync_reverse();

        result
    }

    /// Creates a child scope with fresh scoped state.
    ///
    /// Used by labeled scopes for hierarchical scope management in workflow engines.
    /// The child scope inherits the same root ServiceProvider but has independent scoped storage.
    pub fn create_child(&self) -> Self {
        self.clone()
    }
}

impl Drop for Scope {
    fn drop(&mut self) {
        // Check if there are undisposed scoped resources and warn
        let bag = self.scoped_disposers.get_mut().unwrap();
        if !bag.is_empty() {
            eprintln!("[ferrous-di] Scope dropped with undisposed resources. Call dispose_all().await before dropping.");
        }
    }
}

impl Resolver for Scope {
    fn register_disposer<T>(&self, service: Arc<T>)
    where
        T: Dispose + 'static,
    {
        self.push_sync_disposer(Box::new(move || service.dispose()));
    }

    fn register_async_disposer<T>(&self, service: Arc<T>)
    where
        T: AsyncDispose + 'static,
    {
        self.push_async_disposer(Box::new(move || {
            let service = service.clone();
            Box::pin(async move { service.dispose().await })
        }));
    }
}

// ===== ScopedResolver =====

/// Block-scoped resolver with automatic disposal of requested services.
///
/// `ScopedResolver` provides automatic disposal registration for services resolved
/// within a `using()` block. It maintains a shared `DisposeBag` that is automatically
/// disposed at the end of the block in LIFO order (async disposers first, then sync).
///
/// The resolver is cloneable and can be safely moved into async closures thanks to
/// its shared interior state.
///
/// # Usage
///
/// Use the `*_disposable` methods to resolve services that should be automatically
/// disposed when the block exits. Regular `get*` methods work normally without
/// auto-disposal registration.
///
/// # Examples
///
/// ```
/// use ferrous_di::{ServiceCollection, Dispose, AsyncDispose};
/// use async_trait::async_trait;
/// use std::sync::Arc;
///
/// struct DbConnection;
/// impl Dispose for DbConnection {
///     fn dispose(&self) {
///         // Close database connection
///     }
/// }
///
/// struct ApiClient;
/// #[async_trait]
/// impl AsyncDispose for ApiClient {
///     async fn dispose(&self) {
///         // Graceful shutdown
///     }
/// }
///
/// # async fn example() -> Result<(), ferrous_di::DiError> {
/// let mut services = ServiceCollection::new();
/// services.add_scoped_factory::<DbConnection, _>(|_| DbConnection);
/// services.add_scoped_factory::<ApiClient, _>(|_| ApiClient);
///
/// let provider = services.build();
/// let scope = provider.create_scope();
///
/// let result = scope.using(|resolver| async move {
///     let db = resolver.get_disposable::<DbConnection>()?; // Auto-disposed
///     let api = resolver.get_async_disposable::<ApiClient>()?; // Auto-disposed
///     // ... use services ...
///     Ok::<i32, ferrous_di::DiError>(42)
/// }).await?;
/// // db and api automatically disposed in LIFO order here
/// # Ok(())
/// # }
/// ```
#[derive(Clone)]
pub struct ScopedResolver {
    scope: Arc<Scope>,
    // Shared bag so resolver can be moved into async closures safely
    pub(crate) bag: Arc<Mutex<DisposeBag>>,
}

impl ScopedResolver {
    pub(crate) fn new(scope: &Scope) -> Self {
        Self { 
            scope: Arc::new(scope.clone()), 
            bag: Arc::new(Mutex::new(DisposeBag::default()))
        }
    }

    // --- Plain resolution (no auto-dispose) ---

    /// Resolves a concrete service type without auto-disposal registration.
    ///
    /// This method works exactly like `Scope::get()` and preserves circular
    /// dependency detection. The service will NOT be automatically disposed.
    pub fn get<T: 'static + Send + Sync>(&self) -> DiResult<Arc<T>> {
        self.scope.get::<T>()
    }

    /// Resolves a single trait implementation without auto-disposal registration.
    ///
    /// This method works exactly like `Scope::get_trait()` and preserves circular
    /// dependency detection. The service will NOT be automatically disposed.
    pub fn get_trait<T: ?Sized + 'static + Send + Sync>(&self) -> DiResult<Arc<T>> {
        self.scope.get_trait::<T>()
    }

    /// Resolves all trait implementations without auto-disposal registration.
    ///
    /// This method works exactly like `Scope::get_all_trait()`. The services
    /// will NOT be automatically disposed.
    pub fn get_all_trait<T: ?Sized + 'static + Send + Sync>(&self) -> DiResult<Vec<Arc<T>>> {
        self.scope.get_all_trait::<T>()
    }

    // --- Auto-disposing variants for concrete types ---

    /// Resolves a concrete service type and registers it for automatic synchronous disposal.
    ///
    /// The service will be disposed when the `using()` block exits, in LIFO order.
    /// The service must implement the `Dispose` trait.
    ///
    /// # Examples
    ///
    /// ```
    /// # use ferrous_di::{ServiceCollection, Dispose};
    /// # use std::sync::Arc;
    /// struct Cache;
    /// impl Dispose for Cache {
    ///     fn dispose(&self) { /* cleanup */ }
    /// }
    ///
    /// # async fn example() -> Result<(), ferrous_di::DiError> {
    /// # let mut services = ServiceCollection::new();
    /// # services.add_scoped_factory::<Cache, _>(|_| Cache);
    /// # let provider = services.build();
    /// # let scope = provider.create_scope();
    /// scope.using(|resolver| async move {
    ///     let cache = resolver.get_disposable::<Cache>()?; // Auto-disposed on block exit
    ///     Ok::<(), ferrous_di::DiError>(())
    /// }).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn get_disposable<T>(&self) -> DiResult<Arc<T>>
    where
        T: Dispose + 'static,
    {
        let s = self.scope.get::<T>()?;
        let clone = s.clone();
        self.bag.lock().unwrap().push_sync(Box::new(move || clone.dispose()));
        Ok(s)
    }

    /// Resolves a concrete service type and registers it for automatic asynchronous disposal.
    ///
    /// The service will be disposed when the `using()` block exits, in LIFO order.
    /// Async disposers run before sync disposers. The service must implement `AsyncDispose`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use ferrous_di::{ServiceCollection, AsyncDispose};
    /// # use async_trait::async_trait;
    /// # use std::sync::Arc;
    /// struct ApiClient;
    /// #[async_trait]
    /// impl AsyncDispose for ApiClient {
    ///     async fn dispose(&self) { /* async cleanup */ }
    /// }
    ///
    /// # async fn example() -> Result<(), ferrous_di::DiError> {
    /// # let mut services = ServiceCollection::new();
    /// # services.add_scoped_factory::<ApiClient, _>(|_| ApiClient);
    /// # let provider = services.build();
    /// # let scope = provider.create_scope();
    /// scope.using(|resolver| async move {
    ///     let client = resolver.get_async_disposable::<ApiClient>()?;
    ///     Ok::<(), ferrous_di::DiError>(())
    /// }).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn get_async_disposable<T>(&self) -> DiResult<Arc<T>>
    where
        T: AsyncDispose + 'static,
    {
        let s = self.scope.get::<T>()?;
        let clone = s.clone();
        self.bag.lock().unwrap().push_async(move || async move { clone.dispose().await });
        Ok(s)
    }

    // --- Auto-disposing variants for trait objects ---

    /// Resolves a trait implementation and registers it for automatic synchronous disposal.
    ///
    /// The trait object will be disposed when the `using()` block exits, in LIFO order.
    /// The trait must extend `Dispose`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use ferrous_di::{ServiceCollection, Dispose};
    /// # use std::sync::Arc;
    /// trait Cache: Dispose + Send + Sync {}
    /// struct MemoryCache;
    /// impl Dispose for MemoryCache {
    ///     fn dispose(&self) { /* cleanup */ }
    /// }
    /// impl Cache for MemoryCache {}
    ///
    /// # async fn example() -> Result<(), ferrous_di::DiError> {
    /// # let mut services = ServiceCollection::new();
    /// # services.add_scoped_trait_factory::<dyn Cache, _>(|_| Arc::new(MemoryCache));
    /// # let provider = services.build();
    /// # let scope = provider.create_scope();
    /// scope.using(|resolver| async move {
    ///     let cache = resolver.get_trait_disposable::<dyn Cache>()?;
    ///     Ok::<(), ferrous_di::DiError>(())
    /// }).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn get_trait_disposable<T>(&self) -> DiResult<Arc<T>>
    where
        T: ?Sized + Dispose + 'static + Send + Sync,
    {
        let s = self.scope.get_trait::<T>()?;
        let clone = s.clone();
        self.bag.lock().unwrap().push_sync(Box::new(move || clone.dispose()));
        Ok(s)
    }

    /// Resolves a trait implementation and registers it for automatic asynchronous disposal.
    ///
    /// The trait object will be disposed when the `using()` block exits, in LIFO order.
    /// Async disposers run before sync disposers. The trait must extend `AsyncDispose`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use ferrous_di::{ServiceCollection, AsyncDispose};
    /// # use async_trait::async_trait;
    /// # use std::sync::Arc;
    /// #[async_trait]
    /// trait ApiClient: AsyncDispose + Send + Sync {
    ///     async fn call_api(&self) -> String;
    /// }
    ///
    /// struct HttpClient;
    /// #[async_trait]
    /// impl AsyncDispose for HttpClient {
    ///     async fn dispose(&self) { /* cleanup */ }
    /// }
    /// #[async_trait]
    /// impl ApiClient for HttpClient {
    ///     async fn call_api(&self) -> String { "response".to_string() }
    /// }
    ///
    /// # async fn example() -> Result<(), ferrous_di::DiError> {
    /// # let mut services = ServiceCollection::new();
    /// # services.add_scoped_trait_factory::<dyn ApiClient, _>(|_| Arc::new(HttpClient));
    /// # let provider = services.build();
    /// # let scope = provider.create_scope();
    /// scope.using(|resolver| async move {
    ///     let client = resolver.get_trait_async_disposable::<dyn ApiClient>()?;
    ///     let response = client.call_api().await;
    ///     Ok::<String, ferrous_di::DiError>(response)
    /// }).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn get_trait_async_disposable<T>(&self) -> DiResult<Arc<T>>
    where
        T: ?Sized + AsyncDispose + 'static + Send + Sync,
    {
        let s = self.scope.get_trait::<T>()?;
        let clone = s.clone();
        self.bag.lock().unwrap().push_async(move || async move { clone.dispose().await });
        Ok(s)
    }
}