//! Resolver traits for service resolution.

use std::any::TypeId;
use std::sync::Arc;
use crate::error::DiResult;
use crate::key::Key;
use crate::traits::{Dispose, AsyncDispose};
use crate::internal::BoxFutureUnit;

/// Core resolver trait for object-safe service resolution.
///
/// This trait provides the fundamental service resolution capabilities that are
/// object-safe (can be used as trait objects). It handles the low-level resolution
/// mechanics including circular dependency detection through thread-local stacks.
///
/// Most users should use the [`Resolver`] trait instead, which provides more
/// ergonomic generic methods built on top of this trait.
pub trait ResolverCore: Send + Sync {
    /// Resolves a single service using thread-local stack for circular dependency detection.
    ///
    /// This is the core resolution method that handles circular dependency detection
    /// and proper lifetime management. Returns the service wrapped in an `Arc` for
    /// thread-safe sharing.
    ///
    /// # Arguments
    ///
    /// * `key` - The service key to resolve (type, trait, or multi-trait)
    ///
    /// # Returns
    ///
    /// * `Ok(AnyArc)` - The resolved service wrapped in `Arc<dyn Any>`
    /// * `Err(DiError)` - Resolution error (not found, wrong lifetime, circular, etc.)
    fn resolve_any(&self, key: &Key) -> DiResult<Arc<dyn std::any::Any + Send + Sync>>;
    
    /// Resolves all multi-bound services for a trait using circular dependency detection.
    ///
    /// For traits registered with multiple implementations, this returns all of them
    /// in registration order. Single-bound traits and concrete types return empty vectors.
    ///
    /// # Arguments
    ///
    /// * `key` - The service key to resolve (typically a trait key)
    ///
    /// # Returns
    ///
    /// * `Ok(Vec<AnyArc>)` - All resolved implementations as `Arc<dyn Any>`
    /// * `Err(DiError)` - Resolution error for any implementation
    fn resolve_many(&self, key: &Key) -> DiResult<Vec<Arc<dyn std::any::Any + Send + Sync>>>;
    
    /// Legacy internal resolve method for compatibility.
    ///
    /// Delegates to [`resolve_any`](Self::resolve_any) for backward compatibility.
    fn resolve_any_internal(&self, key: &Key) -> DiResult<Arc<dyn std::any::Any + Send + Sync>> {
        self.resolve_any(key)
    }
    
    /// Legacy internal resolve many method for compatibility.
    ///
    /// Delegates to [`resolve_many`](Self::resolve_many) for backward compatibility.
    fn resolve_many_internal(&self, key: &Key) -> DiResult<Vec<Arc<dyn std::any::Any + Send + Sync>>> {
        self.resolve_many(key)
    }

    /// Registers a synchronous disposal hook.
    ///
    /// Used internally by factories to register disposal callbacks that will be
    /// executed when the containing scope or provider is disposed.
    fn push_sync_disposer(&self, f: Box<dyn FnOnce() + Send>);

    /// Registers an asynchronous disposal hook.
    ///
    /// Used internally by factories to register async disposal callbacks that will be
    /// executed when the containing scope or provider is disposed.
    fn push_async_disposer(&self, f: Box<dyn FnOnce() -> BoxFutureUnit + Send>);
}

/// High-level resolver interface with generic methods for type-safe service resolution.
///
/// This trait provides the main API that users interact with for resolving services.
/// It builds on [`ResolverCore`] to offer type-safe generic methods that handle
/// the complexities of type erasure and casting internally.
///
/// Both `ServiceProvider` and `Scope` implement this trait, making them
/// interchangeable for service resolution within their respective contexts.
///
/// # Examples
///
/// ```
/// use ferrous_di::{ServiceCollection, Resolver};
/// use std::sync::Arc;
///
/// trait Logger: Send + Sync {
///     fn log(&self, msg: &str);
/// }
///
/// struct ConsoleLogger;
/// impl Logger for ConsoleLogger {
///     fn log(&self, msg: &str) {
///         println!("LOG: {}", msg);
///     }
/// }
///
/// let mut collection = ServiceCollection::new();
/// collection.add_singleton(42usize);
/// collection.add_singleton_trait(Arc::new(ConsoleLogger) as Arc<dyn Logger>);
///
/// let provider = collection.build();
///
/// // Resolve concrete types
/// let number = provider.get_required::<usize>();
/// assert_eq!(*number, 42);
///
/// // Resolve trait objects
/// let logger = provider.get_required_trait::<dyn Logger>();
/// logger.log("Service resolved successfully");
/// ```
pub trait Resolver: ResolverCore {
    /// Resolves a concrete service type.
    ///
    /// Returns the service instance wrapped in an `Arc` for thread-safe sharing.
    /// The service must be registered with the exact type `T`.
    ///
    /// # Type Parameters
    ///
    /// * `T` - The concrete service type to resolve
    ///
    /// # Returns
    ///
    /// * `Ok(Arc<T>)` - The resolved service instance
    /// * `Err(DiError)` - Resolution error (not found, wrong lifetime, circular, etc.)
    ///
    /// # Examples
    ///
    /// ```
    /// use ferrous_di::{ServiceCollection, Resolver};
    ///
    /// let mut collection = ServiceCollection::new();
    /// collection.add_singleton("configuration".to_string());
    ///
    /// let provider = collection.build();
    /// let config = provider.get::<String>().unwrap();
    /// assert_eq!(&*config, "configuration");
    /// ```
    fn get<T: 'static + Send + Sync>(&self) -> DiResult<Arc<T>> {
        let key = Key::Type(TypeId::of::<T>(), std::any::type_name::<T>());
        let any = self.resolve_any_internal(&key)?;
        any.downcast::<T>()
            .map_err(|_| crate::error::DiError::TypeMismatch(std::any::type_name::<T>()))
    }
    
    /// Resolves a single trait implementation.
    ///
    /// Returns the most recently registered implementation for the trait `T`.
    /// If multiple implementations are registered, this returns the last one.
    /// For accessing all implementations, use [`get_all_trait`](Self::get_all_trait).
    ///
    /// # Type Parameters
    ///
    /// * `T` - The trait type to resolve (can be unsized with `?Sized`)
    ///
    /// # Returns
    ///
    /// * `Ok(Arc<T>)` - The resolved trait implementation
    /// * `Err(DiError)` - Resolution error (not found, wrong lifetime, circular, etc.)
    ///
    /// # Examples
    ///
    /// ```
    /// use ferrous_di::{ServiceCollection, Resolver};
    /// use std::sync::Arc;
    ///
    /// trait Database: Send + Sync {
    ///     fn connect(&self) -> &str;
    /// }
    ///
    /// struct PostgresDb;
    /// impl Database for PostgresDb {
    ///     fn connect(&self) -> &str { "postgres://..." }
    /// }
    ///
    /// let mut collection = ServiceCollection::new();
    /// collection.add_singleton_trait(Arc::new(PostgresDb) as Arc<dyn Database>);
    ///
    /// let provider = collection.build();
    /// let db = provider.get_trait::<dyn Database>().unwrap();
    /// assert_eq!(db.connect(), "postgres://...");
    /// ```
    fn get_trait<T: ?Sized + 'static + Send + Sync>(&self) -> DiResult<Arc<T>> 
    where
        Arc<T>: 'static,
    {
        let key = Key::Trait(std::any::type_name::<T>());
        let any = self.resolve_any_internal(&key)?;
        // Expert fix: Handle Arc<Arc<dyn Trait>> storage pattern
        any.downcast::<Arc<T>>()
            .map(|boxed| (*boxed).clone())
            .map_err(|_| crate::error::DiError::TypeMismatch(std::any::type_name::<T>()))
    }
    
    /// Resolves all registered implementations of a trait.
    ///
    /// Returns all implementations registered for trait `T` in the order they
    /// were registered. This is useful for collecting all implementations of
    /// a plugin interface or service collection pattern.
    ///
    /// # Type Parameters
    ///
    /// * `T` - The trait type to resolve all implementations for
    ///
    /// # Returns
    ///
    /// * `Ok(Vec<Arc<T>>)` - All registered trait implementations
    /// * `Err(DiError)` - Resolution error for any implementation
    ///
    /// # Examples
    ///
    /// ```
    /// use ferrous_di::{ServiceCollection, Resolver};
    /// use std::sync::Arc;
    ///
    /// trait Plugin: Send + Sync {
    ///     fn name(&self) -> &str;
    /// }
    ///
    /// struct PluginA;
    /// impl Plugin for PluginA {
    ///     fn name(&self) -> &str { "Plugin A" }
    /// }
    ///
    /// struct PluginB;
    /// impl Plugin for PluginB {
    ///     fn name(&self) -> &str { "Plugin B" }
    /// }
    ///
    /// let mut collection = ServiceCollection::new();
    /// collection.add_trait_implementation(Arc::new(PluginA) as Arc<dyn Plugin>, ferrous_di::Lifetime::Singleton);
    /// collection.add_trait_implementation(Arc::new(PluginB) as Arc<dyn Plugin>, ferrous_di::Lifetime::Singleton);
    ///
    /// let provider = collection.build();
    /// let plugins = provider.get_all_trait::<dyn Plugin>().unwrap();
    /// assert_eq!(plugins.len(), 2);
    /// assert_eq!(plugins[0].name(), "Plugin A");
    /// assert_eq!(plugins[1].name(), "Plugin B");
    /// ```
    fn get_all_trait<T: ?Sized + 'static + Send + Sync>(&self) -> DiResult<Vec<Arc<T>>>
    where
        Arc<T>: 'static,
    {
        let key = Key::Trait(std::any::type_name::<T>());
        let anys = self.resolve_many_internal(&key)?;
        
        let mut results = Vec::with_capacity(anys.len());
        for any in anys {
            // Expert fix: Handle Arc<Arc<dyn Trait>> storage pattern
            let arc = any.downcast::<Arc<T>>()
                .map(|boxed| (*boxed).clone())
                .map_err(|_| crate::error::DiError::TypeMismatch(std::any::type_name::<T>()))?;
            results.push(arc);
        }
        Ok(results)
    }
    
    /// Resolves a concrete service type, panicking on failure.
    ///
    /// This is a convenience method that calls [`get`](Self::get) and panics if
    /// the service cannot be resolved. Use this when you're certain the service
    /// is registered and want to fail fast on configuration errors.
    ///
    /// # Type Parameters
    ///
    /// * `T` - The concrete service type to resolve
    ///
    /// # Returns
    ///
    /// * `Arc<T>` - The resolved service instance
    ///
    /// # Panics
    ///
    /// Panics if the service cannot be resolved (not found, wrong lifetime,
    /// circular dependency, etc.).
    ///
    /// # Examples
    ///
    /// ```
    /// use ferrous_di::{ServiceCollection, Resolver};
    ///
    /// let mut collection = ServiceCollection::new();
    /// collection.add_singleton(42usize);
    ///
    /// let provider = collection.build();
    /// let number = provider.get_required::<usize>(); // Will panic if not found
    /// assert_eq!(*number, 42);
    /// ```
    fn get_required<T: 'static + Send + Sync>(&self) -> Arc<T> {
        self.get::<T>()
            .unwrap_or_else(|e| panic!("Failed to resolve {}: {:?}", std::any::type_name::<T>(), e))
    }
    
    /// Resolves a trait implementation, panicking on failure.
    ///
    /// This is a convenience method that calls [`get_trait`](Self::get_trait) and panics
    /// if the trait cannot be resolved. Use this when you're certain the trait is
    /// registered and want to fail fast on configuration errors.
    ///
    /// # Type Parameters
    ///
    /// * `T` - The trait type to resolve
    ///
    /// # Returns
    ///
    /// * `Arc<T>` - The resolved trait implementation
    ///
    /// # Panics
    ///
    /// Panics if the trait cannot be resolved (not found, wrong lifetime,
    /// circular dependency, etc.).
    ///
    /// # Examples
    ///
    /// ```
    /// use ferrous_di::{ServiceCollection, Resolver};
    /// use std::sync::Arc;
    ///
    /// trait Cache: Send + Sync {
    ///     fn get(&self, key: &str) -> Option<String>;
    /// }
    ///
    /// struct MemoryCache;
    /// impl Cache for MemoryCache {
    ///     fn get(&self, _key: &str) -> Option<String> {
    ///         Some("cached_value".to_string())
    ///     }
    /// }
    ///
    /// let mut collection = ServiceCollection::new();
    /// collection.add_singleton_trait(Arc::new(MemoryCache) as Arc<dyn Cache>);
    ///
    /// let provider = collection.build();
    /// let cache = provider.get_required_trait::<dyn Cache>(); // Will panic if not found
    /// assert_eq!(cache.get("key"), Some("cached_value".to_string()));
    /// ```
    fn get_required_trait<T: ?Sized + 'static + Send + Sync>(&self) -> Arc<T>
    where
        Arc<T>: 'static,
    {
        self.get_trait::<T>()
            .unwrap_or_else(|e| panic!("Failed to resolve trait {}: {:?}", std::any::type_name::<T>(), e))
    }

    /// Registers a service for synchronous disposal.
    ///
    /// This method should be called from service factories to ensure proper cleanup
    /// when the containing scope or provider is disposed. Disposal hooks execute
    /// in LIFO order (last registered, first disposed).
    ///
    /// # Arguments
    ///
    /// * `service` - The service instance to register for disposal
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
    ///         println!("Disposing cache: {}", self.name);
    ///     }
    /// }
    ///
    /// let mut services = ServiceCollection::new();
    /// services.add_scoped_factory::<Cache, _>(|resolver| {
    ///     let cache = Arc::new(Cache { name: "user_cache".to_string() });
    ///     resolver.register_disposer(cache.clone());
    ///     Cache { name: "user_cache".to_string() }
    /// });
    /// ```
    fn register_disposer<T: Dispose>(&self, service: Arc<T>) {
        self.push_sync_disposer(Box::new(move || service.dispose()));
    }

    /// Registers a service for asynchronous disposal.
    ///
    /// This method should be called from service factories to ensure proper async cleanup
    /// when the containing scope or provider is disposed. Async disposal hooks execute
    /// before sync hooks, in LIFO order (last registered, first disposed).
    ///
    /// # Arguments
    ///
    /// * `service` - The service instance to register for async disposal
    ///
    /// # Examples
    ///
    /// ```
    /// use ferrous_di::{AsyncDispose, ServiceCollection, Resolver};
    /// use async_trait::async_trait;
    /// use std::sync::Arc;
    ///
    /// struct DbConnection {
    ///     id: String,
    /// }
    ///
    /// #[async_trait]
    /// impl AsyncDispose for DbConnection {
    ///     async fn dispose(&self) {
    ///         println!("Closing connection: {}", self.id);
    ///         // Async cleanup...
    ///     }
    /// }
    ///
    /// let mut services = ServiceCollection::new();
    /// services.add_singleton_factory::<DbConnection, _>(|resolver| {
    ///     let conn = Arc::new(DbConnection { id: "conn_1".to_string() });
    ///     resolver.register_async_disposer(conn.clone());
    ///     DbConnection { id: "conn_1".to_string() }
    /// });
    /// ```
    fn register_async_disposer<T: AsyncDispose>(&self, service: Arc<T>) {
        self.push_async_disposer(Box::new(move || Box::pin(async move {
            service.dispose().await;
        })));
    }
    
    // Named service resolution methods continue...
    
    /// Resolves a named concrete service type.
    ///
    /// Returns the service instance registered with the given name and type `T`.
    ///
    /// # Type Parameters
    ///
    /// * `T` - The concrete service type to resolve
    ///
    /// # Arguments
    ///
    /// * `name` - The service name to resolve
    ///
    /// # Returns
    ///
    /// * `Ok(Arc<T>)` - The resolved named service instance
    /// * `Err(DiError)` - Resolution error (not found, wrong lifetime, circular, etc.)
    fn get_named<T: 'static + Send + Sync>(&self, name: &'static str) -> DiResult<Arc<T>> {
        let key = Key::TypeNamed(TypeId::of::<T>(), std::any::type_name::<T>(), name);
        let any = self.resolve_any_internal(&key)?;
        any.downcast::<T>()
            .map_err(|_| crate::error::DiError::TypeMismatch(std::any::type_name::<T>()))
    }
    
    /// Resolves a named concrete service type, panicking on failure.
    fn get_named_required<T: 'static + Send + Sync>(&self, name: &'static str) -> Arc<T> {
        self.get_named::<T>(name)
            .unwrap_or_else(|e| panic!("Failed to resolve named {} ({}): {:?}", std::any::type_name::<T>(), name, e))
    }
    
    /// Resolves a named trait implementation.
    fn get_named_trait<T: ?Sized + 'static + Send + Sync>(&self, name: &'static str) -> DiResult<Arc<T>>
    where
        Arc<T>: 'static,
    {
        let key = Key::TraitNamed(std::any::type_name::<T>(), name);
        let any = self.resolve_any_internal(&key)?;
        any.downcast::<Arc<T>>()
            .map(|boxed| (*boxed).clone())
            .map_err(|_| crate::error::DiError::TypeMismatch(std::any::type_name::<T>()))
    }
    
    /// Resolves a named trait implementation, panicking on failure.
    fn get_named_trait_required<T: ?Sized + 'static + Send + Sync>(&self, name: &'static str) -> Arc<T>
    where
        Arc<T>: 'static,
    {
        self.get_named_trait::<T>(name)
            .unwrap_or_else(|e| panic!("Failed to resolve named trait {} ({}): {:?}", std::any::type_name::<T>(), name, e))
    }
}