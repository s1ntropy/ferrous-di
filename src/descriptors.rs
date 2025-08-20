//! Service descriptors for introspection and diagnostics.

use std::any::TypeId;
use crate::key::Key;
use crate::lifetime::Lifetime;

/// Service descriptor for introspection and diagnostics
///
/// Contains metadata about registered services that can be used for
/// debugging, validation, and runtime introspection of the dependency
/// injection container configuration.
///
/// # Use Cases
///
/// - **Debugging**: Inspect what services are registered and their lifetimes
/// - **Validation**: Ensure all required services are registered
/// - **Documentation**: Generate service dependency graphs
/// - **Health checks**: Verify container configuration at startup
///
/// # Examples
///
/// ```rust
/// use ferrous_di::{ServiceCollection, ServiceDescriptor, Lifetime};
/// use std::sync::Arc;
///
/// struct Database { url: String }
/// struct Repository { name: String }
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
/// let mut services = ServiceCollection::new();
/// services.add_singleton(Database { url: "postgres://localhost".to_string() });
/// services.add_scoped_factory::<Repository, _>(|_| Repository { name: "UserRepo".to_string() });
/// services.add_singleton_trait(Arc::new(ConsoleLogger) as Arc<dyn Logger>);
/// services.add_named_singleton("config_value", 42u32);
///
/// // Get service descriptors for inspection
/// let descriptors = services.get_service_descriptors();
///
/// // Find specific services
/// let db_descriptor = descriptors.iter()
///     .find(|d| d.type_name().contains("Database"))
///     .unwrap();
/// assert_eq!(db_descriptor.lifetime, Lifetime::Singleton);
/// assert!(!db_descriptor.is_named());
///
/// let config_descriptor = descriptors.iter()
///     .find(|d| d.is_named() && d.service_name() == Some("config_value"))
///     .unwrap();
/// assert_eq!(config_descriptor.type_name(), "u32");
/// assert_eq!(config_descriptor.service_name(), Some("config_value"));
///
/// // Count services by lifetime
/// let singleton_count = descriptors.iter()
///     .filter(|d| d.lifetime == Lifetime::Singleton)
///     .count();
/// let scoped_count = descriptors.iter()
///     .filter(|d| d.lifetime == Lifetime::Scoped)
///     .count();
///
/// println!("Registered {} singletons, {} scoped services", singleton_count, scoped_count);
/// ```
#[derive(Debug, Clone)]
pub struct ServiceDescriptor {
    /// The service key (type/trait name with optional service name)
    pub key: Key,
    /// Service lifetime
    pub lifetime: Lifetime,
    /// Implementation type ID (if available)
    pub impl_type_id: Option<TypeId>,
    /// Implementation type name (if available)
    pub impl_type_name: Option<&'static str>,
    /// Whether this registration has metadata attached
    pub has_metadata: bool,
}

impl ServiceDescriptor {
    /// Get the service name for named services, or None for unnamed services
    ///
    /// Returns the service name for descriptors representing named service
    /// registrations, or `None` for unnamed services.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use ferrous_di::{ServiceCollection, Lifetime};
    ///
    /// let mut services = ServiceCollection::new();
    /// services.add_singleton(42u32);
    /// services.add_named_singleton("database_port", 5432u32);
    ///
    /// let descriptors = services.get_service_descriptors();
    ///
    /// let unnamed = descriptors.iter().find(|d| !d.is_named()).unwrap();
    /// assert_eq!(unnamed.service_name(), None);
    ///
    /// let named = descriptors.iter().find(|d| d.is_named()).unwrap();
    /// assert_eq!(named.service_name(), Some("database_port"));
    /// ```
    pub fn service_name(&self) -> Option<&'static str> {
        self.key.service_name()
    }
    
    /// Get the type/trait name
    ///
    /// Returns the human-readable type or trait name for this service.
    /// This is typically the result of `std::any::type_name`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use ferrous_di::{ServiceCollection, Lifetime};
    /// use std::sync::Arc;
    ///
    /// trait Logger: Send + Sync {}
    /// struct ConsoleLogger;
    /// impl Logger for ConsoleLogger {}
    ///
    /// let mut services = ServiceCollection::new();
    /// services.add_singleton("config".to_string());
    /// services.add_singleton_trait(Arc::new(ConsoleLogger) as Arc<dyn Logger>);
    ///
    /// let descriptors = services.get_service_descriptors();
    ///
    /// let string_descriptor = descriptors.iter()
    ///     .find(|d| d.type_name().contains("String"))
    ///     .unwrap();
    /// assert!(string_descriptor.type_name().contains("String"));
    ///
    /// let logger_descriptor = descriptors.iter()
    ///     .find(|d| d.type_name().contains("Logger"))
    ///     .unwrap();
    /// assert!(logger_descriptor.type_name().contains("Logger"));
    /// ```
    pub fn type_name(&self) -> &'static str {
        self.key.display_name()
    }
    
    /// Check if this is a named service
    ///
    /// Returns `true` if this descriptor represents a named service
    /// registration, `false` for unnamed services.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use ferrous_di::{ServiceCollection, Lifetime};
    ///
    /// let mut services = ServiceCollection::new();
    /// services.add_singleton(42u32);
    /// services.add_named_singleton("max_connections", 100u32);
    ///
    /// let descriptors = services.get_service_descriptors();
    ///
    /// let unnamed = descriptors.iter().find(|d| !d.is_named()).unwrap();
    /// assert!(!unnamed.is_named());
    /// assert_eq!(unnamed.service_name(), None);
    ///
    /// let named = descriptors.iter().find(|d| d.is_named()).unwrap();
    /// assert!(named.is_named());
    /// assert_eq!(named.service_name(), Some("max_connections"));
    /// ```
    pub fn is_named(&self) -> bool {
        self.service_name().is_some()
    }
}