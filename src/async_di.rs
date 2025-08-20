//! Async/await support for ferrous-di.
//!
//! This module provides asynchronous service resolution, async factories,
//! and integration with async runtimes like Tokio.

use async_trait::async_trait;
use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};

use crate::{DiResult, DiError, Key, Lifetime, ServiceDescriptor};

/// Type alias for async service factories
pub type AsyncFactory<T> = Arc<
    dyn Fn(Arc<AsyncServiceProvider>) -> Pin<Box<dyn Future<Output = DiResult<Arc<T>>> + Send>>
        + Send
        + Sync,
>;

/// Type alias for async any factory
pub type AsyncAnyFactory = Arc<
    dyn Fn(Arc<AsyncServiceProvider>) -> Pin<Box<dyn Future<Output = DiResult<Arc<dyn Any + Send + Sync>>> + Send>>
        + Send
        + Sync,
>;

/// Async service provider for resolving services asynchronously
#[derive(Clone)]
pub struct AsyncServiceProvider {
    inner: Arc<AsyncProviderInner>,
}

struct AsyncProviderInner {
    /// Service registry
    registry: AsyncRegistry,
    /// Singleton cache
    singletons: RwLock<HashMap<Key, Arc<dyn Any + Send + Sync>>>,
    /// Async factories for services
    factories: RwLock<HashMap<Key, AsyncAnyFactory>>,
}

/// Async service registry
#[derive(Clone, Default)]
pub struct AsyncRegistry {
    descriptors: Arc<RwLock<Vec<ServiceDescriptor>>>,
}

impl AsyncRegistry {
    /// Create a new async registry
    pub fn new() -> Self {
        Self {
            descriptors: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Register a service descriptor
    pub async fn register(&self, descriptor: ServiceDescriptor) {
        let mut descriptors = self.descriptors.write().await;
        descriptors.push(descriptor);
    }

    /// Get all descriptors
    pub async fn get_all(&self) -> Vec<ServiceDescriptor> {
        let descriptors = self.descriptors.read().await;
        descriptors.clone()
    }

    /// Find a descriptor by key
    pub async fn find(&self, key: &Key) -> Option<ServiceDescriptor> {
        let descriptors = self.descriptors.read().await;
        descriptors.iter().find(|d| d.key == *key).cloned()
    }
}

impl AsyncServiceProvider {
    /// Create a new async service provider
    pub fn new(registry: AsyncRegistry) -> Self {
        Self {
            inner: Arc::new(AsyncProviderInner {
                registry,
                singletons: RwLock::new(HashMap::new()),
                factories: RwLock::new(HashMap::new()),
            }),
        }
    }

    /// Register an async factory for a service
    pub async fn register_factory<T>(&self, key: Key, factory: AsyncFactory<T>)
    where
        T: Send + Sync + 'static,
    {
        let any_factory: AsyncAnyFactory = Arc::new(move |provider| {
            let factory = factory.clone();
            Box::pin(async move {
                let result = factory(provider).await?;
                Ok(result as Arc<dyn Any + Send + Sync>)
            })
        });

        let mut factories = self.inner.factories.write().await;
        factories.insert(key, any_factory);
    }

    /// Resolve a service asynchronously
    pub async fn resolve<T>(&self) -> DiResult<Arc<T>>
    where
        T: Send + Sync + 'static,
    {
        let key = Key::Type(TypeId::of::<T>(), std::any::type_name::<T>());
        self.resolve_by_key(&key).await
    }

    /// Resolve a service by key
    pub async fn resolve_by_key<T>(&self, key: &Key) -> DiResult<Arc<T>>
    where
        T: Send + Sync + 'static,
    {
        // Check if it's a singleton and already cached
        if let Some(descriptor) = self.inner.registry.find(key).await {
            if descriptor.lifetime == Lifetime::Singleton {
                let singletons = self.inner.singletons.read().await;
                if let Some(service) = singletons.get(key) {
                    // The service is stored as Arc<dyn Any>, we need to clone and downcast
                    return service.clone()
                        .downcast::<T>()
                        .map_err(|_| DiError::TypeMismatch("Type mismatch in singleton cache"));
                }
            }
        }

        // Try to create using factory
        let factories = self.inner.factories.read().await;
        if let Some(factory) = factories.get(key) {
            let provider = Arc::new(self.clone());
            let service = factory(provider).await?;
            
            // Cache if singleton
            if let Some(descriptor) = self.inner.registry.find(key).await {
                if descriptor.lifetime == Lifetime::Singleton {
                    let mut singletons = self.inner.singletons.write().await;
                    singletons.insert(key.clone(), service.clone());
                }
            }

            service.clone()
                .downcast::<T>()
                .map_err(|_| DiError::TypeMismatch("Failed to downcast service"))
        } else {
            Err(DiError::NotFound(std::any::type_name::<T>()))
        }
    }

    /// Create an async scope for scoped services
    pub async fn create_scope(self: Arc<Self>) -> AsyncScope {
        AsyncScope::new(self).await
    }
}

/// Async scope for scoped service resolution
pub struct AsyncScope {
    provider: Arc<AsyncServiceProvider>,
    scoped_services: Arc<RwLock<HashMap<Key, Arc<dyn Any + Send + Sync>>>>,
    disposal_handles: Arc<Mutex<Vec<Box<dyn AsyncDisposable>>>>,
}

impl AsyncScope {
    /// Create a new async scope
    async fn new(provider: Arc<AsyncServiceProvider>) -> Self {
        Self {
            provider,
            scoped_services: Arc::new(RwLock::new(HashMap::new())),
            disposal_handles: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Resolve a scoped service
    pub async fn resolve<T>(&self) -> DiResult<Arc<T>>
    where
        T: Send + Sync + 'static,
    {
        let key = Key::Type(TypeId::of::<T>(), std::any::type_name::<T>());
        
        // Check if already resolved in this scope
        {
            let scoped = self.scoped_services.read().await;
            if let Some(service) = scoped.get(&key) {
                return service.clone()
                    .downcast::<T>()
                    .map_err(|_| DiError::TypeMismatch("Type mismatch in scoped cache"));
            }
        }

        // Check service lifetime
        if let Some(descriptor) = self.provider.inner.registry.find(&key).await {
            match descriptor.lifetime {
                Lifetime::Singleton => {
                    // Delegate to provider for singletons
                    self.provider.resolve_by_key(&key).await
                }
                Lifetime::Scoped => {
                    // Create and cache in scope
                    let service = self.create_service(&key).await?;
                    let mut scoped = self.scoped_services.write().await;
                    scoped.insert(key.clone(), service.clone());
                    
                    service.clone()
                        .downcast::<T>()
                        .map_err(|_| DiError::TypeMismatch("Failed to downcast scoped service"))
                }
                Lifetime::Transient => {
                    // Always create new instance
                    let service = self.create_service(&key).await?;
                    service.clone()
                        .downcast::<T>()
                        .map_err(|_| DiError::TypeMismatch("Failed to downcast transient service"))
                }
            }
        } else {
            Err(DiError::NotFound(std::any::type_name::<T>()))
        }
    }

    /// Create a service using its factory
    async fn create_service(&self, key: &Key) -> DiResult<Arc<dyn Any + Send + Sync>> {
        let factories = self.provider.inner.factories.read().await;
        if let Some(factory) = factories.get(key) {
            let provider = self.provider.clone();
            factory(provider).await
        } else {
            Err(DiError::NotFound("Factory not found"))
        }
    }

    /// Add a disposal handle for cleanup
    pub async fn add_disposal<D: AsyncDisposable + 'static>(&self, disposable: D) {
        let mut handles = self.disposal_handles.lock().await;
        handles.push(Box::new(disposable));
    }

    /// Dispose all scoped services
    pub async fn dispose(self) {
        // Clear scoped services
        self.scoped_services.write().await.clear();
        
        // Run disposal handles
        let mut handles = self.disposal_handles.lock().await;
        for handle in handles.drain(..) {
            handle.dispose().await;
        }
    }
}

/// Trait for async disposable resources
#[async_trait]
pub trait AsyncDisposable: Send + Sync {
    /// Dispose the resource asynchronously
    async fn dispose(self: Box<Self>);
}

/// Async service collection builder
pub struct AsyncServiceCollection {
    registry: AsyncRegistry,
    registrations: Vec<AsyncRegistration>,
    descriptors: Vec<ServiceDescriptor>,
}

struct AsyncRegistration {
    key: Key,
    lifetime: Lifetime,
    factory: AsyncAnyFactory,
}

impl AsyncServiceCollection {
    /// Create a new async service collection
    pub fn new() -> Self {
        Self {
            registry: AsyncRegistry::new(),
            registrations: Vec::new(),
            descriptors: Vec::new(),
        }
    }

    /// Add a singleton service with async factory
    pub fn add_singleton_async<T, F, Fut>(&mut self, factory: F) -> &mut Self
    where
        T: Send + Sync + 'static,
        F: Fn(Arc<AsyncServiceProvider>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = DiResult<T>> + Send + 'static,
    {
        let key = Key::Type(TypeId::of::<T>(), std::any::type_name::<T>());
        let lifetime = Lifetime::Singleton;
        
        let boxed_factory: AsyncFactory<T> = Arc::new(move |provider| {
            let fut = factory(provider);
            Box::pin(async move { Ok(Arc::new(fut.await?)) })
        });

        let any_factory: AsyncAnyFactory = Arc::new(move |provider| {
            let factory = boxed_factory.clone();
            Box::pin(async move {
                let result = factory(provider).await?;
                Ok(result as Arc<dyn Any + Send + Sync>)
            })
        });

        self.registrations.push(AsyncRegistration {
            key: key.clone(),
            lifetime,
            factory: any_factory,
        });

        let descriptor = ServiceDescriptor {
            key,
            lifetime,
            impl_type_id: Some(TypeId::of::<T>()),
            impl_type_name: Some(std::any::type_name::<T>()),
            has_metadata: false,
        };

        self.descriptors.push(descriptor);
        self
    }

    /// Add a scoped service with async factory
    pub fn add_scoped_async<T, F, Fut>(&mut self, factory: F) -> &mut Self
    where
        T: Send + Sync + 'static,
        F: Fn(Arc<AsyncServiceProvider>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = DiResult<T>> + Send + 'static,
    {
        let key = Key::Type(TypeId::of::<T>(), std::any::type_name::<T>());
        let lifetime = Lifetime::Scoped;
        
        let boxed_factory: AsyncFactory<T> = Arc::new(move |provider| {
            let fut = factory(provider);
            Box::pin(async move { Ok(Arc::new(fut.await?)) })
        });

        let any_factory: AsyncAnyFactory = Arc::new(move |provider| {
            let factory = boxed_factory.clone();
            Box::pin(async move {
                let result = factory(provider).await?;
                Ok(result as Arc<dyn Any + Send + Sync>)
            })
        });

        self.registrations.push(AsyncRegistration {
            key: key.clone(),
            lifetime,
            factory: any_factory,
        });

        let descriptor = ServiceDescriptor {
            key,
            lifetime,
            impl_type_id: Some(TypeId::of::<T>()),
            impl_type_name: Some(std::any::type_name::<T>()),
            has_metadata: false,
        };

        self.descriptors.push(descriptor);
        self
    }

    /// Add a transient service with async factory
    pub fn add_transient_async<T, F, Fut>(&mut self, factory: F) -> &mut Self
    where
        T: Send + Sync + 'static,
        F: Fn(Arc<AsyncServiceProvider>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = DiResult<T>> + Send + 'static,
    {
        let key = Key::Type(TypeId::of::<T>(), std::any::type_name::<T>());
        let lifetime = Lifetime::Transient;
        
        let boxed_factory: AsyncFactory<T> = Arc::new(move |provider| {
            let fut = factory(provider);
            Box::pin(async move { Ok(Arc::new(fut.await?)) })
        });

        let any_factory: AsyncAnyFactory = Arc::new(move |provider| {
            let factory = boxed_factory.clone();
            Box::pin(async move {
                let result = factory(provider).await?;
                Ok(result as Arc<dyn Any + Send + Sync>)
            })
        });

        self.registrations.push(AsyncRegistration {
            key: key.clone(),
            lifetime,
            factory: any_factory,
        });

        let descriptor = ServiceDescriptor {
            key,
            lifetime,
            impl_type_id: Some(TypeId::of::<T>()),
            impl_type_name: Some(std::any::type_name::<T>()),
            has_metadata: false,
        };

        self.descriptors.push(descriptor);
        self
    }

    /// Build the async service provider
    pub async fn build(self) -> Arc<AsyncServiceProvider> {
        // Register all descriptors first
        for descriptor in self.descriptors {
            self.registry.register(descriptor).await;
        }
        
        let provider = Arc::new(AsyncServiceProvider::new(self.registry));
        
        // Register all factories
        for registration in self.registrations {
            let mut factories = provider.inner.factories.write().await;
            factories.insert(registration.key, registration.factory);
        }

        provider
    }
}

impl Default for AsyncServiceCollection {
    fn default() -> Self {
        Self::new()
    }
}

/// Async resolver trait for dependency injection
#[async_trait]
pub trait AsyncResolver: Send + Sync {
    /// Resolve a service asynchronously
    async fn resolve<T>(&self) -> DiResult<Arc<T>>
    where
        T: Send + Sync + 'static;

    /// Resolve an optional service
    async fn resolve_optional<T>(&self) -> Option<Arc<T>>
    where
        T: Send + Sync + 'static;

    /// Resolve a required service (panics if not found)
    async fn resolve_required<T>(&self) -> Arc<T>
    where
        T: Send + Sync + 'static;
}

#[async_trait]
impl AsyncResolver for AsyncServiceProvider {
    async fn resolve<T>(&self) -> DiResult<Arc<T>>
    where
        T: Send + Sync + 'static,
    {
        self.resolve().await
    }

    async fn resolve_optional<T>(&self) -> Option<Arc<T>>
    where
        T: Send + Sync + 'static,
    {
        self.resolve().await.ok()
    }

    async fn resolve_required<T>(&self) -> Arc<T>
    where
        T: Send + Sync + 'static,
    {
        self.resolve()
            .await
            .unwrap_or_else(|e| panic!("Failed to resolve required service: {}", e))
    }
}

#[async_trait]
impl AsyncResolver for AsyncScope {
    async fn resolve<T>(&self) -> DiResult<Arc<T>>
    where
        T: Send + Sync + 'static,
    {
        self.resolve().await
    }

    async fn resolve_optional<T>(&self) -> Option<Arc<T>>
    where
        T: Send + Sync + 'static,
    {
        self.resolve().await.ok()
    }

    async fn resolve_required<T>(&self) -> Arc<T>
    where
        T: Send + Sync + 'static,
    {
        self.resolve()
            .await
            .unwrap_or_else(|e| panic!("Failed to resolve required service: {}", e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone)]
    struct TestService {
        value: String,
    }

    #[derive(Debug, Clone)]
    struct DependentService {
        test_service: Arc<TestService>,
        id: u32,
    }

    #[tokio::test]
    async fn test_async_singleton_resolution() {
        let mut collection = AsyncServiceCollection::new();
        
        collection.add_singleton_async(|_provider| async {
            Ok(TestService {
                value: "singleton".to_string(),
            })
        });

        let provider = collection.build().await;
        
        let service1 = provider.resolve::<TestService>().await.unwrap();
        let service2 = provider.resolve::<TestService>().await.unwrap();
        
        // Should be the same instance
        assert!(Arc::ptr_eq(&service1, &service2));
        assert_eq!(service1.value, "singleton");
    }

    #[tokio::test]
    async fn test_async_scoped_resolution() {
        let mut collection = AsyncServiceCollection::new();
        
        let counter = Arc::new(tokio::sync::Mutex::new(0));
        let counter_clone = counter.clone();
        
        collection.add_scoped_async(move |_provider| {
            let counter = counter_clone.clone();
            async move {
                let mut count = counter.lock().await;
                *count += 1;
                Ok(TestService {
                    value: format!("scoped_{}", *count),
                })
            }
        });

        let provider = collection.build().await;
        
        // Create first scope
        let scope1 = provider.clone().create_scope().await;
        let service1_1 = scope1.resolve::<TestService>().await.unwrap();
        let service1_2 = scope1.resolve::<TestService>().await.unwrap();
        
        // Same instance within scope
        assert!(Arc::ptr_eq(&service1_1, &service1_2));
        
        // Create second scope
        let scope2 = provider.create_scope().await;
        let service2_1 = scope2.resolve::<TestService>().await.unwrap();
        
        // Different instance in different scope
        assert!(!Arc::ptr_eq(&service1_1, &service2_1));
        assert_eq!(service1_1.value, "scoped_1");
        assert_eq!(service2_1.value, "scoped_2");
    }

    #[tokio::test]
    async fn test_async_transient_resolution() {
        let mut collection = AsyncServiceCollection::new();
        
        let counter = Arc::new(tokio::sync::Mutex::new(0));
        let counter_clone = counter.clone();
        
        collection.add_transient_async(move |_provider| {
            let counter = counter_clone.clone();
            async move {
                let mut count = counter.lock().await;
                *count += 1;
                Ok(TestService {
                    value: format!("transient_{}", *count),
                })
            }
        });

        let provider = collection.build().await;
        
        let service1 = provider.resolve::<TestService>().await.unwrap();
        let service2 = provider.resolve::<TestService>().await.unwrap();
        
        // Should be different instances
        assert!(!Arc::ptr_eq(&service1, &service2));
        assert_eq!(service1.value, "transient_1");
        assert_eq!(service2.value, "transient_2");
    }

    #[tokio::test]
    async fn test_async_dependency_injection() {
        let mut collection = AsyncServiceCollection::new();
        
        collection.add_singleton_async(|_provider| async {
            Ok(TestService {
                value: "base_service".to_string(),
            })
        });

        collection.add_scoped_async(|provider| async move {
            let test_service = provider.resolve::<TestService>().await?;
            Ok(DependentService {
                test_service,
                id: 42,
            })
        });

        let provider = collection.build().await;
        let scope = provider.create_scope().await;
        
        let dependent = scope.resolve::<DependentService>().await.unwrap();
        assert_eq!(dependent.test_service.value, "base_service");
        assert_eq!(dependent.id, 42);
    }

    struct TestDisposable {
        disposed: Arc<tokio::sync::Mutex<bool>>,
    }

    #[async_trait]
    impl AsyncDisposable for TestDisposable {
        async fn dispose(self: Box<Self>) {
            let mut disposed = self.disposed.lock().await;
            *disposed = true;
        }
    }

    #[tokio::test]
    async fn test_async_scope_disposal() {
        let disposed = Arc::new(tokio::sync::Mutex::new(false));
        let disposed_clone = disposed.clone();
        
        let mut collection = AsyncServiceCollection::new();
        let provider = collection.build().await;
        
        {
            let scope = provider.create_scope().await;
            let disposable = TestDisposable {
                disposed: disposed_clone,
            };
            scope.add_disposal(disposable).await;
            
            // Dispose the scope
            scope.dispose().await;
        }
        
        // Check that disposal was called
        assert!(*disposed.lock().await);
    }
}