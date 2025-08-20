//! Service module system for modular registration.
//!
//! This module provides traits and functionality for organizing service
//! registrations into reusable modules.

use crate::{ServiceCollection, DiResult};

/// A module that can register services with a ServiceCollection.
/// 
/// This trait enables modular service registration, similar to .NET's extension methods.
/// Each module can implement this trait to provide its own service registrations.
/// 
/// # Example
/// 
/// ```rust
/// use ferrous_di::{ServiceCollection, ServiceModule, ServiceCollectionExt, DiResult, Resolver};
/// 
/// #[derive(Default)]
/// struct UserConfig;
/// 
/// struct UserService;
/// impl UserService {
///     fn new(_config: std::sync::Arc<UserConfig>) -> Self { Self }
/// }
/// 
/// struct UserModule;
/// 
/// impl ServiceModule for UserModule {
///     fn register_services(self, services: &mut ServiceCollection) -> DiResult<()> {
///         services.add_singleton(UserConfig::default());
///         services.add_scoped_factory::<UserService, _>(|r| {
///             let config = r.get_required::<UserConfig>();
///             UserService::new(config)
///         });
///         Ok(())
///     }
/// }
/// 
/// # fn main() -> DiResult<()> {
/// // Usage
/// let mut services = ServiceCollection::new();
/// let provider = services.add_module(UserModule)?.build();
/// # Ok(())
/// # }
/// ```
pub trait ServiceModule {
    /// Register this module's services with the ServiceCollection.
    fn register_services(self, services: &mut ServiceCollection) -> DiResult<()>;
}

/// Extension trait for ServiceCollection that provides module registration capabilities.
/// 
/// This trait enables .NET-style extension method chaining for modules.
pub trait ServiceCollectionExt {
    /// Add a module to the service collection using extension method syntax.
    /// 
    /// # Example
    /// 
    /// ```rust
    /// use ferrous_di::{ServiceCollection, ServiceCollectionExt, ServiceModule, DiResult};
    /// 
    /// struct DatabaseModule;
    /// impl ServiceModule for DatabaseModule {
    ///     fn register_services(self, _: &mut ServiceCollection) -> DiResult<()> { Ok(()) }
    /// }
    /// 
    /// struct UserModule;  
    /// impl ServiceModule for UserModule {
    ///     fn register_services(self, _: &mut ServiceCollection) -> DiResult<()> { Ok(()) }
    /// }
    /// 
    /// # fn main() -> DiResult<()> {
    /// let mut services = ServiceCollection::new();
    /// let provider = services
    ///     .add_module(DatabaseModule)?
    ///     .add_module(UserModule)?
    ///     .build();
    /// # Ok(())
    /// # }
    /// ```
    fn add_module<M: ServiceModule>(self, module: M) -> DiResult<Self>
    where 
        Self: Sized;
}

impl ServiceCollectionExt for ServiceCollection {
    fn add_module<M: ServiceModule>(mut self, module: M) -> DiResult<Self> {
        module.register_services(&mut self)?;
        Ok(self)
    }
}

/// Additional extension trait for ServiceCollection that provides fluent module registration
/// that matches the existing &mut Self pattern.
pub trait ServiceCollectionModuleExt {
    /// Add a module to the service collection in-place.
    /// Returns a DiResult to handle any registration errors.
    fn add_module_mut<M: ServiceModule>(&mut self, module: M) -> DiResult<&mut Self>;
}

impl ServiceCollectionModuleExt for ServiceCollection {
    fn add_module_mut<M: ServiceModule>(&mut self, module: M) -> DiResult<&mut Self> {
        module.register_services(self)?;
        Ok(self)
    }
}