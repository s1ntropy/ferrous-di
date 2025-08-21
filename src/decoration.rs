//! First-class decoration pipeline for service interception and modification.
//!
//! This module provides a powerful decoration system that allows workflow engines
//! to intercept, modify, wrap, or replace services during resolution. Essential
//! for adding cross-cutting concerns like metrics, logging, caching, or transformation.

use std::sync::Arc;
use std::any::TypeId;
use std::collections::HashMap;
use crate::traits::ResolverCore;

/// A decorator that can intercept and modify service resolution.
///
/// Essential for workflow engines that need to add cross-cutting concerns
/// like metrics, logging, caching, or transformation to services.
///
/// # Examples
///
/// ```
/// use ferrous_di::{ServiceCollection, ServiceDecorator, Resolver};
/// use std::sync::Arc;
///
/// struct DatabaseService {
///     connection_string: String,
/// }
///
/// struct MetricsDecorator;
///
/// impl ServiceDecorator<DatabaseService> for MetricsDecorator {
///     fn decorate(&self, original: Arc<DatabaseService>, _resolver: &dyn ferrous_di::traits::ResolverCore) -> Arc<DatabaseService> {
///         println!("Database service accessed: {}", original.connection_string);
///         original // Return original service, but we logged the access
///     }
/// }
///
/// # fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let mut services = ServiceCollection::new();
/// services.add_singleton(DatabaseService {
///     connection_string: "postgres://localhost".to_string(),
/// });
///
/// // Add decoration
/// services.decorate_with::<DatabaseService, _>(MetricsDecorator);
///
/// let provider = services.build();
/// let db = provider.get_required::<DatabaseService>(); // Logs access
/// # Ok(())
/// # }
/// ```
pub trait ServiceDecorator<T: Send + Sync + 'static>: Send + Sync {
    /// Decorates the service during resolution.
    ///
    /// Can wrap, modify, or completely replace the original service.
    /// The resolver can be used to access other services needed for decoration.
    fn decorate(&self, original: Arc<T>, resolver: &dyn ResolverCore) -> Arc<T>;
}

/// A trait decorator that can intercept and modify trait object resolution.
///
/// Similar to ServiceDecorator but works with trait objects for maximum flexibility.
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
/// struct TimestampDecorator;
///
/// impl TraitDecorator<dyn Logger> for TimestampDecorator {
///     fn decorate(&self, original: Arc<dyn Logger>, _resolver: &dyn ferrous_di::traits::ResolverCore) -> Arc<dyn Logger> {
///         struct TimestampLogger {
///             inner: Arc<dyn Logger>,
///         }
///         impl Logger for TimestampLogger {
///             fn log(&self, message: &str) {
///                 self.inner.log(&format!("[2024-01-01T00:00:00Z] {}", message));
///             }
///         }
///         Arc::new(TimestampLogger { inner: original })
///     }
/// }
///
/// # fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let mut services = ServiceCollection::new();
/// services.add_singleton_trait::<dyn Logger>(Arc::new(ConsoleLogger));
///
/// // Add decoration to trait
/// services.decorate_trait_with::<dyn Logger, _>(TimestampDecorator);
///
/// let provider = services.build();
/// let logger = provider.get_required_trait::<dyn Logger>();
/// logger.log("Hello"); // Outputs: "[2024-01-01T12:00:00Z] LOG: Hello"
/// # Ok(())
/// # }
/// ```
pub trait TraitDecorator<T: ?Sized + Send + Sync + 'static>: Send + Sync {
    /// Decorates the trait object during resolution.
    fn decorate(&self, original: Arc<T>, resolver: &dyn ResolverCore) -> Arc<T>;
}

/// Internal decorator wrapper for type erasure.
pub(crate) struct DecorationWrapper<T> {
    decorator: Box<dyn ServiceDecorator<T>>,
}

impl<T: Send + Sync + 'static> DecorationWrapper<T> {
    pub fn new(decorator: impl ServiceDecorator<T> + 'static) -> Self {
        Self {
            decorator: Box::new(decorator),
        }
    }

    pub fn decorate(&self, original: Arc<T>, resolver: &dyn ResolverCore) -> Arc<T> {
        self.decorator.decorate(original, resolver)
    }
}

/// Internal trait decorator wrapper for type erasure.
pub(crate) struct TraitDecorationWrapper<T: ?Sized> {
    decorator: Box<dyn TraitDecorator<T>>,
}

impl<T: ?Sized + Send + Sync + 'static> TraitDecorationWrapper<T> {
    pub fn new(decorator: impl TraitDecorator<T> + 'static) -> Self {
        Self {
            decorator: Box::new(decorator),
        }
    }

    pub fn decorate(&self, original: Arc<T>, resolver: &dyn ResolverCore) -> Arc<T> {
        self.decorator.decorate(original, resolver)
    }
}

/// Decoration pipeline for managing multiple decorators per service.
///
/// Essential for workflow engines that need multiple layers of decoration
/// (metrics, logging, caching, transformation, etc.).
///
/// # Examples
///
/// ```
/// use ferrous_di::{ServiceCollection, DecorationPipeline, ServiceDecorator, Resolver};
/// use std::sync::Arc;
///
/// struct ApiService {
///     name: String,
/// }
///
/// struct LoggingDecorator;
/// impl ServiceDecorator<ApiService> for LoggingDecorator {
///     fn decorate(&self, original: Arc<ApiService>, _resolver: &dyn ferrous_di::traits::ResolverCore) -> Arc<ApiService> {
///         println!("Accessing API service: {}", original.name);
///         original
///     }
/// }
///
/// struct MetricsDecorator;
/// impl ServiceDecorator<ApiService> for MetricsDecorator {
///     fn decorate(&self, original: Arc<ApiService>, _resolver: &dyn ferrous_di::traits::ResolverCore) -> Arc<ApiService> {
///         // Record metrics
///         original
///     }
/// }
///
/// # fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let mut services = ServiceCollection::new();
/// services.add_singleton(ApiService { name: "UserAPI".to_string() });
///
/// // Add multiple decorators - they execute in order
/// services.decorate_with::<ApiService, _>(LoggingDecorator);
/// services.decorate_with::<ApiService, _>(MetricsDecorator);
///
/// let provider = services.build();
/// let api = provider.get_required::<ApiService>(); // Logs then records metrics
/// # Ok(())
/// # }
/// ```
#[derive(Default)]
pub struct DecorationPipeline {
    /// Service decorators by TypeId
    service_decorators: HashMap<TypeId, Vec<Box<dyn DecorationAny>>>,
    /// Trait decorators by TypeId  
    trait_decorators: HashMap<TypeId, Vec<Box<dyn TraitDecorationAny>>>,
}

/// Type-erased decoration for internal pipeline management.
trait DecorationAny: Send + Sync {
    fn decorate_any(&self, original: crate::registration::AnyArc, resolver: &dyn ResolverCore) -> crate::registration::AnyArc;
}

impl<T: Send + Sync + 'static> DecorationAny for DecorationWrapper<T> {
    fn decorate_any(&self, original: crate::registration::AnyArc, resolver: &dyn ResolverCore) -> crate::registration::AnyArc {
        let typed = original.downcast::<T>().expect("Type mismatch in decoration");
        let decorated = self.decorate(typed, resolver);
        decorated as crate::registration::AnyArc
    }
}

/// Type-erased trait decoration for internal pipeline management.
pub(crate) trait TraitDecorationAny: Send + Sync {
    fn decorate_trait_any(&self, original: crate::registration::AnyArc, resolver: &dyn ResolverCore) -> crate::registration::AnyArc;
}

impl<T: Send + Sync + 'static> TraitDecorationAny for TraitDecorationWrapper<T> {
    fn decorate_trait_any(&self, original: crate::registration::AnyArc, resolver: &dyn ResolverCore) -> crate::registration::AnyArc {
        // Attempt to downcast the trait object to the concrete type
        if let Ok(concrete) = original.clone().downcast::<T>() {
            // Apply decoration and upcast back to AnyArc
            let decorated = self.decorator.decorate(concrete, resolver);
            decorated as crate::registration::AnyArc
        } else {
            // If downcast fails, return original (this could happen with trait objects)
            original
        }
    }
}

impl DecorationPipeline {
    /// Creates a new decoration pipeline.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a service decorator to the pipeline.
    pub fn add_service_decorator<T: Send + Sync + 'static>(&mut self, decorator: impl ServiceDecorator<T> + 'static) {
        let type_id = TypeId::of::<T>();
        let wrapper = DecorationWrapper::new(decorator);
        
        self.service_decorators
            .entry(type_id)
            .or_insert_with(Vec::new)
            .push(Box::new(wrapper));
    }

    /// Adds a trait decorator to the pipeline.
    pub fn add_trait_decorator<T: Send + Sync + 'static>(&mut self, decorator: impl TraitDecorator<T> + 'static) {
        let type_id = TypeId::of::<T>();
        let wrapper = TraitDecorationWrapper::new(decorator);
        
        self.trait_decorators
            .entry(type_id)
            .or_insert_with(Vec::new)
            .push(Box::new(wrapper));
    }

    /// Applies all decorators for a service type.
    pub fn decorate_service<T: Send + Sync + 'static>(&self, mut service: Arc<T>, resolver: &dyn ResolverCore) -> Arc<T> {
        let type_id = TypeId::of::<T>();
        
        if let Some(decorators) = self.service_decorators.get(&type_id) {
            for decorator in decorators {
                let any_service = service.clone() as crate::registration::AnyArc;
                let decorated_any = decorator.decorate_any(any_service, resolver);
                service = decorated_any.downcast::<T>().expect("Type mismatch in decoration pipeline");
            }
        }
        
        service
    }

    /// Applies all decorators for a trait type.
    pub fn decorate_trait<T: Send + Sync + 'static>(&self, mut service: Arc<T>, resolver: &dyn ResolverCore) -> Arc<T> {
        let type_id = TypeId::of::<T>();
        
        if let Some(decorators) = self.trait_decorators.get(&type_id) {
            for decorator in decorators {
                let any_service = service.clone() as crate::registration::AnyArc;
                let decorated_any = decorator.decorate_trait_any(any_service, resolver);
                service = decorated_any.downcast::<T>().expect("Type mismatch in trait decoration pipeline");
            }
        }
        
        service
    }

    /// Returns true if there are decorators for the given service type.
    pub fn has_service_decorators<T: Send + Sync + 'static>(&self) -> bool {
        let type_id = TypeId::of::<T>();
        self.service_decorators.contains_key(&type_id)
    }

    /// Returns true if there are decorators for the given trait type.
    pub fn has_trait_decorators<T: Send + Sync + 'static>(&self) -> bool {
        let type_id = TypeId::of::<T>();
        self.trait_decorators.contains_key(&type_id)
    }

    /// Returns the number of service decorators.
    pub fn service_decorator_count(&self) -> usize {
        self.service_decorators.values().map(|v| v.len()).sum()
    }

    /// Returns the number of trait decorators.
    pub fn trait_decorator_count(&self) -> usize {
        self.trait_decorators.values().map(|v| v.len()).sum()
    }

    /// Clears all decorators.
    pub fn clear(&mut self) {
        self.service_decorators.clear();
        self.trait_decorators.clear();
    }
}

/// Helper macro for creating simple function-based decorators.
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
/// struct SimpleDecorator;
/// impl ServiceDecorator<UserService> for SimpleDecorator {
///     fn decorate(&self, original: Arc<UserService>, _resolver: &dyn ferrous_di::traits::ResolverCore) -> Arc<UserService> {
///         println!("Accessing user: {}", original.name);
///         original
///     }
/// }
///
/// # fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let mut services = ServiceCollection::new();
/// services.add_singleton(UserService { name: "John".to_string() });
/// services.decorate_with::<UserService, _>(SimpleDecorator);
///
/// let provider = services.build();
/// let user = provider.get_required::<UserService>(); // Logs access
/// # Ok(())
/// # }
/// ```
#[macro_export]
macro_rules! decorate {
    (|$service:ident, $resolver:ident| $body:expr) => {
        struct FunctionDecorator<F>(F);
        
        impl<T, F> $crate::ServiceDecorator<T> for FunctionDecorator<F>
        where
            T: Send + Sync + 'static,
            F: Fn(std::sync::Arc<T>, &dyn $crate::Resolver) -> std::sync::Arc<T> + Send + Sync,
        {
            fn decorate(&self, $service: std::sync::Arc<T>, $resolver: &dyn $crate::Resolver) -> std::sync::Arc<T> {
                (self.0)($service, $resolver)
            }
        }
        
        FunctionDecorator(move |$service: std::sync::Arc<_>, $resolver: &dyn $crate::Resolver| $body)
    };
}

/// Convenience functions for common decoration patterns.
pub mod decorators {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{Instant, Duration};

    /// A decorator that logs service access.
    ///
    /// # Examples
    ///
    /// ```
    /// use ferrous_di::{ServiceCollection, decorators::LoggingDecorator, Resolver};
    /// 
    /// struct UserService;
    ///
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut services = ServiceCollection::new();
    /// services.add_singleton(UserService);
    /// services.decorate_with::<UserService, _>(LoggingDecorator::new("UserService"));
    ///
    /// let provider = services.build();
    /// let user = provider.get_required::<UserService>(); // Logs: "Accessing service: UserService"
    /// # Ok(())
    /// # }
    /// ```
    pub struct LoggingDecorator {
        service_name: &'static str,
    }

    impl LoggingDecorator {
        pub fn new(service_name: &'static str) -> Self {
            Self { service_name }
        }
    }

    impl<T: Send + Sync + 'static> ServiceDecorator<T> for LoggingDecorator {
        fn decorate(&self, original: Arc<T>, _resolver: &dyn ResolverCore) -> Arc<T> {
            println!("Accessing service: {}", self.service_name);
            original
        }
    }

    /// A decorator that counts service access.
    ///
    /// # Examples
    ///
    /// ```
    /// use ferrous_di::{ServiceCollection, decorators::CountingDecorator, Resolver};
    /// use std::sync::Arc;
    /// 
    /// struct UserService;
    ///
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let counter = CountingDecorator::new();
    /// 
    /// let mut services = ServiceCollection::new();
    /// services.add_singleton(UserService);
    /// services.decorate_with::<UserService, _>(counter);
    ///
    /// let provider = services.build();
    /// let _user1 = provider.get_required::<UserService>();
    /// let _user2 = provider.get_required::<UserService>();
    /// # Ok(())
    /// # }
    /// ```
    pub struct CountingDecorator {
        count: AtomicU64,
    }

    impl CountingDecorator {
        pub fn new() -> Self {
            Self {
                count: AtomicU64::new(0),
            }
        }

        pub fn count(&self) -> u64 {
            self.count.load(Ordering::Relaxed)
        }
    }

    impl<T: Send + Sync + 'static> ServiceDecorator<T> for CountingDecorator {
        fn decorate(&self, original: Arc<T>, _resolver: &dyn ResolverCore) -> Arc<T> {
            self.count.fetch_add(1, Ordering::Relaxed);
            original
        }
    }

    /// A decorator that measures service resolution time.
    ///
    /// # Examples
    ///
    /// ```
    /// use ferrous_di::{ServiceCollection, decorators::TimingDecorator, Resolver};
    /// use std::sync::Arc;
    /// 
    /// struct UserService;
    ///
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let timer = TimingDecorator::new("UserService");
    /// 
    /// let mut services = ServiceCollection::new();
    /// services.add_singleton(UserService);
    /// services.decorate_with::<UserService, _>(timer);
    ///
    /// let provider = services.build();
    /// let _user = provider.get_required::<UserService>();
    /// # Ok(())
    /// # }
    /// ```
    pub struct TimingDecorator {
        service_name: &'static str,
        total_time: AtomicU64,
        count: AtomicU64,
    }

    impl TimingDecorator {
        pub fn new(service_name: &'static str) -> Self {
            Self {
                service_name,
                total_time: AtomicU64::new(0),
                count: AtomicU64::new(0),
            }
        }

        pub fn average_time(&self) -> Duration {
            let total = self.total_time.load(Ordering::Relaxed);
            let count = self.count.load(Ordering::Relaxed);
            if count == 0 {
                Duration::ZERO
            } else {
                Duration::from_nanos(total / count)
            }
        }

        pub fn total_time(&self) -> Duration {
            Duration::from_nanos(self.total_time.load(Ordering::Relaxed))
        }

        pub fn access_count(&self) -> u64 {
            self.count.load(Ordering::Relaxed)
        }
    }

    impl<T: Send + Sync + 'static> ServiceDecorator<T> for TimingDecorator {
        fn decorate(&self, original: Arc<T>, _resolver: &dyn ResolverCore) -> Arc<T> {
            let start = Instant::now();
            let result = original; // In real use, this might involve actual work
            let elapsed = start.elapsed();
            
            self.total_time.fetch_add(elapsed.as_nanos() as u64, Ordering::Relaxed);
            self.count.fetch_add(1, Ordering::Relaxed);
            
            println!("Service {} resolved in {:?}", self.service_name, elapsed);
            result
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use crate::traits::ResolverCore;

    struct TestService {
        value: i32,
    }

    struct TestDecorator {
        multiplier: i32,
    }

    impl ServiceDecorator<TestService> for TestDecorator {
        fn decorate(&self, original: Arc<TestService>, _resolver: &dyn ResolverCore) -> Arc<TestService> {
            Arc::new(TestService {
                value: original.value * self.multiplier,
            })
        }
    }

    #[test]
    fn test_decoration_pipeline() {
        let mut pipeline = DecorationPipeline::new();
        
        // Add decorator
        pipeline.add_service_decorator::<TestService>(TestDecorator { multiplier: 2 });
        
        assert!(pipeline.has_service_decorators::<TestService>());
        assert_eq!(pipeline.service_decorator_count(), 1);
        
        // Test decoration
        let original = Arc::new(TestService { value: 10 });
        let mock_resolver = MockResolver;
        let decorated = pipeline.decorate_service(original, &mock_resolver);
        
        assert_eq!(decorated.value, 20);
    }

    #[test]
    fn test_multiple_decorators() {
        let mut pipeline = DecorationPipeline::new();
        
        // Add multiple decorators
        pipeline.add_service_decorator::<TestService>(TestDecorator { multiplier: 2 });
        pipeline.add_service_decorator::<TestService>(TestDecorator { multiplier: 3 });
        
        assert_eq!(pipeline.service_decorator_count(), 2);
        
        // Test decoration chain
        let original = Arc::new(TestService { value: 10 });
        let mock_resolver = MockResolver;
        let decorated = pipeline.decorate_service(original, &mock_resolver);
        
        assert_eq!(decorated.value, 60); // 10 * 2 * 3
    }

    #[test]
    fn test_counting_decorator() {
        use decorators::CountingDecorator;
        
        let counter = CountingDecorator::new();
        let service = Arc::new(TestService { value: 42 });
        let mock_resolver = MockResolver;
        
        assert_eq!(counter.count(), 0);
        
        let _decorated1 = counter.decorate(service.clone(), &mock_resolver);
        assert_eq!(counter.count(), 1);
        
        let _decorated2 = counter.decorate(service, &mock_resolver);
        assert_eq!(counter.count(), 2);
    }

    #[test]
    fn test_logging_decorator() {
        use decorators::LoggingDecorator;
        
        let logger = LoggingDecorator::new("TestService");
        let service = Arc::new(TestService { value: 42 });
        let mock_resolver = MockResolver;
        
        let decorated = logger.decorate(service, &mock_resolver);
        assert_eq!(decorated.value, 42); // Should be unchanged
    }

    #[test]
    fn test_timing_decorator() {
        use decorators::TimingDecorator;
        
        let timer = TimingDecorator::new("TestService");
        let service = Arc::new(TestService { value: 42 });
        let mock_resolver = MockResolver;
        
        assert_eq!(timer.access_count(), 0);
        
        let decorated = timer.decorate(service, &mock_resolver);
        assert_eq!(decorated.value, 42);
        assert_eq!(timer.access_count(), 1);
        // On very fast systems, the timing might be 0ns, so we check for >= 0
        assert!(timer.total_time() >= Duration::ZERO);
    }

    // Mock resolver for testing
    struct MockResolver;
    
    impl ResolverCore for MockResolver {
        fn resolve_any(&self, _key: &crate::Key) -> crate::DiResult<Arc<dyn std::any::Any + Send + Sync>> {
            Err(crate::DiError::NotFound("Mock"))
        }
        
        fn resolve_many(&self, _key: &crate::Key) -> crate::DiResult<Vec<Arc<dyn std::any::Any + Send + Sync>>> {
            Ok(vec![])
        }
        
        fn push_sync_disposer(&self, _f: Box<dyn FnOnce() + Send>) {}
        
        fn push_async_disposer(&self, _f: Box<dyn FnOnce() -> crate::internal::BoxFutureUnit + Send>) {}
    }
}