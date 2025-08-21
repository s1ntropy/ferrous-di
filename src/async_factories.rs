//! Async factory support for dependency injection.
//!
//! This module provides async factory capabilities for services that require
//! asynchronous initialization such as database connections, network handshakes,
//! model warm-up, or authentication flows.

use std::sync::Arc;
use async_trait::async_trait;
use crate::traits::ResolverCore;

/// Trait for factories that create services asynchronously.
///
/// This is essential for workflow engines where nodes/tools may need async
/// initialization (network handshakes, auth, model warm-up).
///
/// # Examples
///
/// ```
/// use ferrous_di::{ServiceCollection, AsyncFactory, Resolver};
/// use async_trait::async_trait;
/// use std::sync::Arc;
///
/// struct DatabasePool {
///     connection_string: String,
/// }
///
/// struct AsyncDbPoolFactory {
///     connection_string: String,
/// }
///
/// #[async_trait]
/// impl AsyncFactory<DatabasePool> for AsyncDbPoolFactory {
///     async fn create(&self, _resolver: &dyn Resolver) -> Arc<DatabasePool> {
///         // Simulate async database connection setup
///         tokio::time::sleep(std::time::Duration::from_millis(100)).await;
///         Arc::new(DatabasePool {
///             connection_string: self.connection_string.clone(),
///         })
///     }
/// }
///
/// impl AsyncDbPoolFactory {
///     fn new(connection_string: String) -> Self {
///         Self { connection_string }
///     }
/// }
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let mut services = ServiceCollection::new();
/// services.add_singleton_async::<DatabasePool, _>(
///     AsyncDbPoolFactory::new("postgres://localhost".to_string())
/// );
///
/// let provider = services.build().await?;
/// let db_pool = provider.get_required::<DatabasePool>().await?;
/// # Ok(())
/// # }
/// ```
#[async_trait]
pub trait AsyncFactory<T: Send + Sync + 'static>: Send + Sync {
    /// Creates a new instance of the service asynchronously.
    ///
    /// The resolver can be used to access other services that this
    /// service depends on during its async initialization.
    async fn create(&self, resolver: &dyn ResolverCore) -> Result<Arc<T>, Box<dyn std::error::Error + Send + Sync>>;
}

// Async service state machine for managing initialization.
// This is reserved for future use when implementing lazy async initialization
// with proper state management and error handling.
#[allow(dead_code)]
pub(crate) enum AsyncServiceState<T> {
    /// Service not yet initialized
    Pending,
    /// Service is being initialized (future in progress)
    Initializing,
    /// Service initialization completed
    Ready(Arc<T>),
    /// Service initialization failed
    Failed(String),
}

/// Internal wrapper for async factory functions.
pub(crate) struct AsyncFactoryWrapper<T> {
    factory: Box<dyn AsyncFactory<T>>,
    // State would be managed by the ServiceProvider
    // using OnceCell or similar for thread-safe lazy initialization
}

impl<T: Send + Sync + 'static> AsyncFactoryWrapper<T> {
    pub fn new(factory: impl AsyncFactory<T> + 'static) -> Self {
        Self {
            factory: Box::new(factory),
        }
    }

    pub async fn create(&self, resolver: &dyn ResolverCore) -> Result<Arc<T>, Box<dyn std::error::Error + Send + Sync>> {
        self.factory.create(resolver).await
    }
}

/// Helper trait for creating async factories from closures.
#[async_trait]
impl<T, F, Fut> AsyncFactory<T> for F
where
    T: Send + Sync + 'static,
    F: Fn(&dyn ResolverCore) -> Fut + Send + Sync,
    Fut: std::future::Future<Output = Result<Arc<T>, Box<dyn std::error::Error + Send + Sync>>> + Send,
{
    async fn create(&self, resolver: &dyn ResolverCore) -> Result<Arc<T>, Box<dyn std::error::Error + Send + Sync>> {
        self(resolver).await
    }
}

/// Macro for creating async factories from async closures.
///
/// # Examples
///
/// ```
/// use ferrous_di::{async_factory, ServiceCollection};
/// use std::sync::Arc;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let mut services = ServiceCollection::new();
/// 
/// services.add_singleton_async::<String, _>(async_factory!(|_resolver| async {
///     // Simulate async initialization
///     tokio::time::sleep(std::time::Duration::from_millis(50)).await;
///     Arc::new("initialized async service".to_string())
/// }));
/// # Ok(())
/// # }
/// ```
#[macro_export]
macro_rules! async_factory {
    (|$resolver:ident| async $body:block) => {
        move |$resolver: &dyn $crate::ResolverCore| async move {
            Ok($body)
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{traits::ResolverCore, Key, DiResult};
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::time::Duration;

    struct AsyncService {
        init_count: Arc<AtomicU32>,
        value: String,
    }

    struct AsyncServiceFactory {
        init_count: Arc<AtomicU32>,
    }

    #[async_trait]
    impl AsyncFactory<AsyncService> for AsyncServiceFactory {
        async fn create(&self, _resolver: &dyn ResolverCore) -> Result<Arc<AsyncService>, Box<dyn std::error::Error + Send + Sync>> {
            // Simulate async initialization
            tokio::time::sleep(Duration::from_millis(10)).await;
            self.init_count.fetch_add(1, Ordering::Relaxed);
            
            Ok(Arc::new(AsyncService {
                init_count: self.init_count.clone(),
                value: "async initialized".to_string(),
            }))
        }
    }

    #[tokio::test]
    async fn test_async_factory_wrapper() {
        let init_count = Arc::new(AtomicU32::new(0));
        let factory = AsyncServiceFactory {
            init_count: init_count.clone(),
        };
        
        let wrapper = AsyncFactoryWrapper::new(factory);
        
        // Create a mock resolver (in real implementation, this would be a proper resolver)
        struct MockResolver;
        impl ResolverCore for MockResolver {
            fn resolve_any(&self, _key: &Key) -> DiResult<Arc<dyn std::any::Any + Send + Sync>> {
                Err(crate::DiError::NotFound("Mock"))
            }
            
            fn resolve_many(&self, _key: &Key) -> DiResult<Vec<Arc<dyn std::any::Any + Send + Sync>>> {
                Ok(vec![])
            }
            
            fn push_sync_disposer(&self, _f: Box<dyn FnOnce() + Send>) {}
            
            fn push_async_disposer(&self, _f: Box<dyn FnOnce() -> crate::internal::BoxFutureUnit + Send>) {}
        }
        
        let resolver = MockResolver;
        let service = wrapper.create(&resolver).await.unwrap();
        
        assert_eq!(service.value, "async initialized");
        assert_eq!(init_count.load(Ordering::Relaxed), 1);
    }

    #[tokio::test] 
    async fn test_async_factory_from_closure() {
        let init_count = Arc::new(AtomicU32::new(0));
        let init_count_clone = init_count.clone();
        
        let factory = move |_resolver: &dyn ResolverCore| {
            let count = init_count_clone.clone();
            async move {
                tokio::time::sleep(Duration::from_millis(5)).await;
                count.fetch_add(1, Ordering::Relaxed);
                Ok(Arc::new("closure async service".to_string()))
            }
        };
        
        struct MockResolver;
        impl ResolverCore for MockResolver {
            fn resolve_any(&self, _key: &Key) -> DiResult<Arc<dyn std::any::Any + Send + Sync>> {
                Err(crate::DiError::NotFound("Mock"))
            }
            
            fn resolve_many(&self, _key: &Key) -> DiResult<Vec<Arc<dyn std::any::Any + Send + Sync>>> {
                Ok(vec![])
            }
            
            fn push_sync_disposer(&self, _f: Box<dyn FnOnce() + Send>) {}
            
            fn push_async_disposer(&self, _f: Box<dyn FnOnce() -> crate::internal::BoxFutureUnit + Send>) {}
        }
        
        let resolver = MockResolver;
        let service = factory.create(&resolver).await.unwrap();
        
        assert_eq!(*service, "closure async service");
        assert_eq!(init_count.load(Ordering::Relaxed), 1);
    }
}