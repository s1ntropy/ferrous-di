//! Service collection module for dependency injection.
//!
//! This module contains the ServiceCollection type and related functionality
//! for registering services and building service providers.

use std::any::TypeId;
use std::sync::Arc;

use crate::{DiResult, DiError, Key, Lifetime, ServiceDescriptor, DiObserver};
use crate::registration::{Registry, Registration, AnyArc};
use crate::provider::ResolverContext;
use crate::observer::Observers;
use crate::prewarm::PrewarmSet;
use crate::capabilities::CapabilityRegistry;
use crate::ServiceProvider;


pub mod module_system;
pub use module_system::*;

pub struct ServiceCollection {
    registry: Registry,
    observers: Observers,
    prewarm: PrewarmSet,
    pub(crate) capabilities: CapabilityRegistry,
}

impl ServiceCollection {
    /// Creates a new empty service collection.
    pub fn new() -> Self {
        Self {
            registry: Registry::new(),
            observers: Observers::new(),
            prewarm: PrewarmSet::new(),
            capabilities: CapabilityRegistry::new(),
        }
    }
    
    // ----- Concrete Type Registrations -----
    
    /// Registers a singleton instance that will be shared across the entire application.
    /// 
    /// The instance is created immediately and wrapped in an `Arc` for thread-safe sharing.
    /// All requests for this service type will return the same instance.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use ferrous_di::ServiceCollection;
    /// struct Config { 
    ///     database_url: String 
    /// }
    ///
    /// let mut services = ServiceCollection::new();
    /// services.add_singleton(Config {
    ///     database_url: "postgres://localhost".to_string()
    /// });
    /// ```
    pub fn add_singleton<T: 'static + Send + Sync>(&mut self, value: T) -> &mut Self {
        let arc = Arc::new(value);
        let key = Key::Type(TypeId::of::<T>(), std::any::type_name::<T>());
        let ctor = move |_: &ResolverContext| -> DiResult<AnyArc> {
            Ok(arc.clone())
        };
        self.registry.insert(key, Registration::with_metadata(
            Lifetime::Singleton,
            Arc::new(ctor),
            None,
            Some(TypeId::of::<T>()),
        ));
        self
    }
    
    /// Registers a singleton factory that creates the instance on first request.
    ///
    /// The factory is called only once, and the result is cached and shared across
    /// all subsequent requests. The factory receives a `ResolverContext` to resolve
    /// dependencies.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use ferrous_di::{ServiceCollection, Resolver};
    /// # use std::sync::Arc;
    /// struct Database { url: String }
    /// struct UserService { db: Arc<Database> }
    ///
    /// let mut services = ServiceCollection::new();
    /// services.add_singleton(Database { url: "postgres://localhost".to_string() });
    /// services.add_singleton_factory::<UserService, _>(|resolver| {
    ///     UserService {
    ///         db: resolver.get_required::<Database>()
    ///     }
    /// });
    /// ```
    pub fn add_singleton_factory<T, F>(&mut self, factory: F) -> &mut Self
    where
        T: 'static + Send + Sync,
        F: Fn(&ResolverContext) -> T + Send + Sync + 'static,
    {
        self.add_factory(Lifetime::Singleton, factory)
    }
    
    /// Registers a scoped factory that creates one instance per scope.
    ///
    /// Each scope gets its own instance, but within a scope, the same instance
    /// is reused. Perfect for per-request services in web applications.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use ferrous_di::{ServiceCollection, Resolver};
    /// # use std::sync::Arc;
    /// struct Database { url: String }
    /// struct RequestContext { request_id: String }
    /// struct UserService { db: Arc<Database>, context: Arc<RequestContext> }
    ///
    /// let mut services = ServiceCollection::new();
    /// services.add_singleton(Database { url: "postgres://localhost".to_string() });
    /// services.add_scoped_factory::<RequestContext, _>(|_| {
    ///     RequestContext { request_id: "req-123".to_string() }
    /// });
    /// services.add_scoped_factory::<UserService, _>(|resolver| {
    ///     UserService {
    ///         db: resolver.get_required::<Database>(),
    ///         context: resolver.get_required::<RequestContext>()
    ///     }
    /// });
    /// ```
    pub fn add_scoped_factory<T, F>(&mut self, factory: F) -> &mut Self
    where
        T: 'static + Send + Sync,
        F: Fn(&ResolverContext) -> T + Send + Sync + 'static,
    {
        self.add_factory(Lifetime::Scoped, factory)
    }
    
    /// Registers a transient factory that creates a new instance on every request.
    ///
    /// No caching is performed - the factory is called every time this service
    /// is resolved, even within the same scope.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use ferrous_di::{ServiceCollection, Resolver};
    /// # use std::sync::Arc;
    /// struct Database { url: String }
    /// struct Logger { timestamp: std::time::SystemTime }
    ///
    /// let mut services = ServiceCollection::new();
    /// services.add_singleton(Database { url: "postgres://localhost".to_string() });
    /// services.add_transient_factory::<Logger, _>(|_| {
    ///     Logger { timestamp: std::time::SystemTime::now() }
    /// });
    /// ```
    pub fn add_transient_factory<T, F>(&mut self, factory: F) -> &mut Self
    where
        T: 'static + Send + Sync,
        F: Fn(&ResolverContext) -> T + Send + Sync + 'static,
    {
        self.add_factory(Lifetime::Transient, factory)
    }
    
    fn add_factory<T, F>(&mut self, lifetime: Lifetime, factory: F) -> &mut Self
    where
        T: 'static + Send + Sync,
        F: Fn(&ResolverContext) -> T + Send + Sync + 'static,
    {
        let key = Key::Type(TypeId::of::<T>(), std::any::type_name::<T>());
        let factory = Arc::new(factory);
        let ctor = move |r: &ResolverContext| -> DiResult<AnyArc> {
            // Let factories run - circular dependencies will panic with CircularPanic
            // All other panics (including from get_required) will be caught at the top level
            Ok(Arc::new(factory(r)))
        };
        self.registry.insert(key, Registration::with_metadata(
            lifetime,
            Arc::new(ctor),
            None,
            Some(TypeId::of::<T>()),
        ));
        self
    }
    
    // ----- Trait Single-Binding Registrations -----
    
    /// Registers a singleton trait implementation.
    ///
    /// Binds a concrete implementation to a trait, creating a single instance
    /// that's shared across the entire application. The implementation must
    /// already be wrapped in an `Arc`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use ferrous_di::{ServiceCollection, Resolver};
    /// # use std::sync::Arc;
    /// trait Logger: Send + Sync {
    ///     fn log(&self, message: &str);
    /// }
    ///
    /// struct FileLogger { path: String }
    /// impl Logger for FileLogger {
    ///     fn log(&self, message: &str) {
    ///         // Write to file
    ///     }
    /// }
    ///
    /// let mut services = ServiceCollection::new();
    /// let logger = Arc::new(FileLogger { path: "/var/log/app.log".to_string() });
    /// services.add_singleton_trait::<dyn Logger>(logger);
    /// ```
    pub fn add_singleton_trait<T>(&mut self, value: Arc<T>) -> &mut Self
    where
        T: ?Sized + 'static + Send + Sync,
    {
        let key = Key::Trait(std::any::type_name::<T>());
        // Expert fix: Store as Arc<Arc<dyn Trait>> in Any
        let any_arc: AnyArc = Arc::new(value.clone());
        let ctor = move |_: &ResolverContext| -> DiResult<AnyArc> {
            Ok(any_arc.clone())
        };
        self.registry.insert(key, Registration::with_metadata(
            Lifetime::Singleton,
            Arc::new(ctor),
            None,
            None, // We don't know the concrete implementation type for trait objects
        ));
        self
    }
    
    /// Registers a singleton trait factory.
    ///
    /// The factory creates a trait implementation on first request, and the result
    /// is cached as a singleton. The factory must return an `Arc<Trait>`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use ferrous_di::{ServiceCollection, Resolver};
    /// # use std::sync::Arc;
    /// trait Logger: Send + Sync {
    ///     fn log(&self, message: &str);
    /// }
    ///
    /// struct FileLogger { path: String }
    /// impl Logger for FileLogger {
    ///     fn log(&self, message: &str) {
    ///         // Write to file
    ///     }
    /// }
    ///
    /// let mut services = ServiceCollection::new();
    /// services.add_singleton_trait_factory::<dyn Logger, _>(|_| {
    ///     Arc::new(FileLogger { path: "/var/log/app.log".to_string() })
    /// });
    /// ```
    pub fn add_singleton_trait_factory<Trait, F>(&mut self, factory: F) -> &mut Self
    where
        Trait: ?Sized + 'static + Send + Sync,
        F: Fn(&ResolverContext) -> Arc<Trait> + Send + Sync + 'static,
    {
        self.add_trait_factory_impl(Lifetime::Singleton, factory)
    }
    
    /// Registers a scoped trait factory.
    ///
    /// Creates one trait implementation per scope. Within a scope, the same instance
    /// is reused, but different scopes get different instances.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use ferrous_di::{ServiceCollection, Resolver};
    /// # use std::sync::Arc;
    /// trait RequestLogger: Send + Sync {
    ///     fn log_request(&self, path: &str);
    /// }
    ///
    /// struct FileRequestLogger { 
    ///     request_id: String,
    ///     file_handle: std::fs::File 
    /// }
    /// impl RequestLogger for FileRequestLogger {
    ///     fn log_request(&self, path: &str) {
    ///         // Log with request ID
    ///     }
    /// }
    ///
    /// let mut services = ServiceCollection::new();
    /// services.add_scoped_trait_factory::<dyn RequestLogger, _>(|_| {
    ///     Arc::new(FileRequestLogger { 
    ///         request_id: "req-456".to_string(),
    ///         file_handle: std::fs::File::create("/tmp/request.log").unwrap()
    ///     })
    /// });
    /// ```
    pub fn add_scoped_trait_factory<Trait, F>(&mut self, factory: F) -> &mut Self
    where
        Trait: ?Sized + 'static + Send + Sync,
        F: Fn(&ResolverContext) -> Arc<Trait> + Send + Sync + 'static,
    {
        self.add_trait_factory_impl(Lifetime::Scoped, factory)
    }
    
    /// Registers a transient trait factory.
    ///
    /// Creates a new trait implementation on every request. No caching is performed,
    /// making this suitable for lightweight, stateless services.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use ferrous_di::{ServiceCollection, Resolver};
    /// # use std::sync::Arc;
    /// trait TimeProvider: Send + Sync {
    ///     fn now(&self) -> std::time::SystemTime;
    /// }
    ///
    /// struct SystemTimeProvider;
    /// impl TimeProvider for SystemTimeProvider {
    ///     fn now(&self) -> std::time::SystemTime {
    ///         std::time::SystemTime::now()
    ///     }
    /// }
    ///
    /// let mut services = ServiceCollection::new();
    /// services.add_transient_trait_factory::<dyn TimeProvider, _>(|_| {
    ///     Arc::new(SystemTimeProvider)
    /// });
    /// ```
    pub fn add_transient_trait_factory<Trait, F>(&mut self, factory: F) -> &mut Self
    where
        Trait: ?Sized + 'static + Send + Sync,
        F: Fn(&ResolverContext) -> Arc<Trait> + Send + Sync + 'static,
    {
        self.add_trait_factory_impl(Lifetime::Transient, factory)
    }
    
    fn add_trait_factory_impl<Trait, F>(&mut self, lifetime: Lifetime, factory: F) -> &mut Self
    where
        Trait: ?Sized + 'static + Send + Sync,
        F: Fn(&ResolverContext) -> Arc<Trait> + Send + Sync + 'static,
    {
        let key = Key::Trait(std::any::type_name::<Trait>());
        let factory = Arc::new(factory);
        let ctor = move |r: &ResolverContext| -> DiResult<AnyArc> {
            // Expert fix: Store as Arc<Arc<dyn Trait>> in Any
            Ok(Arc::new(factory(r)))
        };
        self.registry.insert(key, Registration::with_metadata(
            lifetime,
            Arc::new(ctor),
            None,
            None, // We don't know the concrete implementation type for trait factories
        ));
        self
    }
    
    // ----- Trait Multi-Binding Registrations -----
    
    /// Add trait implementation to multi-binding list
    pub fn add_trait_implementation<T>(&mut self, value: Arc<T>, lifetime: Lifetime) -> &mut Self
    where
        T: ?Sized + 'static + Send + Sync,
    {
        let name = std::any::type_name::<T>();
        // Expert fix: Store Arc<dyn Trait> INSIDE Any as Arc<Arc<dyn Trait>>
        let any_arc: AnyArc = Arc::new(value.clone());
        let ctor = move |_: &ResolverContext| -> DiResult<AnyArc> {
            Ok(any_arc.clone())
        };
        self.registry.many.entry(name).or_default().push(Registration::with_metadata(
            lifetime,
            Arc::new(ctor),
            None,
            None, // We don't know the concrete implementation type for trait objects
        ));
        self
    }
    
    /// Add trait factory to multi-binding list
    pub fn add_trait_factory<Trait, F>(&mut self, lifetime: Lifetime, factory: F) -> &mut Self
    where
        Trait: ?Sized + 'static + Send + Sync,
        F: Fn(&ResolverContext) -> Arc<Trait> + Send + Sync + 'static,
    {
        let name = std::any::type_name::<Trait>();
        let factory = Arc::new(factory);
        let ctor = move |r: &ResolverContext| -> DiResult<AnyArc> {
            // Expert fix: Store as Arc<Arc<dyn Trait>> in Any
            Ok(Arc::new(factory(r)))
        };
        self.registry.many.entry(name).or_default().push(Registration::with_metadata(
            lifetime,
            Arc::new(ctor),
            None,
            None, // We don't know the concrete implementation type for trait factories
        ));
        self
    }
    
    // ----- Service Descriptors and Introspection -----
    
    /// Get all service descriptors for introspection and diagnostics.
    ///
    /// Returns a vector of `ServiceDescriptor` objects that describe all registered services,
    /// including their keys, lifetimes, and implementation type information when available.
    ///
    /// # Examples
    ///
    /// ```
    /// use ferrous_di::{ServiceCollection, Lifetime};
    /// use std::sync::Arc;
    ///
    /// let mut services = ServiceCollection::new();
    /// services.add_singleton(42usize);
    /// services.add_scoped_factory::<String, _>(|_| "hello".to_string());
    ///
    /// let descriptors = services.get_service_descriptors();
    /// assert_eq!(descriptors.len(), 2);
    /// 
    /// // Find the usize singleton
    /// let usize_desc = descriptors.iter()
    ///     .find(|d| d.type_name().contains("usize"))
    ///     .unwrap();
    /// assert_eq!(usize_desc.lifetime, Lifetime::Singleton);
    /// ```
    pub fn get_service_descriptors(&self) -> Vec<ServiceDescriptor> {
        let mut descriptors = Vec::new();
        
        // Single-binding services
        for (key, registration) in self.registry.iter() {
            descriptors.push(ServiceDescriptor {
                key: key.clone(),
                lifetime: registration.lifetime,
                impl_type_id: registration.impl_id,
                impl_type_name: registration.impl_id.map(|_| key.display_name()), // Use the key's display name as impl name
                has_metadata: registration.metadata.is_some(),
            });
        }
        
        // Multi-binding services
        for (trait_name, registrations) in &self.registry.many {
            for (index, registration) in registrations.iter().enumerate() {
                descriptors.push(ServiceDescriptor {
                    key: Key::MultiTrait(trait_name, index),
                    lifetime: registration.lifetime,
                    impl_type_id: registration.impl_id,
                    impl_type_name: registration.impl_id.map(|_| *trait_name),
                    has_metadata: registration.metadata.is_some(),
                });
            }
        }
        
        descriptors
    }
    
    /// Register a service with custom metadata.
    ///
    /// Metadata can be used for diagnostics, configuration, or other runtime introspection.
    /// The metadata must implement Send + Sync + 'static.
    ///
    /// # Examples
    ///
    /// ```
    /// use ferrous_di::{ServiceCollection, Lifetime};
    /// use std::sync::Arc;
    ///
    /// #[derive(Debug)]
    /// struct ServiceMetadata {
    ///     description: String,
    ///     version: String,
    /// }
    ///
    /// let mut services = ServiceCollection::new();
    /// services.add_with_metadata(
    ///     42usize,
    ///     Lifetime::Singleton,
    ///     ServiceMetadata {
    ///         description: "Answer to everything".to_string(),
    ///         version: "1.0".to_string(),
    ///     }
    /// );
    /// ```
    pub fn add_with_metadata<T, M>(&mut self, value: T, lifetime: Lifetime, metadata: M) -> &mut Self
    where
        T: 'static + Send + Sync,
        M: Send + Sync + 'static,
    {
        let arc = Arc::new(value);
        let key = Key::Type(TypeId::of::<T>(), std::any::type_name::<T>());
        let ctor = move |_: &ResolverContext| -> DiResult<AnyArc> {
            Ok(arc.clone())
        };
        self.registry.insert(key, Registration::with_metadata(
            lifetime,
            Arc::new(ctor),
            Some(Box::new(metadata)),
            Some(TypeId::of::<T>()),
        ));
        self
    }
    
    /// Get metadata for a specific service key.
    ///
    /// Returns the metadata if it exists and can be downcast to the specified type.
    ///
    /// # Examples
    ///
    /// ```
    /// # use ferrous_di::{ServiceCollection, Lifetime, Key};
    /// # use std::any::TypeId;
    /// # #[derive(Debug, PartialEq)]
    /// # struct ServiceMetadata { description: String }
    /// # let mut services = ServiceCollection::new();
    /// # services.add_with_metadata(42usize, Lifetime::Singleton, ServiceMetadata { description: "test".to_string() });
    /// let key = Key::Type(TypeId::of::<usize>(), "usize");
    /// let metadata = services.get_metadata::<ServiceMetadata>(&key);
    /// assert!(metadata.is_some());
    /// ```
    pub fn get_metadata<M: 'static>(&self, key: &Key) -> Option<&M> {
        self.registry.get(key)?
            .metadata.as_ref()?
            .downcast_ref::<M>()
    }
    
    // ----- Conditional Registration (TryAdd*) -----
    
    /// Register a singleton if not already registered.
    ///
    /// This method only registers the service if no service of type `T` is currently registered.
    /// It returns `true` if the service was registered, `false` if it was already registered.
    ///
    /// # Examples
    ///
    /// ```
    /// use ferrous_di::ServiceCollection;
    ///
    /// let mut services = ServiceCollection::new();
    /// 
    /// let registered1 = services.try_add_singleton(42usize);
    /// assert!(registered1); // First registration succeeds
    /// 
    /// let registered2 = services.try_add_singleton(100usize);
    /// assert!(!registered2); // Second registration is ignored
    /// ```
    pub fn try_add_singleton<T: 'static + Send + Sync>(&mut self, value: T) -> bool {
        let key = Key::Type(TypeId::of::<T>(), std::any::type_name::<T>());
        if self.registry.contains_key(&key) {
            false
        } else {
            self.add_singleton(value);
            true
        }
    }
    
    /// Register a singleton factory if not already registered.
    pub fn try_add_singleton_factory<T, F>(&mut self, factory: F) -> bool
    where
        T: 'static + Send + Sync,
        F: Fn(&ResolverContext) -> T + Send + Sync + 'static,
    {
        let key = Key::Type(TypeId::of::<T>(), std::any::type_name::<T>());
        if self.registry.contains_key(&key) {
            false
        } else {
            self.add_singleton_factory(factory);
            true
        }
    }
    
    /// Register a scoped factory if not already registered.
    pub fn try_add_scoped_factory<T, F>(&mut self, factory: F) -> bool
    where
        T: 'static + Send + Sync,
        F: Fn(&ResolverContext) -> T + Send + Sync + 'static,
    {
        let key = Key::Type(TypeId::of::<T>(), std::any::type_name::<T>());
        if self.registry.contains_key(&key) {
            false
        } else {
            self.add_scoped_factory(factory);
            true
        }
    }
    
    /// Register a transient factory if not already registered.
    pub fn try_add_transient_factory<T, F>(&mut self, factory: F) -> bool
    where
        T: 'static + Send + Sync,
        F: Fn(&ResolverContext) -> T + Send + Sync + 'static,
    {
        let key = Key::Type(TypeId::of::<T>(), std::any::type_name::<T>());
        if self.registry.contains_key(&key) {
            false
        } else {
            self.add_transient_factory(factory);
            true
        }
    }
    
    /// Register a singleton trait if not already registered.
    pub fn try_add_singleton_trait<T>(&mut self, value: Arc<T>) -> bool
    where
        T: ?Sized + 'static + Send + Sync,
    {
        let key = Key::Trait(std::any::type_name::<T>());
        if self.registry.contains_key(&key) {
            false
        } else {
            self.add_singleton_trait(value);
            true
        }
    }
    
    /// Register a singleton trait factory if not already registered.
    pub fn try_add_singleton_trait_factory<Trait, F>(&mut self, factory: F) -> bool
    where
        Trait: ?Sized + 'static + Send + Sync,
        F: Fn(&ResolverContext) -> Arc<Trait> + Send + Sync + 'static,
    {
        let key = Key::Trait(std::any::type_name::<Trait>());
        if self.registry.contains_key(&key) {
            false
        } else {
            self.add_singleton_trait_factory(factory);
            true
        }
    }
    
    /// Register a scoped trait factory if not already registered.
    pub fn try_add_scoped_trait_factory<Trait, F>(&mut self, factory: F) -> bool
    where
        Trait: ?Sized + 'static + Send + Sync,
        F: Fn(&ResolverContext) -> Arc<Trait> + Send + Sync + 'static,
    {
        let key = Key::Trait(std::any::type_name::<Trait>());
        if self.registry.contains_key(&key) {
            false
        } else {
            self.add_scoped_trait_factory(factory);
            true
        }
    }
    
    /// Register a transient trait factory if not already registered.
    pub fn try_add_transient_trait_factory<Trait, F>(&mut self, factory: F) -> bool
    where
        Trait: ?Sized + 'static + Send + Sync,
        F: Fn(&ResolverContext) -> Arc<Trait> + Send + Sync + 'static,
    {
        let key = Key::Trait(std::any::type_name::<Trait>());
        if self.registry.contains_key(&key) {
            false
        } else {
            self.add_transient_trait_factory(factory);
            true
        }
    }
    
    /// Add enumerable trait registration (always adds, doesn't check for existing).
    ///
    /// This method is equivalent to `add_trait_implementation` but with a name that matches
    /// Microsoft.Extensions.DependencyInjection conventions.
    pub fn try_add_enumerable<T>(&mut self, value: Arc<T>, lifetime: Lifetime) -> &mut Self
    where
        T: ?Sized + 'static + Send + Sync,
    {
        // For enumerable services, we always add (no conditional logic)
        self.add_trait_implementation(value, lifetime)
    }
    
    // ----- Named Service Registration -----
    
    /// Register a named singleton service.
    ///
    /// Named services allow multiple registrations of the same type distinguished by name.
    /// This is useful for scenarios like multiple database connections, different configurations, etc.
    ///
    /// # Examples
    ///
    /// ```
    /// use ferrous_di::ServiceCollection;
    ///
    /// let mut services = ServiceCollection::new();
    /// services.add_named_singleton("primary", 42usize);
    /// services.add_named_singleton("secondary", 100usize);
    /// 
    /// let provider = services.build();
    /// // These would be resolved separately by name
    /// ```
    pub fn add_named_singleton<T: 'static + Send + Sync>(&mut self, name: &'static str, value: T) -> &mut Self {
        let arc = Arc::new(value);
        let key = Key::TypeNamed(TypeId::of::<T>(), std::any::type_name::<T>(), name);
        let ctor = move |_: &ResolverContext| -> DiResult<AnyArc> {
            Ok(arc.clone())
        };
        self.registry.insert(key, Registration::with_metadata(
            Lifetime::Singleton,
            Arc::new(ctor),
            None,
            Some(TypeId::of::<T>()),
        ));
        self
    }
    
    /// Register a named singleton factory.
    pub fn add_named_singleton_factory<T, F>(&mut self, name: &'static str, factory: F) -> &mut Self
    where
        T: 'static + Send + Sync,
        F: Fn(&ResolverContext) -> T + Send + Sync + 'static,
    {
        let key = Key::TypeNamed(TypeId::of::<T>(), std::any::type_name::<T>(), name);
        let factory = Arc::new(factory);
        let ctor = move |r: &ResolverContext| -> DiResult<AnyArc> {
            Ok(Arc::new(factory(r)))
        };
        self.registry.insert(key, Registration::with_metadata(
            Lifetime::Singleton,
            Arc::new(ctor),
            None,
            Some(TypeId::of::<T>()),
        ));
        self
    }
    
    /// Register a named scoped factory.
    pub fn add_named_scoped_factory<T, F>(&mut self, name: &'static str, factory: F) -> &mut Self
    where
        T: 'static + Send + Sync,
        F: Fn(&ResolverContext) -> T + Send + Sync + 'static,
    {
        let key = Key::TypeNamed(TypeId::of::<T>(), std::any::type_name::<T>(), name);
        let factory = Arc::new(factory);
        let ctor = move |r: &ResolverContext| -> DiResult<AnyArc> {
            Ok(Arc::new(factory(r)))
        };
        self.registry.insert(key, Registration::with_metadata(
            Lifetime::Scoped,
            Arc::new(ctor),
            None,
            Some(TypeId::of::<T>()),
        ));
        self
    }
    
    /// Register a named transient factory.
    pub fn add_named_transient_factory<T, F>(&mut self, name: &'static str, factory: F) -> &mut Self
    where
        T: 'static + Send + Sync,
        F: Fn(&ResolverContext) -> T + Send + Sync + 'static,
    {
        let key = Key::TypeNamed(TypeId::of::<T>(), std::any::type_name::<T>(), name);
        let factory = Arc::new(factory);
        let ctor = move |r: &ResolverContext| -> DiResult<AnyArc> {
            Ok(Arc::new(factory(r)))
        };
        self.registry.insert(key, Registration::with_metadata(
            Lifetime::Transient,
            Arc::new(ctor),
            None,
            Some(TypeId::of::<T>()),
        ));
        self
    }
    
    /// Register a named singleton trait.
    pub fn add_named_singleton_trait<T>(&mut self, name: &'static str, value: Arc<T>) -> &mut Self
    where
        T: ?Sized + 'static + Send + Sync,
    {
        let key = Key::TraitNamed(std::any::type_name::<T>(), name);
        let any_arc: AnyArc = Arc::new(value.clone());
        let ctor = move |_: &ResolverContext| -> DiResult<AnyArc> {
            Ok(any_arc.clone())
        };
        self.registry.insert(key, Registration::with_metadata(
            Lifetime::Singleton,
            Arc::new(ctor),
            None,
            None, // We don't know the concrete implementation type for trait objects
        ));
        self
    }
    
    /// Register a named singleton trait factory.
    pub fn add_named_singleton_trait_factory<Trait, F>(&mut self, name: &'static str, factory: F) -> &mut Self
    where
        Trait: ?Sized + 'static + Send + Sync,
        F: Fn(&ResolverContext) -> Arc<Trait> + Send + Sync + 'static,
    {
        let key = Key::TraitNamed(std::any::type_name::<Trait>(), name);
        let factory = Arc::new(factory);
        let ctor = move |r: &ResolverContext| -> DiResult<AnyArc> {
            Ok(Arc::new(factory(r)))
        };
        self.registry.insert(key, Registration::with_metadata(
            Lifetime::Singleton,
            Arc::new(ctor),
            None,
            None,
        ));
        self
    }
    
    /// Register a named scoped trait factory.
    pub fn add_named_scoped_trait_factory<Trait, F>(&mut self, name: &'static str, factory: F) -> &mut Self
    where
        Trait: ?Sized + 'static + Send + Sync,
        F: Fn(&ResolverContext) -> Arc<Trait> + Send + Sync + 'static,
    {
        let key = Key::TraitNamed(std::any::type_name::<Trait>(), name);
        let factory = Arc::new(factory);
        let ctor = move |r: &ResolverContext| -> DiResult<AnyArc> {
            Ok(Arc::new(factory(r)))
        };
        self.registry.insert(key, Registration::with_metadata(
            Lifetime::Scoped,
            Arc::new(ctor),
            None,
            None,
        ));
        self
    }
    
    /// Register a named transient trait factory.
    pub fn add_named_transient_trait_factory<Trait, F>(&mut self, name: &'static str, factory: F) -> &mut Self
    where
        Trait: ?Sized + 'static + Send + Sync,
        F: Fn(&ResolverContext) -> Arc<Trait> + Send + Sync + 'static,
    {
        let key = Key::TraitNamed(std::any::type_name::<Trait>(), name);
        let factory = Arc::new(factory);
        let ctor = move |r: &ResolverContext| -> DiResult<AnyArc> {
            Ok(Arc::new(factory(r)))
        };
        self.registry.insert(key, Registration::with_metadata(
            Lifetime::Transient,
            Arc::new(ctor),
            None,
            None,
        ));
        self
    }
    
    /// Add named multi-trait registration.
    pub fn add_named_trait_implementation<T>(&mut self, name: &'static str, value: Arc<T>, lifetime: Lifetime) -> &mut Self
    where
        T: ?Sized + 'static + Send + Sync,
    {
        let trait_name = std::any::type_name::<T>();
        let any_arc: AnyArc = Arc::new(value.clone());
        let ctor = move |_: &ResolverContext| -> DiResult<AnyArc> {
            Ok(any_arc.clone())
        };
        
        // For named multi-trait, we need to create unique keys with names
        // We'll use a combination approach: store in many with a combined key
        let combined_key = format!("{}#{}", trait_name, name);
        let static_key: &'static str = Box::leak(combined_key.into_boxed_str());
        
        self.registry.many.entry(static_key).or_default().push(Registration::with_metadata(
            lifetime,
            Arc::new(ctor),
            None,
            None,
        ));
        self
    }
    
    // ----- Observer Management -----
    
    /// Adds a diagnostic observer for DI resolution events.
    ///
    /// Observers enable structured tracing and monitoring of the dependency injection
    /// container's behavior. This is particularly valuable for agentic systems where
    /// you need to correlate DI events with agent execution steps and debug complex
    /// resolution chains.
    ///
    /// # Performance
    ///
    /// Observer calls are made synchronously during resolution. Keep observer
    /// implementations lightweight to avoid impacting performance.
    ///
    /// # Examples
    ///
    /// ```
    /// use ferrous_di::{ServiceCollection, LoggingObserver, DiObserver};
    /// use std::sync::Arc;
    ///
    /// // Using the built-in logging observer
    /// let mut services = ServiceCollection::new();
    /// services.add_observer(Arc::new(LoggingObserver::new()));
    ///
    /// // Using a custom observer
    /// struct MetricsObserver {
    ///     counter: std::sync::Arc<std::sync::atomic::AtomicU64>,
    /// }
    ///
    /// impl DiObserver for MetricsObserver {
    ///     fn resolving(&self, key: &ferrous_di::Key) {
    ///         self.counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    ///     }
    ///
    ///     fn resolved(&self, _key: &ferrous_di::Key, _duration: std::time::Duration) {}
    ///     fn factory_panic(&self, _key: &ferrous_di::Key, _message: &str) {}
    /// }
    ///
    /// let counter = Arc::new(std::sync::atomic::AtomicU64::new(0));
    /// services.add_observer(Arc::new(MetricsObserver { counter: counter.clone() }));
    ///
    /// let provider = services.build();
    /// // All resolutions will be observed
    /// ```
    pub fn add_observer(&mut self, observer: Arc<dyn DiObserver>) -> &mut Self {
        self.observers.add(observer);
        self
    }
    
    // ----- Decoration / Interceptors -----
    
    /// Decorates all registrations of a trait with a wrapper function.
    ///
    /// This enables cross-cutting concerns like logging, timeouts, retries, rate limiting,
    /// authentication, and PII scrubbing without modifying the original implementations.
    /// The decorator function is applied to both single-binding and multi-binding registrations.
    ///
    /// This is particularly powerful for agentic systems where you need to apply consistent
    /// policies across all tools or services.
    ///
    /// # Arguments
    ///
    /// * `decorator` - A function that takes an `Arc<T>` and returns a wrapped `Arc<T>`
    ///
    /// # Examples
    ///
    /// ```
    /// use ferrous_di::{ServiceCollection, Resolver};
    /// use std::sync::Arc;
    ///
    /// trait Tool: Send + Sync {
    ///     fn execute(&self, input: &str) -> String;
    /// }
    ///
    /// struct FileTool;
    /// impl Tool for FileTool {
    ///     fn execute(&self, input: &str) -> String {
    ///         format!("File operation: {}", input)
    ///     }
    /// }
    ///
    /// struct LoggingWrapper<T: ?Sized> {
///     inner: Arc<T>,
/// }
///
/// impl<T: ?Sized> LoggingWrapper<T> {
///     fn new(inner: Arc<T>) -> Self { Self { inner } }
/// }
///
/// impl<T: Tool + ?Sized> Tool for LoggingWrapper<T> {
    ///     fn execute(&self, input: &str) -> String {
    ///         println!("Executing tool with input: {}", input);
    ///         let result = self.inner.execute(input);
    ///         println!("Tool result: {}", result);
    ///         result
    ///     }
    /// }
    ///
    /// let mut services = ServiceCollection::new();
    /// 
    /// // Register tools
    /// services.add_singleton_trait::<dyn Tool>(Arc::new(FileTool));
    ///
    /// // Apply logging to all tools
    /// services.decorate_trait::<dyn Tool, _>(|tool| {
    ///     Arc::new(LoggingWrapper::new(tool))
    /// });
    ///
    /// let provider = services.build();
    /// let tool = provider.get_required_trait::<dyn Tool>();
    /// let result = tool.execute("test.txt");
    /// // Logs: "Executing tool with input: test.txt"
    /// // Logs: "Tool result: File operation: test.txt"
    /// ```
    pub fn decorate_trait<T, F>(&mut self, decorator: F) -> &mut Self
    where
        T: ?Sized + 'static + Send + Sync,
        F: Fn(Arc<T>) -> Arc<T> + Send + Sync + 'static,
    {
        let trait_name = std::any::type_name::<T>();
        let decorator = Arc::new(decorator);
        
        // Decorate single-binding registration if it exists
        let single_key = Key::Trait(trait_name);
        if let Some(registration) = self.registry.get_mut(&single_key) {
            let old_ctor = registration.ctor.clone();
            let decorator_clone = decorator.clone();
            
            registration.ctor = Arc::new(move |resolver| {
                // Call original constructor
                let original = old_ctor(resolver)?;
                
                // Cast to the trait type and apply decorator
                let typed = original.downcast::<Arc<T>>()
                    .map_err(|_| DiError::TypeMismatch(trait_name))?;
                let decorated = decorator_clone((*typed).clone());
                
                // Wrap back in Arc<dyn Any>
                Ok(Arc::new(decorated))
            });
        }
        
        // Decorate multi-binding registrations if they exist
        if let Some(registrations) = self.registry.many.get_mut(trait_name) {
            for registration in registrations.iter_mut() {
                let old_ctor = registration.ctor.clone();
                let decorator_clone = decorator.clone();
                
                registration.ctor = Arc::new(move |resolver| {
                    // Call original constructor
                    let original = old_ctor(resolver)?;
                    
                    // Cast to the trait type and apply decorator
                    let typed = original.downcast::<Arc<T>>()
                        .map_err(|_| DiError::TypeMismatch(trait_name))?;
                    let decorated = decorator_clone((*typed).clone());
                    
                    // Wrap back in Arc<dyn Any>
                    Ok(Arc::new(decorated))
                });
            }
        }
        
        self
    }

    /// Decorates a concrete service type with a first-class decorator.
    ///
    /// This is the modern, type-safe approach to service decoration that provides
    /// access to the resolver during decoration. Perfect for workflow engines that
    /// need to inject dependencies during decoration.
    ///
    /// # Examples
    ///
    /// ```
    /// use ferrous_di::{ServiceCollection, ServiceDecorator, Resolver};
    /// use std::sync::Arc;
    ///
    /// struct UserService {
    ///     name: String,
    /// }
    ///
    /// struct LoggingDecorator;
    ///
    /// impl ServiceDecorator<UserService> for LoggingDecorator {
    ///     fn decorate(&self, original: Arc<UserService>, _resolver: &dyn ferrous_di::traits::ResolverCore) -> Arc<UserService> {
    ///         println!("Accessing user: {}", original.name);
    ///         original
    ///     }
    /// }
    ///
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut services = ServiceCollection::new();
    /// services.add_singleton(UserService { name: "Alice".to_string() });
    /// services.decorate_with::<UserService, _>(LoggingDecorator);
    ///
    /// let provider = services.build();
    /// let user = provider.get_required::<UserService>(); // Logs: "Accessing user: Alice"
    /// # Ok(())
    /// # }
    /// ```
    pub fn decorate_with<T, D>(&mut self, decorator: D) -> &mut Self
    where
        T: 'static + Send + Sync,
        D: crate::decoration::ServiceDecorator<T> + 'static,
    {
        use crate::decoration::DecorationWrapper;

        let key = crate::key::key_of_type::<T>();
        
        if let Some(registration) = self.registry.get_mut(&key) {
            let old_ctor = registration.ctor.clone();
            let wrapper = Arc::new(DecorationWrapper::new(decorator));
            
            registration.ctor = Arc::new(move |resolver| {
                // Call original constructor
                let original = old_ctor(resolver)?;
                
                // Cast to the concrete type
                let typed = original.downcast::<T>()
                    .map_err(|_| crate::DiError::TypeMismatch(std::any::type_name::<T>()))?;
                
                // Apply decoration
                let decorated = wrapper.decorate(typed, resolver);
                
                // Wrap back in Arc<dyn Any>
                Ok(decorated as crate::registration::AnyArc)
            });
        }
        
        self
    }

    /// Decorates a trait service type with a first-class decorator.
    ///
    /// Similar to `decorate_with` but works with trait objects for maximum flexibility.
    /// Essential for workflow engines that need to wrap trait implementations.
    ///
    /// # Examples
    ///
    /// ```
    /// use ferrous_di::{ServiceCollection, TraitDecorator, Resolver};
    /// use std::sync::Arc;
    ///
    /// trait Logger: Send + Sync {
    ///     fn log(&self, message: &str);
    /// }
    ///
    /// struct ConsoleLogger;
    /// impl Logger for ConsoleLogger {
    ///     fn log(&self, message: &str) {
    ///         println!("LOG: {}", message);
    ///     }
    /// }
    ///
    /// struct PrefixDecorator;
    ///
    /// impl TraitDecorator<dyn Logger> for PrefixDecorator {
    ///     fn decorate(&self, original: Arc<dyn Logger>, _resolver: &dyn ferrous_di::traits::ResolverCore) -> Arc<dyn Logger> {
    ///         struct PrefixLogger {
    ///             inner: Arc<dyn Logger>,
    ///         }
    ///         impl Logger for PrefixLogger {
    ///             fn log(&self, message: &str) {
    ///                 self.inner.log(&format!("[WORKFLOW] {}", message));
    ///             }
    ///         }
    ///         Arc::new(PrefixLogger { inner: original })
    ///     }
    /// }
    ///
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut services = ServiceCollection::new();
    /// services.add_singleton_trait::<dyn Logger>(Arc::new(ConsoleLogger));
    /// services.decorate_trait_with::<dyn Logger, _>(PrefixDecorator);
    ///
    /// let provider = services.build();
    /// let logger = provider.get_required_trait::<dyn Logger>();
    /// logger.log("Hello"); // Outputs: "[WORKFLOW] LOG: Hello"
    /// # Ok(())
    /// # }
    /// ```
    pub fn decorate_trait_with<T, D>(&mut self, decorator: D) -> &mut Self
    where
        T: ?Sized + 'static + Send + Sync,
        D: crate::decoration::TraitDecorator<T> + 'static,
    {
        use crate::decoration::TraitDecorationWrapper;

        let trait_name = std::any::type_name::<T>();
        let wrapper = Arc::new(TraitDecorationWrapper::new(decorator));
        
        // Decorate single-binding registration if it exists
        let single_key = crate::Key::Trait(trait_name);
        if let Some(registration) = self.registry.get_mut(&single_key) {
            let old_ctor = registration.ctor.clone();
            let wrapper_clone = wrapper.clone();
            
            registration.ctor = Arc::new(move |resolver| {
                // Call original constructor
                let original = old_ctor(resolver)?;
                
                // Cast to the trait type and apply decorator
                let typed = original.downcast::<Arc<T>>()
                    .map_err(|_| crate::DiError::TypeMismatch(trait_name))?;
                let decorated = wrapper_clone.decorate((*typed).clone(), resolver);
                
                // Wrap back in Arc<dyn Any>
                Ok(Arc::new(decorated) as crate::registration::AnyArc)
            });
        }
        
        // Decorate multi-binding registrations if they exist
        if let Some(registrations) = self.registry.many.get_mut(trait_name) {
            for registration in registrations.iter_mut() {
                let old_ctor = registration.ctor.clone();
                let wrapper_clone = wrapper.clone();
                
                registration.ctor = Arc::new(move |resolver| {
                    // Call original constructor
                    let original = old_ctor(resolver)?;
                    
                    // Cast to the trait type and apply decorator
                    let typed = original.downcast::<Arc<T>>()
                        .map_err(|_| crate::DiError::TypeMismatch(trait_name))?;
                    let decorated = wrapper_clone.decorate((*typed).clone(), resolver);
                    
                    // Wrap back in Arc<dyn Any>
                    Ok(Arc::new(decorated) as crate::registration::AnyArc)
                });
            }
        }
        
        self
    }
    
    // ----- Pre-warm / Readiness -----
    
    /// Marks a concrete service type for pre-warming during startup.
    ///
    /// Pre-warmed services are resolved during the `ServiceProvider::ready()` call,
    /// eliminating cold-start penalties during agent execution. This is particularly
    /// useful for expensive-to-initialize services like ML models, database
    /// connections, and authentication tokens.
    ///
    /// Services that implement `ReadyCheck` will also have their readiness
    /// verified during the pre-warm phase.
    ///
    /// # Examples
    ///
    /// ```
    /// use ferrous_di::{ServiceCollection, ReadyCheck};
    /// use async_trait::async_trait;
    ///
    /// struct DatabaseService {
    ///     connection: String,
    /// }
    ///
    /// #[async_trait]
    /// impl ReadyCheck for DatabaseService {
    ///     async fn ready(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    ///         // Test database connection
    ///         Ok(())
    ///     }
    /// }
    ///
    /// let mut services = ServiceCollection::new();
    /// services.add_singleton(DatabaseService {
    ///     connection: "postgres://localhost".to_string()
    /// });
    /// services.prewarm::<DatabaseService>(); // Pre-warm during startup
    ///
    /// // Later during startup:
    /// let provider = services.build();
    /// // let report = provider.ready().await?; // Resolves and checks DatabaseService
    /// ```
    pub fn prewarm<T: 'static + Send + Sync>(&mut self) -> &mut Self {
        self.prewarm.add_type::<T>();
        self
    }

    /// Marks a trait service type for pre-warming during startup.
    ///
    /// Pre-warmed trait services have all their implementations resolved
    /// during the `ServiceProvider::ready()` call.
    ///
    /// # Examples
    ///
    /// ```
    /// use ferrous_di::{ServiceCollection, ReadyCheck};
    /// use async_trait::async_trait;
    /// use std::sync::Arc;
    ///
    /// trait CacheService: Send + Sync {
    ///     fn get(&self, key: &str) -> Option<String>;
    /// }
    ///
    /// struct RedisCache;
    /// impl CacheService for RedisCache {
    ///     fn get(&self, key: &str) -> Option<String> {
    ///         // Redis implementation
    ///         None
    ///     }
    /// }
    ///
    /// #[async_trait]
    /// impl ReadyCheck for RedisCache {
    ///     async fn ready(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    ///         // Test Redis connection
    ///         Ok(())
    ///     }
    /// }
    ///
    /// let mut services = ServiceCollection::new();
    /// services.add_singleton_trait::<dyn CacheService>(Arc::new(RedisCache));
    /// services.prewarm_trait::<dyn CacheService>(); // Pre-warm all cache implementations
    ///
    /// let provider = services.build();
    /// // let report = provider.ready().await?; // Resolves and checks all cache services
    /// ```
    pub fn prewarm_trait<T: ?Sized + 'static + Send + Sync>(&mut self) -> &mut Self {
        self.prewarm.add_trait::<T>();
        self
    }
    
    /// Builds the final service provider from this collection.
    ///
    /// This method consumes the `ServiceCollection` and creates a `ServiceProvider`
    /// that can resolve registered services. The service provider is thread-safe
    /// and can be used to create scoped contexts for request-scoped services.
    ///
    /// # Returns
    ///
    /// A `ServiceProvider` that can resolve all registered services according to
    /// their configured lifetimes.
    ///
    /// # Examples
    ///
    /// ```
    /// use ferrous_di::{ServiceCollection, Resolver};
    /// use std::sync::Arc;
    ///
    /// let mut collection = ServiceCollection::new();
    /// collection.add_singleton(42usize);
    /// collection.add_transient_factory::<String, _>(|_| "Hello".to_string());
    ///
    /// let provider = collection.build();
    /// let number = provider.get_required::<usize>();
    /// let text = provider.get_required::<String>();
    ///
    /// assert_eq!(*number, 42);
    /// assert_eq!(&*text, "Hello");
    /// ```
    pub fn build(mut self) -> ServiceProvider {
        // Finalize registry by assigning scoped slot indices
        self.registry.finalize();
        ServiceProvider::new_with_observers_and_capabilities(self.registry, self.observers, self.capabilities)
    }

    /// Registers an async singleton service with a factory.
    ///
    /// Perfect for workflow engines where nodes/tools need async initialization
    /// (network handshakes, auth, model warm-up).
    ///
    /// # Examples
    ///
    /// ```
    /// use ferrous_di::{ServiceCollection, AsyncFactory};
    /// use async_trait::async_trait;
    /// use std::sync::Arc;
    ///
    /// struct DatabasePool {
    ///     connection_string: String,
    /// }
    ///
    /// struct AsyncDbPoolFactory;
    ///
    /// #[async_trait]
    /// impl AsyncFactory<DatabasePool> for AsyncDbPoolFactory {
    ///     async fn create(&self, _resolver: &dyn ferrous_di::Resolver) -> Arc<DatabasePool> {
    ///         // Simulate async database connection setup
    ///         Arc::new(DatabasePool {
    ///             connection_string: "postgres://localhost".to_string(),
    ///         })
    ///     }
    /// }
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut services = ServiceCollection::new();
    /// services.add_singleton_async::<DatabasePool, _>(AsyncDbPoolFactory);
    /// # Ok(())
    /// # }
    /// ```
    #[cfg(feature = "async")]
    pub fn add_singleton_async<T, F>(&mut self, factory: F) -> &mut Self
    where
        T: Send + Sync + Clone + 'static,
        F: crate::async_factories::AsyncFactory<T> + 'static,
    {
        use crate::async_factories::AsyncFactoryWrapper;
        
        let wrapper = AsyncFactoryWrapper::new(factory);
        
        // Create a sync factory that executes the async factory in a blocking context
        let sync_factory = move |resolver: &ResolverContext| -> Arc<T> {
            // Try to get current tokio runtime handle
            if let Ok(_handle) = tokio::runtime::Handle::try_current() {
                // We're in an async context, but we can't block_on from within the runtime
                // Instead, use block_in_place to run the async task
                let future = wrapper.create(resolver);
                let result = tokio::task::block_in_place(|| {
                    // Create a new runtime for this blocking task
                    let rt = tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                        .expect("Failed to create blocking runtime");
                    rt.block_on(future)
                });
                
                match result {
                    Ok(result) => result,
                    Err(e) => {
                        panic!("Async factory failed: {}", e);
                    }
                }
            } else {
                // No async runtime, create a new one for this operation
                let rt = tokio::runtime::Builder::new_multi_thread()
                    .enable_all()
                    .build()
                    .expect("Failed to create async runtime");
                match rt.block_on(wrapper.create(resolver)) {
                    Ok(result) => result,
                    Err(e) => {
                        panic!("Async factory failed: {}", e);
                    }
                }
            }
        };
        
        // Register as singleton with the sync factory that returns Arc<T>
        self.add_singleton_factory::<Arc<T>, _>(sync_factory);
        self
    }

    /// Registers an async scoped service with a factory.
    ///
    /// Perfect for per-workflow or per-node async initialization in workflow engines.
    ///
    /// # Examples
    ///
    /// ```
    /// use ferrous_di::{ServiceCollection, async_factory};
    /// use std::sync::Arc;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut services = ServiceCollection::new();
    /// 
    /// services.add_scoped_async::<String, _>(async_factory!(|_resolver| async {
    ///     // Simulate async session initialization
    ///     Arc::new("session_initialized".to_string())
    /// }));
    /// # Ok(())
    /// # }
    /// ```
    #[cfg(feature = "async")]
    pub fn add_scoped_async<T, F>(&mut self, factory: F) -> &mut Self
    where
        T: Send + Sync + Clone + 'static,
        F: crate::async_factories::AsyncFactory<T> + 'static,
    {
        use crate::async_factories::AsyncFactoryWrapper;
        
        let wrapper = AsyncFactoryWrapper::new(factory);
        
        // Create a sync factory that executes the async factory in a blocking context
        let sync_factory = move |resolver: &ResolverContext| -> Arc<T> {
            // Try to get current tokio runtime handle
            if let Ok(_handle) = tokio::runtime::Handle::try_current() {
                // We're in an async context, but we can't block_on from within the runtime
                // Instead, use spawn_blocking to run the async task
                let future = wrapper.create(resolver);
                let result = tokio::task::block_in_place(|| {
                    // Create a new runtime for this blocking task
                    let rt = tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                        .expect("Failed to create blocking runtime");
                    rt.block_on(future)
                });
                
                match result {
                    Ok(result) => result,
                    Err(e) => {
                        panic!("Async factory failed: {}", e);
                    }
                }
            } else {
                // No async runtime, create a new one for this operation
                let rt = tokio::runtime::Builder::new_multi_thread()
                    .enable_all()
                    .build()
                    .expect("Failed to create async runtime");
                match rt.block_on(wrapper.create(resolver)) {
                    Ok(result) => result,
                    Err(e) => {
                        panic!("Async factory failed: {}", e);
                    }
                }
            }
        };
        
        // Register as scoped with the sync factory that returns Arc<T>
        self.add_scoped_factory::<Arc<T>, _>(sync_factory);
        self
    }
}

impl Default for ServiceCollection {
    fn default() -> Self {
        Self::new()
    }
}

