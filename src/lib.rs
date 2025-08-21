//! # ferrous-di
//!
//! Type-safe, performant dependency injection for Rust, inspired by Microsoft.Extensions.DependencyInjection.
//!
//! ## Features
//!
//! - **Type-safe lifetimes**: Singleton, Scoped, and Transient services
//! - **Trait support**: Single and multi-binding trait resolution
//! - **Thread-safe**: Arc-based sharing with proper lifetime management  
//! - **Circular dependency detection**: Prevents infinite loops with detailed error paths
//! - **Scoped isolation**: Request-scoped services with proper cleanup
//! - **Zero-cost abstractions**: Compile-time type safety with runtime performance
//!
//! ## Quick Start
//!
//! ```rust
//! use ferrous_di::{ServiceCollection, Resolver};
//! use std::sync::Arc;
//!
//! // Define your services
//! struct Database {
//!     connection_string: String,
//! }
//!
//! struct UserService {
//!     db: Arc<Database>,
//! }
//!
//! // Register services
//! let mut services = ServiceCollection::new();
//! services.add_singleton(Database {
//!     connection_string: "postgres://localhost".to_string(),
//! });
//! services.add_transient_factory::<UserService, _>(|resolver| {
//!     UserService {
//!         db: resolver.get_required::<Database>(),
//!     }
//! });
//!
//! // Build and use the service provider
//! let provider = services.build();
//! let user_service = provider.get_required::<UserService>();
//! assert_eq!(user_service.db.connection_string, "postgres://localhost");
//! ```
//!
//! ## Service Lifetimes
//!
//! - **Singleton**: Created once and shared across the entire application
//! - **Scoped**: Created once per scope (ideal for web request contexts)
//! - **Transient**: Created fresh on every resolution
//!
//! ## Trait Resolution
//!
//! ```rust
//! use ferrous_di::{ServiceCollection, Resolver};
//! use std::sync::Arc;
//!
//! trait Logger: Send + Sync {
//!     fn log(&self, message: &str);
//! }
//!
//! struct ConsoleLogger;
//! impl Logger for ConsoleLogger {
//!     fn log(&self, message: &str) {
//!         println!("[LOG] {}", message);
//!     }
//! }
//!
//! let mut services = ServiceCollection::new();
//! services.add_singleton_trait::<dyn Logger>(Arc::new(ConsoleLogger));
//!
//! let provider = services.build();
//! let logger = provider.get_required_trait::<dyn Logger>();
//! logger.log("Hello, World!");
//! ```
//!
//! ## Scoped Services
//!
//! ```rust
//! use ferrous_di::{ServiceCollection, Resolver};
//! use std::sync::{Arc, Mutex};
//!
//! struct RequestId(String);
//!
//! let mut services = ServiceCollection::new();
//! let counter = Arc::new(Mutex::new(0));
//! let counter_clone = counter.clone();
//!
//! services.add_scoped_factory::<RequestId, _>(move |_| {
//!     let mut c = counter_clone.lock().unwrap();
//!     *c += 1;
//!     RequestId(format!("req-{}", *c))
//! });
//!
//! let provider = services.build();
//! let scope1 = provider.create_scope();
//! let scope2 = provider.create_scope();
//!
//! let req1 = scope1.get_required::<RequestId>();
//! let req2 = scope2.get_required::<RequestId>();
//! // Different scopes get different instances
//! ```

// Module declarations
pub mod collection;
pub mod provider;
pub mod descriptors;
pub mod error;
pub mod key;
pub mod lifetime;
pub mod metrics;
pub mod observer;
pub mod performance;
pub mod prewarm;
pub mod scope_local;
pub mod capabilities;
pub mod validation;
pub mod fast_singletons;
pub mod traits;

#[cfg(feature = "config")]
pub mod config;

#[cfg(feature = "web")]
#[cfg(feature = "async")]
pub mod web_integration;

#[cfg(feature = "axum-integration")]
pub mod axum_integration;

#[cfg(feature = "async")]
pub mod async_factories;
pub mod cancellation;
pub mod labeled_scopes;
pub mod decoration;
pub mod graph_export;

// Internal modules
mod internal;
mod registration;

// Standard library imports for Options pattern
use std::sync::Arc;

// Import for internal use in Options pattern
use self::provider::ResolverContext as InternalResolverContext;

// Re-export core types
pub use collection::{ServiceCollection, ServiceModule, ServiceCollectionExt, ServiceCollectionModuleExt};
pub use provider::{ServiceProvider, Scope, ScopedResolver, ResolverContext};
pub use descriptors::ServiceDescriptor;
pub use error::{DiError, DiResult};
pub use internal::CircularPanic;
pub use key::{Key, key_of_type};
pub use lifetime::Lifetime;
pub use observer::{DiObserver, LoggingObserver, ObservationContext, WorkflowObserver, WorkflowContextProvider, MetricsObserver};
pub use prewarm::{ReadyCheck, ReadinessResult, ReadinessReport};
pub use scope_local::{ScopeLocal, WorkflowContext, ScopeLocalBuilder, workflow};
pub use capabilities::{ToolCapability, CapabilityRequirement, ToolSelectionCriteria, ToolInfo, ToolDiscoveryResult};
pub use validation::{ValidationBuilder, ValidationResult, ValidationError, ValidationWarning};
pub use fast_singletons::{FastSingletonCache, FastSingletonMetrics};
pub use traits::{Dispose, AsyncDispose, Resolver, ResolverCore};

#[cfg(feature = "async")]
pub use async_factories::AsyncFactory;
pub use cancellation::{CancellationToken, CancellationError, ScopeCancellationExt};
pub use labeled_scopes::{LabeledScope, LabeledScopeExt, LabeledScopeContext, LabeledScopeRegistry, ScopeMetadata};
pub use decoration::{ServiceDecorator, TraitDecorator, DecorationPipeline, decorators};
pub use graph_export::{
    DependencyGraph, GraphNode, GraphEdge, GraphMetadata, GraphLayout, NodePosition, LayoutBounds,
    DependencyType, ExportOptions, ExportFormat, GraphBuilder, GraphExporter, DefaultGraphExporter,
    exports, workflow_integration
};

// ===== Options Pattern =====

/// Options interface for dependency injection.
///
/// This trait provides access to immutable configuration snapshots that are resolved
/// once during application startup and remain consistent throughout the application lifetime.
/// 
/// Inspired by Microsoft.Extensions.DependencyInjection's `IOptions<T>` pattern.
///
/// # Examples
///
/// ```
/// use ferrous_di::{ServiceCollection, IOptions, Options, Resolver};
/// use std::sync::Arc;
///
/// #[derive(Default)]
/// struct AppSettings {
///     name: String,
///     debug: bool,
/// }
///
/// let mut services = ServiceCollection::new();
/// services.add_options::<AppSettings>()
///     .configure(|_r, s| {
///         s.name = "MyApp".to_string();
///         s.debug = true;
///     })
///     .register();
///
/// let provider = services.build();
/// let options = provider.get_required::<Options<AppSettings>>();
/// let settings = options.get();
/// assert_eq!(settings.name, "MyApp");
/// assert!(settings.debug);
/// ```
pub trait IOptions<T>: Send + Sync + 'static {
    /// Gets the configured options instance.
    fn get(&self) -> Arc<T>;
}

/// Immutable options wrapper that implements `IOptions<T>`.
///
/// This struct holds an `Arc<T>` containing the final configured options snapshot.
/// Options are built once during container setup and remain immutable thereafter.
///
/// # Examples
///
/// ```
/// use ferrous_di::{ServiceCollection, IOptions, Options, Resolver};
///
/// #[derive(Default)]
/// struct DatabaseConfig {
///     connection_string: String,
///     max_connections: u32,
/// }
///
/// let mut services = ServiceCollection::new();
/// services.add_options::<DatabaseConfig>()
///     .default_with(|| DatabaseConfig {
///         connection_string: "postgres://localhost".to_string(),
///         max_connections: 10,
///     })
///     .validate(|cfg| {
///         if cfg.max_connections == 0 {
///             Err("max_connections must be > 0".to_string())
///         } else {
///             Ok(())
///         }
///     })
///     .register();
///
/// let provider = services.build();
/// let config = provider.get_required::<Options<DatabaseConfig>>();
/// let db_config = config.get();
/// assert_eq!(db_config.max_connections, 10);
/// ```
pub struct Options<T> {
    inner: Arc<T>,
}

impl<T> Options<T> {
    /// Creates a new `Options<T>` wrapping the given value.
    pub fn new(value: T) -> Self {
        Self { inner: Arc::new(value) }
    }

    /// Gets a reference to the inner `Arc<T>`.
    pub fn value(&self) -> &Arc<T> {
        &self.inner
    }

    /// Gets a clone of the inner `Arc<T>` containing the configured options.
    /// This is a convenience method that calls the `IOptions<T>` trait method.
    pub fn get(&self) -> Arc<T> {
        self.inner.clone()
    }
}

impl<T> IOptions<T> for Options<T>
where
    T: Send + Sync + 'static,
{
    fn get(&self) -> Arc<T> {
        self.inner.clone()
    }
}

// Type aliases for common options patterns
type ConfigureFn<T> = Arc<dyn Fn(&InternalResolverContext, &mut T) + Send + Sync>;
type PostConfigureFn<T> = Arc<dyn Fn(&InternalResolverContext, &mut T) + Send + Sync>;
type ValidateFn<T> = Arc<dyn Fn(&T) -> Result<(), String> + Send + Sync>;

/// Options builder for configuring complex options with dependencies.
///
/// The `OptionsBuilder` provides a fluent API for configuring options that depend on
/// other services from the DI container. It supports default value creation, configuration
/// phases, post-configuration for computed values, and validation.
///
/// This builder allows you to configure options using other services from the DI container,
/// post-process them after all configurations are applied, and validate the final result.
/// It follows the builder pattern popularized by Microsoft.Extensions.DependencyInjection.
///
/// # Examples
///
/// ```
/// use ferrous_di::{ServiceCollection, Resolver, Options};
/// use std::sync::Arc;
///
/// #[derive(Default)]
/// struct ApiConfig {
///     base_url: String,
///     timeout_ms: u64,
///     api_key: Option<String>,
/// }
///
/// // Mock config provider
/// trait ConfigProvider: Send + Sync {
///     fn get(&self, key: &str) -> Option<String>;
/// }
/// struct EnvConfig;
/// impl ConfigProvider for EnvConfig {
///     fn get(&self, key: &str) -> Option<String> {
///         std::env::var(key).ok()
///     }
/// }
///
/// let mut services = ServiceCollection::new();
/// services.add_singleton_trait::<dyn ConfigProvider>(Arc::new(EnvConfig));
///
/// services.add_options::<ApiConfig>()
///     .default_with(|| ApiConfig {
///         base_url: "https://api.example.com".to_string(),
///         timeout_ms: 5000,
///         api_key: None,
///     })
///     .configure(|resolver, config| {
///         let provider = resolver.get_required_trait::<dyn ConfigProvider>();
///         if let Some(url) = provider.get("API_BASE_URL") {
///             config.base_url = url;
///         }
///         if let Some(timeout) = provider.get("API_TIMEOUT").and_then(|s| s.parse().ok()) {
///             config.timeout_ms = timeout;
///         }
///         config.api_key = provider.get("API_KEY");
///     })
///     .post_configure(|_resolver, config| {
///         // Normalize the base URL
///         if !config.base_url.ends_with('/') {
///             config.base_url.push('/');
///         }
///     })
///     .validate(|config| {
///         if config.timeout_ms == 0 {
///             return Err("timeout_ms must be greater than 0".to_string());
///         }
///         if config.base_url.is_empty() {
///             return Err("base_url cannot be empty".to_string());
///         }
///         Ok(())
///     })
///     .register();
///
/// let provider = services.build();
/// let options = provider.get_required::<Options<ApiConfig>>();
/// let api_config = options.get();
/// assert!(api_config.timeout_ms > 0);
/// assert!(api_config.base_url.ends_with('/'));
/// ```
pub struct OptionsBuilder<T>
where
    T: Default + Send + Sync + 'static,
{
    sc: *mut ServiceCollection, // raw ptr to allow builder-style API before build()
    default_maker: Option<Arc<dyn Fn() -> T + Send + Sync>>,
    configures: Vec<ConfigureFn<T>>,
    post_configures: Vec<PostConfigureFn<T>>,
    validates: Vec<ValidateFn<T>>,
    // Named options: optional future extension
    _name: Option<&'static str>,
}

impl<T> OptionsBuilder<T>
where
    T: Default + Send + Sync + 'static,
{
    pub(crate) fn new(sc: &mut ServiceCollection) -> Self {
        Self {
            sc,
            default_maker: None,
            configures: Vec::new(),
            post_configures: Vec::new(),
            validates: Vec::new(),
            _name: None,
        }
    }

    /// Provide a custom default value creator (otherwise T::default()).
    ///
    /// This allows you to set up initial values that differ from the type's Default implementation.
    ///
    /// # Examples
    ///
    /// ```
    /// use ferrous_di::{ServiceCollection, IOptions, Options, Resolver};
    ///
    /// #[derive(Default)]
    /// struct ServerConfig {
    ///     host: String,
    ///     port: u16,
    /// }
    ///
    /// let mut services = ServiceCollection::new();
    /// services.add_options::<ServerConfig>()
    ///     .default_with(|| ServerConfig {
    ///         host: "0.0.0.0".to_string(),
    ///         port: 8080,
    ///     })
    ///     .register();
    ///
    /// let provider = services.build();
    /// let options = provider.get_required::<Options<ServerConfig>>();
    /// let config = options.get();
    /// assert_eq!(config.host, "0.0.0.0");
    /// assert_eq!(config.port, 8080);
    /// ```
    pub fn default_with<F>(mut self, f: F) -> Self
    where
        F: Fn() -> T + Send + Sync + 'static,
    {
        self.default_maker = Some(Arc::new(f));
        self
    }

    /// Configure options by providing a callback that can resolve other services from the container.
    ///
    /// Configure callbacks are executed in the order they were added. The callback receives
    /// a resolver that can be used to access other registered services.
    ///
    /// # Examples
    ///
    /// ```
    /// use ferrous_di::{ServiceCollection, IOptions, Options, Resolver};
    ///
    /// #[derive(Default)]
    /// struct AppConfig {
    ///     feature_enabled: bool,
    /// }
    ///
    /// let mut services = ServiceCollection::new();
    /// services.add_singleton("production".to_string()); // Environment name
    ///
    /// services.add_options::<AppConfig>()
    ///     .configure(|resolver, config| {
    ///         let env = resolver.get_required::<String>();
    ///         config.feature_enabled = env.as_str() == "production";
    ///     })
    ///     .register();
    ///
    /// let provider = services.build();
    /// let options = provider.get_required::<Options<AppConfig>>();
    /// let config = options.get();
    /// assert!(config.feature_enabled);
    /// ```
    pub fn configure<F>(mut self, f: F) -> Self
    where
        F: Fn(&InternalResolverContext, &mut T) + Send + Sync + 'static,
    {
        self.configures.push(Arc::new(f));
        self
    }

    /// Post-configure options after all configure actions have been applied.
    ///
    /// Post-configure callbacks run after all configure callbacks and are useful for
    /// computed values, normalization, or cross-field validation and correction.
    ///
    /// # Examples
    ///
    /// ```
    /// use ferrous_di::{ServiceCollection, IOptions, Options, Resolver};
    ///
    /// #[derive(Default)]
    /// struct UrlConfig {
    ///     base_url: String,
    ///     api_path: String,
    ///     full_url: String, // Computed field
    /// }
    ///
    /// let mut services = ServiceCollection::new();
    /// services.add_options::<UrlConfig>()
    ///     .configure(|_resolver, config| {
    ///         config.base_url = "https://api.example.com".to_string();
    ///         config.api_path = "/v1/users".to_string();
    ///     })
    ///     .post_configure(|_resolver, config| {
    ///         // Compute the full URL after base configuration
    ///         config.full_url = format!("{}{}", config.base_url, config.api_path);
    ///     })
    ///     .register();
    ///
    /// let provider = services.build();
    /// let options = provider.get_required::<Options<UrlConfig>>();
    /// let config = options.get();
    /// assert_eq!(config.full_url, "https://api.example.com/v1/users");
    /// ```
    pub fn post_configure<F>(mut self, f: F) -> Self
    where
        F: Fn(&ResolverContext, &mut T) + Send + Sync + 'static,
    {
        self.post_configures.push(Arc::new(f));
        self
    }

    /// Validate the final options after all configuration steps.
    ///
    /// Validation callbacks are executed after all configure and post-configure callbacks.
    /// If any validation fails, the application will panic with a descriptive error message
    /// following the fail-fast principle for configuration errors.
    ///
    /// # Examples
    ///
    /// ```should_panic
    /// use ferrous_di::{ServiceCollection, IOptions, Options, Resolver};
    ///
    /// #[derive(Default)]
    /// struct DatabaseConfig {
    ///     connection_string: String,
    ///     pool_size: u32,
    /// }
    ///
    /// let mut services = ServiceCollection::new();
    /// services.add_options::<DatabaseConfig>()
    ///     .configure(|_resolver, config| {
    ///         config.connection_string = "".to_string(); // Invalid!
    ///         config.pool_size = 0; // Also invalid!
    ///     })
    ///     .validate(|config| {
    ///         if config.connection_string.is_empty() {
    ///             return Err("connection_string cannot be empty".to_string());
    ///         }
    ///         if config.pool_size == 0 {
    ///             return Err("pool_size must be greater than 0".to_string());
    ///         }
    ///         Ok(())
    ///     })
    ///     .register();
    ///
    /// // This will panic during service provider build when Options<DatabaseConfig> is first resolved
    /// let provider = services.build();
    /// let _options = provider.get_required::<Options<DatabaseConfig>>(); // Panics here
    /// ```
    pub fn validate<F>(mut self, f: F) -> Self
    where
        F: Fn(&T) -> Result<(), String> + Send + Sync + 'static,
    {
        self.validates.push(Arc::new(f));
        self
    }

    /// Finish building and register `Options<T>` as a singleton in the DI container.
    ///
    /// This method consumes the builder and registers:
    /// - `Options<T>` as a singleton factory that builds the configured options
    /// 
    /// To access the configured options, resolve `Options<T>` and call `.get()` to get `Arc<T>`.
    ///
    /// The configuration process follows this order:
    /// 1. Create initial value (default_with or T::default())
    /// 2. Run all configure callbacks in order
    /// 3. Run all post_configure callbacks in order  
    /// 4. Run all validate callbacks - panic on any failure
    /// 5. Wrap in `Options<T>` and register as singleton
    ///
    /// # Panics
    ///
    /// Panics if any validation callback returns an error. This implements fail-fast
    /// behavior for configuration issues.
    pub fn register(self) {
        // Safety: we require the builder not to outlive &mut ServiceCollection.
        let sc = unsafe { &mut *self.sc };

        // Register Options<T> as singleton (factory)
        let default_maker = self.default_maker.clone();
        let configures = self.configures.clone();
        let post_configures = self.post_configures.clone();
        let validates = self.validates.clone();

        sc.add_singleton_factory::<Options<T>, _>(move |resolver| {
            // Build value
            let mut value: T = if let Some(mk) = &default_maker {
                (mk)()
            } else {
                T::default()
            };

            // Run configure steps
            for c in &configures {
                c(resolver, &mut value);
            }
            // Run post configure
            for pc in &post_configures {
                pc(resolver, &mut value);
            }
            // Validate
            for v in &validates {
                if let Err(msg) = v(&value) {
                    // Fail-fast (aligned with panic-for-misconfig stance)
                    panic!("Options<{}> validation failed: {}", std::any::type_name::<T>(), msg);
                }
            }

            Options::new(value)
        });

        // Note: We don't register T directly as a singleton because that would require
        // cloning the T value from the Arc<T> inside Options<T>. Instead, users should
        // resolve Options<T> and call .get() to get the Arc<T>, which is more efficient.
        // 
        // If direct T access is needed, it could be added with a Clone bound:
        // sc.add_singleton_factory::<T, _>(|resolver| {
        //     let opts = resolver.get_required::<Options<T>>();
        //     (*opts.get()).clone()  // requires T: Clone
        // });
    }
}

/// Extensions to ServiceCollection for the Options pattern.
impl ServiceCollection {
    /// Start building `Options<T>`. Call `.register()` to finalize.
    ///
    /// This method begins the configuration of strongly-typed options that will be
    /// available through dependency injection. The options follow an immutable
    /// snapshot model where configuration is resolved once during container setup.
    ///
    /// # Examples
    ///
    /// ```
    /// use ferrous_di::{ServiceCollection, IOptions, Options, Resolver};
    ///
    /// #[derive(Default)]
    /// struct MySettings {
    ///     enabled: bool,
    ///     timeout: u64,
    /// }
    ///
    /// let mut services = ServiceCollection::new();
    /// services.add_options::<MySettings>()
    ///     .configure(|_resolver, settings| {
    ///         settings.enabled = true;
    ///         settings.timeout = 5000;
    ///     })
    ///     .register();
    ///
    /// let provider = services.build();
    /// let options = provider.get_required::<Options<MySettings>>();
    /// let settings = options.get();
    /// assert!(settings.enabled);
    /// assert_eq!(settings.timeout, 5000);
    /// ```
    pub fn add_options<T>(&mut self) -> OptionsBuilder<T>
    where
        T: Default + Send + Sync + 'static,
    {
        OptionsBuilder::new(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    // Note: catch_unwind and AssertUnwindSafe are used in with_circular_catch function
    
    #[test]
    fn test_singleton_resolution() {
        let mut sc = ServiceCollection::new();
        sc.add_singleton(42usize);
        
        let sp = sc.build();
        let a = sp.get_required::<usize>();
        let b = sp.get_required::<usize>();
        
        assert_eq!(*a, 42);
        assert!(Arc::ptr_eq(&a, &b)); // Same instance
    }
    
    #[test]
    fn test_transient_resolution() {
        let mut sc = ServiceCollection::new();
        let counter = Arc::new(Mutex::new(0));
        let counter_clone = counter.clone();
        
        sc.add_transient_factory::<String, _>(move |_| {
            let mut c = counter_clone.lock().unwrap();
            *c += 1;
            format!("instance-{}", *c)
        });
        
        let sp = sc.build();
        let a = sp.get_required::<String>();
        let b = sp.get_required::<String>();
        
        assert_eq!(a.as_str(), "instance-1");
        assert_eq!(b.as_str(), "instance-2");
        assert!(!Arc::ptr_eq(&a, &b)); // Different instances
    }
    
    #[test]
    fn test_scoped_resolution() {
        let mut sc = ServiceCollection::new();
        let counter = Arc::new(Mutex::new(0));
        let counter_clone = counter.clone();
        
        sc.add_scoped_factory::<String, _>(move |_| {
            let mut c = counter_clone.lock().unwrap();
            *c += 1;
            format!("scoped-{}", *c)
        });
        
        let sp = sc.build();
        
        // Same scope should have same instance
        let scope1 = sp.create_scope();
        let s1a = scope1.get_required::<String>();
        let s1b = scope1.get_required::<String>();
        assert!(Arc::ptr_eq(&s1a, &s1b));
        
        // Different scope should have different instance
        let scope2 = sp.create_scope();
        let s2 = scope2.get_required::<String>();
        assert!(!Arc::ptr_eq(&s1a, &s2));
    }
    
    #[test]
    fn test_trait_resolution() {
        trait TestTrait: Send + Sync {
            fn get_value(&self) -> i32;
        }
        
        struct TestImpl {
            value: i32,
        }
        
        impl TestTrait for TestImpl {
            fn get_value(&self) -> i32 {
                self.value
            }
        }
        
        let mut sc = ServiceCollection::new();
        sc.add_singleton_trait::<dyn TestTrait>(Arc::new(TestImpl { value: 42 }));
        
        let sp = sc.build();
        let service = sp.get_required_trait::<dyn TestTrait>();
        assert_eq!(service.get_value(), 42);
    }

    #[test]
    fn test_options_pattern() {
        #[derive(Default)]
        struct TestConfig {
            value: i32,
        }

        let mut sc = ServiceCollection::new();
        sc.add_options::<TestConfig>()
            .configure(|_resolver, config| {
                config.value = 42;
            })
            .register();

        let sp = sc.build();
        let options = sp.get_required::<Options<TestConfig>>();
        let config = options.get();
        assert_eq!(config.value, 42);
    }
}