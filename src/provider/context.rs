//! Resolver context for dependency injection.
//!
//! This module contains the ResolverContext type which provides
//! the interface for factory functions to resolve dependencies.

use crate::traits::{Resolver, ResolverCore};

/// Context passed to factory functions for resolving dependencies.
///
/// ResolverContext wraps a resolver (ServiceProvider or Scope) and provides
/// the interface that factory functions use to access other services. This
/// allows factory functions to be independent of the specific resolver type.
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
/// let mut services = ServiceCollection::new();
/// services.add_singleton(Database { 
///     url: "postgres://localhost".to_string() 
/// });
/// services.add_transient_factory::<UserService, _>(|resolver| {
///     // resolver is a ResolverContext that provides access to other services
///     UserService {
///         db: resolver.get_required::<Database>(),
///     }
/// });
/// ```
pub struct ResolverContext<'a> {
    resolver: &'a dyn ResolverCore,
}

impl<'a> ResolverContext<'a> {
    /// Creates a new ResolverContext wrapping the given resolver.
    pub(crate) fn new<T>(resolver: &'a T) -> Self 
    where 
        T: ResolverCore,
    {
        Self { resolver }
    }
}

impl<'a> ResolverCore for ResolverContext<'a> {
    fn resolve_any(&self, key: &crate::Key) -> crate::DiResult<crate::registration::AnyArc> {
        self.resolver.resolve_any(key)
    }
    
    fn resolve_many(&self, key: &crate::Key) -> crate::DiResult<Vec<crate::registration::AnyArc>> {
        self.resolver.resolve_many(key)
    }

    fn push_sync_disposer(&self, f: Box<dyn FnOnce() + Send>) {
        self.resolver.push_sync_disposer(f);
    }

    fn push_async_disposer(&self, f: Box<dyn FnOnce() -> crate::internal::BoxFutureUnit + Send>) {
        self.resolver.push_async_disposer(f);
    }
}

impl<'a> Resolver for ResolverContext<'a> {
    fn register_disposer<T>(&self, service: std::sync::Arc<T>)
    where
        T: crate::traits::Dispose + 'static,
    {
        self.resolver.push_sync_disposer(Box::new(move || service.dispose()));
    }

    fn register_async_disposer<T>(&self, service: std::sync::Arc<T>)
    where
        T: crate::traits::AsyncDispose + 'static,
    {
        self.resolver.push_async_disposer(Box::new(move || {
            let service = service.clone();
            Box::pin(async move { service.dispose().await })
        }));
    }
}