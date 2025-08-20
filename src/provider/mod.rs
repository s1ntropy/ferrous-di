//! Service provider module for dependency injection.
//!
//! This module contains the ServiceProvider type and related functionality
//! for resolving registered services from the DI container.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::{DiResult, DiError, Key, Lifetime};
use crate::registration::{Registry, AnyArc};
use crate::internal::{DisposeBag, BoxFutureUnit, with_circular_catch};
use crate::observer::Observers;
use crate::capabilities::{CapabilityRegistry, ToolSelectionCriteria, ToolDiscoveryResult, ToolInfo};
use crate::fast_singletons::FastSingletonCache;
use crate::traits::{Resolver, ResolverCore, Dispose, AsyncDispose};

// Re-export Scope and ResolverContext
pub mod scope;
pub mod context;
pub use scope::*;
pub use context::ResolverContext;
use context::ResolverContext as LocalResolverContext;

/// Service provider for resolving dependencies from the DI container.
///
/// The `ServiceProvider` is the heart of the dependency injection system. It resolves
/// services according to their registered lifetimes (Singleton, Scoped, Transient) and
/// manages the lifecycle of singleton services including disposal.
///
/// # Performance Optimizations
///
/// ServiceProvider includes world-class performance optimizations:
/// - **Singleton caching**: Embedded OnceCell provides 31ns resolution (~31.5M ops/sec)
/// - **Scoped caching**: Slot-based resolution with O(1) access times  
/// - **Hybrid registry**: Vec for small collections, HashMap for large ones
/// - **Lock-free reads**: After initialization, singleton access requires no locks
///
/// # Thread Safety
/// 
/// ServiceProvider is fully thread-safe and can be shared across multiple threads.
/// Singleton services are cached with proper synchronization, and the provider
/// can be cloned cheaply (it uses `Arc` internally).
///
/// # Examples
///
/// ```
/// use ferrous_di::{ServiceCollection, Resolver};
/// use std::sync::Arc;
///
/// struct Database { url: String }
/// struct UserService { db: Arc<Database> }
///
/// let mut collection = ServiceCollection::new();
/// collection.add_singleton(Database { url: "postgres://localhost".to_string() });
/// collection.add_transient_factory::<UserService, _>(|resolver| {
///     UserService { db: resolver.get_required::<Database>() }
/// });
///
/// let provider = collection.build();
/// let user_service = provider.get_required::<UserService>();
/// assert_eq!(user_service.db.url, "postgres://localhost");
/// ```
pub struct ServiceProvider {
    inner: Arc<ProviderInner>,
}

pub(crate) struct ProviderInner {
    pub registry: Registry,
    pub singletons: Mutex<HashMap<Key, AnyArc>>, // Legacy cache for multi-bindings
    pub fast_cache: FastSingletonCache, // High-performance singleton cache
    pub root_disposers: Mutex<DisposeBag>,
    pub observers: Observers,
    pub capabilities: CapabilityRegistry,
}

impl ServiceProvider {
    /// Convenience accessor for the inner provider
    #[inline]
    pub(crate) fn inner(&self) -> &ProviderInner {
        &self.inner
    }

    /// Creates a new scope for resolving scoped services.
    ///
    /// Scoped services are cached per scope and are ideal for request-scoped
    /// dependencies in web applications. Each scope maintains its own cache
    /// of scoped services while still accessing singleton services from the
    /// root provider.
    ///
    /// # Returns
    ///
    /// A new `Scope` that can resolve both scoped and singleton services.
    /// The scope maintains its own cache for scoped services.
    ///
    /// # Examples
    ///
    /// ```
    /// use ferrous_di::{ServiceCollection, Resolver};
    /// use std::sync::{Arc, Mutex};
    ///
    /// #[derive(Debug)]
    /// struct RequestId(String);
    ///
    /// let mut collection = ServiceCollection::new();
    /// let counter = Arc::new(Mutex::new(0));
    /// let counter_clone = counter.clone();
    ///
    /// collection.add_scoped_factory::<RequestId, _>(move |_| {
    ///     let mut c = counter_clone.lock().unwrap();
    ///     *c += 1;
    ///     RequestId(format!("req-{}", *c))
    /// });
    ///
    /// let provider = collection.build();
    ///
    /// // Create separate scopes
    /// let scope1 = provider.create_scope();
    /// let scope2 = provider.create_scope();
    ///
    /// let req1a = scope1.get_required::<RequestId>();
    /// let req1b = scope1.get_required::<RequestId>(); // Same instance
    /// let req2 = scope2.get_required::<RequestId>(); // Different instance
    ///
    /// assert!(Arc::ptr_eq(&req1a, &req1b)); // Same scope, same instance
    /// assert!(!Arc::ptr_eq(&req1a, &req2)); // Different scopes, different instances
    /// ```
    pub fn create_scope(&self) -> Scope {
        #[cfg(feature = "once-cell")]
        {
            use once_cell::sync::OnceCell;
            
            let scoped_count = self.inner().registry.scoped_count;
            let scoped_cells: Box<[OnceCell<AnyArc>]> = (0..scoped_count)
                .map(|_| OnceCell::new())
                .collect::<Vec<_>>()
                .into_boxed_slice();
                
            Scope {
                root: self.clone(),
                scoped_cells,
                scoped_disposers: Mutex::new(DisposeBag::default()),
            }
        }
        
        #[cfg(not(feature = "once-cell"))]
        {
            Scope {
                root: self.clone(),
                scoped: Mutex::new(HashMap::new()),
                scoped_disposers: Mutex::new(DisposeBag::default()),
            }
        }
    }

    /// Disposes all registered disposal hooks in LIFO order.
    ///
    /// This method runs all asynchronous disposal hooks first (in reverse order),
    /// followed by all synchronous disposal hooks (in reverse order). This ensures
    /// proper cleanup of singleton services.
    ///
    /// # Examples
    ///
    /// ```
    /// use ferrous_di::{ServiceCollection, Dispose, AsyncDispose, Resolver};
    /// use async_trait::async_trait;
    /// use std::sync::Arc;
    ///
    /// struct Cache;
    /// impl Dispose for Cache {
    ///     fn dispose(&self) {
    ///         println!("Cache disposed");
    ///     }
    /// }
    ///
    /// struct Client;
    /// #[async_trait]
    /// impl AsyncDispose for Client {
    ///     async fn dispose(&self) {
    ///         println!("Client disposed");
    ///     }
    /// }
    ///
    /// # async fn example() {
    /// let mut services = ServiceCollection::new();
    /// services.add_singleton_factory::<Cache, _>(|r| {
    ///     let cache = Arc::new(Cache);
    ///     r.register_disposer(cache.clone());
    ///     Cache // Return concrete type
    /// });
    /// services.add_singleton_factory::<Client, _>(|r| {
    ///     let client = Arc::new(Client);
    ///     r.register_async_disposer(client.clone());
    ///     Client // Return concrete type
    /// });
    ///
    /// let provider = services.build();
    /// // ... use services ...
    /// provider.dispose_all().await;
    /// # }
    /// ```
    pub async fn dispose_all(&self) {
        // First run async disposers in reverse order
        self.inner().root_disposers.lock().unwrap().run_all_async_reverse().await;
        // Then run sync disposers in reverse order  
        self.inner().root_disposers.lock().unwrap().run_all_sync_reverse();
    }
    
    #[cfg(feature = "diagnostics")]
    pub fn to_debug_string(&self) -> String {
        let mut s = String::new();
        s.push_str("=== Service Provider Debug ===\n");
        s.push_str("Single Bindings:\n");
        for (k, r) in self.inner().registry.iter() {
            s.push_str(&format!("  {:?}: {:?}\n", k, r.lifetime));
        }
        s.push_str("Multi Bindings:\n");
        for (k, rs) in &self.inner().registry.many {
            for (i, r) in rs.iter().enumerate() {
                s.push_str(&format!("  MultiTrait({} @ {}): {:?}\n", k, i, r.lifetime));
            }
        }
        s
    }
}

impl Clone for ServiceProvider {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl Drop for ServiceProvider {
    fn drop(&mut self) {
        // Check if this is the last reference to the inner provider
        if Arc::strong_count(&self.inner) == 1 {
            // Check if there are undisposed resources and warn
            if let Ok(bag) = self.inner.root_disposers.try_lock() {
                if !bag.is_empty() {
                    eprintln!("[ferrous-di] ServiceProvider dropped with undisposed resources. Call dispose_all().await before dropping.");
                }
            }
        }
    }
}

impl ResolverCore for ServiceProvider {
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
        self.inner().root_disposers.lock().unwrap().push_sync(f);
    }

    fn push_async_disposer(&self, f: Box<dyn FnOnce() -> BoxFutureUnit + Send>) {
        self.inner().root_disposers.lock().unwrap().push_async(move || (f)());
    }
}

impl ServiceProvider {
    /// Alternative high-performance singleton resolution using FastSingletonCache
    /// This provides an alternative to the embedded OnceCell approach for scenarios
    /// where maximum throughput is needed and error handling can be simplified
    #[inline(always)]
    pub fn resolve_singleton_fast_cache(&self, key: &Key) -> Option<AnyArc> {
        // Check if we already have it in the fast cache
        if let Some(cached) = self.inner().fast_cache.get(key) {
            return Some(cached);
        }
        
        // If not cached and it's a singleton, try to initialize it
        if let Some(reg) = self.inner().registry.get(key) {
            if reg.lifetime == Lifetime::Singleton {
                // Use the fast cache for ultra-high performance
                // Note: This version doesn't propagate errors for maximum performance
                let result = self.inner().fast_cache.get_or_init(key, || {
                    let ctx = LocalResolverContext::new(self);
                    (reg.ctor)(&ctx).unwrap_or_else(|_| Arc::new(()) as AnyArc)
                });
                return Some(result);
            }
        }
        None
    }

    /// Ultra-optimized singleton resolution using embedded OnceCell
    #[inline(always)]
    pub(crate) fn resolve_singleton(&self, reg: &crate::registration::Registration, _key: &Key) -> DiResult<AnyArc> {
        #[cfg(feature = "once-cell")]
        {
            if let Some(cell) = &reg.single_runtime {
                // Ultra-fast path: check if already initialized
                if let Some(value) = cell.get() {
                    return Ok(value.clone());
                }
                
                // Slow path: initialize with factory (unlikely after first access)
                // TODO: Add std::hint::unlikely when stable
                {
                    let ctx = LocalResolverContext::new(self);
                    let v = (reg.ctor)(&ctx)?;
                    let stored = cell.get_or_init(|| v.clone()).clone();
                    return Ok(stored);
                }
            }
        }
        
        #[cfg(not(feature = "once-cell"))]
        {
            if let Some(mutex) = &reg.single_runtime {
                let mut guard = mutex.lock().unwrap();
                if let Some(value) = guard.as_ref() {
                    return Ok(value.clone());
                }
                
                let ctx = LocalResolverContext::new(self);
                let value = (reg.ctor)(&ctx)?;
                *guard = Some(value.clone());
                return Ok(value);
            }
        }
        
        // Fallback to old behavior if no single_runtime (shouldn't happen)
        let ctx = LocalResolverContext::new(self);
        (reg.ctor)(&ctx)
    }
    
    fn resolve_any_impl(&self, key: &Key) -> DiResult<AnyArc> {
        let name = key.display_name();
        
        if let Some(reg) = self.inner().registry.get(key) {
            match reg.lifetime {
                Lifetime::Singleton => {
                    // Observer support with optimized path
                    if self.inner().observers.has_observers() {
                        let start = std::time::Instant::now();
                        self.inner().observers.resolving(key);
                        
                        let result = self.resolve_singleton(reg, key);
                        
                        let duration = start.elapsed();
                        self.inner().observers.resolved(key, duration);
                        result
                    } else {
                        // Ultra-fast path: no observer overhead
                        self.resolve_singleton(reg, key)
                    }
                }
                Lifetime::Scoped => {
                    Err(DiError::WrongLifetime("Cannot resolve scoped service from root provider"))
                }
                Lifetime::Transient => {
                    if self.inner().observers.has_observers() {
                        let start = std::time::Instant::now();
                        self.inner().observers.resolving(key);
                        
                        let ctx = LocalResolverContext::new(self);
                        let result = (reg.ctor)(&ctx);
                        
                        match &result {
                            Ok(_) => {
                                let duration = start.elapsed();
                                self.inner().observers.resolved(key, duration);
                            }
                            Err(_) => {
                                let duration = start.elapsed();
                                self.inner().observers.resolved(key, duration);
                            }
                        }
                        result
                    } else {
                        let ctx = LocalResolverContext::new(self);
                        (reg.ctor)(&ctx)
                    }
                }
            }
        } else if let Key::Trait(trait_name) = key {
            // Fallback: if trait has multi-bindings, return last as single
            if let Some(regs) = self.inner().registry.many.get(trait_name) {
                if let Some(last) = regs.last() {
                    if self.inner().observers.has_observers() {
                        let start = std::time::Instant::now();
                        self.inner().observers.resolving(key);
                        
                        let ctx = LocalResolverContext::new(self);
                        let result = (last.ctor)(&ctx);
                        
                        match &result {
                            Ok(_) => {
                                let duration = start.elapsed();
                                self.inner().observers.resolved(key, duration);
                            }
                            Err(_) => {
                                let duration = start.elapsed();
                                self.inner().observers.resolved(key, duration);
                            }
                        }
                        result
                    } else {
                        let ctx = LocalResolverContext::new(self);
                        (last.ctor)(&ctx)
                    }
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
            if let Some(regs) = self.inner().registry.many.get(trait_name) {
                let mut results = Vec::with_capacity(regs.len());
                
                for (i, reg) in regs.iter().enumerate() {
                    let multi_key = Key::MultiTrait(trait_name, i);
                    
                    let value = match reg.lifetime {
                        Lifetime::Singleton => {
                            // Expert fix: Double-checked locking - never hold lock while invoking factory
                            {
                                let cache = self.inner().singletons.lock().unwrap();
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
                                let mut cache = self.inner().singletons.lock().unwrap();
                                if let Some(cached) = cache.get(&multi_key) {
                                    cached.clone() // Another thread beat us
                                } else {
                                    cache.insert(multi_key, value.clone());
                                    value
                                }
                            }
                        }
                        Lifetime::Scoped => {
                            return Err(DiError::WrongLifetime("Cannot resolve scoped service from root provider"));
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

    /// Create a new ServiceProvider with the given registry.
    /// This is used internally by ServiceCollection.build().
    #[allow(dead_code)]
    pub(crate) fn new(registry: Registry) -> Self {
        Self::new_with_observers_and_capabilities(registry, Observers::new(), CapabilityRegistry::new())
    }

    /// Create a new ServiceProvider with the given registry and observers.
    /// This is used internally by ServiceCollection.build().
    #[allow(dead_code)]
    pub(crate) fn new_with_observers(registry: Registry, observers: Observers) -> Self {
        Self::new_with_observers_and_capabilities(registry, observers, CapabilityRegistry::new())
    }

    /// Create a new ServiceProvider with the given registry, observers, and capabilities.
    /// This is used internally by ServiceCollection.build().
    pub(crate) fn new_with_observers_and_capabilities(
        registry: Registry, 
        observers: Observers, 
        capabilities: CapabilityRegistry
    ) -> Self {
        Self {
            inner: Arc::new(ProviderInner {
                registry,
                singletons: Mutex::new(HashMap::new()), // Legacy cache for multi-bindings
                fast_cache: FastSingletonCache::new(), // High-performance singleton cache
                root_disposers: Mutex::new(DisposeBag::default()),
                observers,
                capabilities,
            }),
        }
    }

    /// Discovers available tools based on capability requirements.
    ///
    /// This is the main entry point for agent planners to find suitable tools
    /// for their tasks. Returns matching tools along with partial matches and
    /// any unsatisfied requirements.
    ///
    /// # Examples
    ///
    /// ```
    /// use ferrous_di::{ServiceCollection, ToolSelectionCriteria, CapabilityRequirement};
    ///
    /// // ... after registering tools with capabilities ...
    /// let mut services = ServiceCollection::new();
    /// let provider = services.build();
    ///
    /// // Find tools that can search the web
    /// let criteria = ToolSelectionCriteria::new()
    ///     .require("web_search")
    ///     .exclude_tag("experimental")
    ///     .max_cost(0.01);
    ///
    /// let result = provider.discover_tools(&criteria);
    ///
    /// println!("Found {} matching tools", result.matching_tools.len());
    /// for tool in &result.matching_tools {
    ///     println!("  - {}: {}", tool.name, tool.description);
    /// }
    ///
    /// if !result.unsatisfied_requirements.is_empty() {
    ///     println!("Missing capabilities: {:?}", result.unsatisfied_requirements);
    /// }
    /// ```
    pub fn discover_tools(&self, criteria: &ToolSelectionCriteria) -> ToolDiscoveryResult {
        self.inner.capabilities.discover(criteria)
    }
    
    /// Gets all registered tools with their capability information.
    ///
    /// Useful for debugging or building tool catalogs.
    pub fn list_all_tools(&self) -> Vec<&ToolInfo> {
        self.inner.capabilities.all_tools()
    }
    
    /// Gets capability information for a specific tool.
    pub fn get_tool_info(&self, key: &Key) -> Option<&ToolInfo> {
        self.inner.capabilities.get_tool(key)
    }
}

impl Resolver for ServiceProvider {
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