//! Web framework integrations for ferrous-di.
//!
//! This module provides integration with popular Rust web frameworks
//! including Actix-Web, Axum, Rocket, and Warp.

use std::sync::Arc;

#[cfg(feature = "async")]
use crate::{ServiceProvider, Scope};

/// Extension trait for Axum integration
#[cfg(feature = "axum-integration")]
pub mod axum {
    use super::*;
    use axum::{
        extract::{FromRequestParts, State},
        http::request::Parts,
        response::Response,
    };
    use std::future::Future;
    use std::pin::Pin;

    /// Axum state wrapper for the async service provider
    #[derive(Clone)]
    pub struct DiState {
        provider: Arc<AsyncServiceProvider>,
    }

    impl DiState {
        /// Create a new DI state from a provider
        pub fn new(provider: Arc<AsyncServiceProvider>) -> Self {
            Self { provider }
        }

        /// Get the service provider
        pub fn provider(&self) -> Arc<AsyncServiceProvider> {
            self.provider.clone()
        }
    }

    /// Axum extractor for async scopes
    pub struct DiScope {
        scope: AsyncScope,
    }

    impl DiScope {
        /// Get the inner scope
        pub fn inner(&self) -> &AsyncScope {
            &self.scope
        }
    }

    #[async_trait::async_trait]
    impl<S> FromRequestParts<S> for DiScope
    where
        S: Send + Sync,
        DiState: FromRef<S>,
    {
        type Rejection = std::convert::Infallible;

        async fn from_request_parts(
            _parts: &mut Parts,
            state: &S,
        ) -> Result<Self, Self::Rejection> {
            let di_state = DiState::from_ref(state);
            let scope = di_state.provider.create_scope().await;
            Ok(DiScope { scope })
        }
    }

    /// Helper trait for converting state references
    pub trait FromRef<T> {
        fn from_ref(input: &T) -> Self;
    }

    impl FromRef<DiState> for DiState {
        fn from_ref(input: &DiState) -> Self {
            input.clone()
        }
    }

    /// Axum layer for dependency injection
    pub struct DiLayer {
        provider: Arc<AsyncServiceProvider>,
    }

    impl DiLayer {
        /// Create a new DI layer
        pub fn new(provider: Arc<AsyncServiceProvider>) -> Self {
            Self { provider }
        }
    }

    impl<S> tower::Layer<S> for DiLayer {
        type Service = DiService<S>;

        fn layer(&self, inner: S) -> Self::Service {
            DiService {
                inner,
                provider: self.provider.clone(),
            }
        }
    }

    /// Axum service wrapper for dependency injection
    #[derive(Clone)]
    pub struct DiService<S> {
        inner: S,
        provider: Arc<AsyncServiceProvider>,
    }

    impl<S, B> tower::Service<axum::http::Request<B>> for DiService<S>
    where
        S: tower::Service<axum::http::Request<B>, Response = Response> + Clone + Send + 'static,
        S::Future: Send + 'static,
        B: Send + 'static,
    {
        type Response = S::Response;
        type Error = S::Error;
        type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

        fn poll_ready(
            &mut self,
            cx: &mut std::task::Context<'_>,
        ) -> std::task::Poll<Result<(), Self::Error>> {
            self.inner.poll_ready(cx)
        }

        fn call(&mut self, req: axum::http::Request<B>) -> Self::Future {
            let mut inner = self.inner.clone();
            Box::pin(async move { inner.call(req).await })
        }
    }
}

/// Extension trait for Actix-Web integration
#[cfg(feature = "actix-integration")]
pub mod actix {
    use super::*;
    use actix_web::{
        dev::{Service, ServiceRequest, ServiceResponse, Transform},
        web::Data,
        Error, HttpMessage,
    };
    use std::future::{Future, Ready, ready};
    use std::pin::Pin;
    use std::rc::Rc;

    /// Actix-Web data wrapper for the async service provider
    pub struct DiData {
        provider: Arc<AsyncServiceProvider>,
    }

    impl DiData {
        /// Create new DI data from a provider
        pub fn new(provider: Arc<AsyncServiceProvider>) -> Data<Self> {
            Data::new(Self { provider })
        }

        /// Get the service provider
        pub fn provider(&self) -> Arc<AsyncServiceProvider> {
            self.provider.clone()
        }
    }

    /// Actix-Web middleware for dependency injection
    pub struct DiMiddleware {
        provider: Arc<AsyncServiceProvider>,
    }

    impl DiMiddleware {
        /// Create new DI middleware
        pub fn new(provider: Arc<AsyncServiceProvider>) -> Self {
            Self { provider }
        }
    }

    impl<S, B> Transform<S, ServiceRequest> for DiMiddleware
    where
        S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
        S::Future: 'static,
        B: 'static,
    {
        type Response = ServiceResponse<B>;
        type Error = Error;
        type Transform = DiMiddlewareService<S>;
        type InitError = ();
        type Future = Ready<Result<Self::Transform, Self::InitError>>;

        fn new_transform(&self, service: S) -> Self::Future {
            ready(Ok(DiMiddlewareService {
                service: Rc::new(service),
                provider: self.provider.clone(),
            }))
        }
    }

    /// Actix-Web middleware service for dependency injection
    pub struct DiMiddlewareService<S> {
        service: Rc<S>,
        provider: Arc<AsyncServiceProvider>,
    }

    impl<S, B> Service<ServiceRequest> for DiMiddlewareService<S>
    where
        S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
        S::Future: 'static,
        B: 'static,
    {
        type Response = ServiceResponse<B>;
        type Error = Error;
        type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>>>>;

        fn poll_ready(
            &self,
            cx: &mut std::task::Context<'_>,
        ) -> std::task::Poll<Result<(), Self::Error>> {
            self.service.poll_ready(cx)
        }

        fn call(&self, req: ServiceRequest) -> Self::Future {
            let service = self.service.clone();
            let provider = self.provider.clone();

            Box::pin(async move {
                // Create a scope for this request
                let scope = provider.create_scope().await;
                req.extensions_mut().insert(scope);
                
                service.call(req).await
            })
        }
    }

    /// Extension trait for extracting DI scope from request
    pub trait DiScopeExt {
        /// Get the DI scope for this request
        fn di_scope(&self) -> Option<AsyncScope>;
    }

    impl DiScopeExt for ServiceRequest {
        fn di_scope(&self) -> Option<AsyncScope> {
            self.extensions().get::<AsyncScope>().cloned()
        }
    }

    impl DiScopeExt for actix_web::HttpRequest {
        fn di_scope(&self) -> Option<AsyncScope> {
            self.extensions().get::<AsyncScope>().cloned()
        }
    }
}

/// Simplified integration helpers for common patterns
pub mod helpers {
    use super::*;
    
    /// Create a request-scoped service factory
    pub fn create_request_scope_factory<T, F>(factory: F) -> impl Fn() -> T
    where
        T: Send + Sync + 'static,
        F: Fn() -> T + Send + Sync + 'static,
    {
        move || factory()
    }

    /// Middleware for injecting services into HTTP headers
    pub struct ServiceInjectionMiddleware {
        service_name: String,
        header_name: String,
    }

    impl ServiceInjectionMiddleware {
        /// Create new service injection middleware
        pub fn new(service_name: impl Into<String>, header_name: impl Into<String>) -> Self {
            Self {
                service_name: service_name.into(),
                header_name: header_name.into(),
            }
        }
    }
}

/// Common web service patterns
pub mod patterns {
    use super::*;
    use std::collections::HashMap;
    use std::sync::Arc;
    use tokio::sync::RwLock;

    /// Request context service for storing request-specific data
    #[derive(Clone)]
    pub struct RequestContext {
        data: Arc<RwLock<HashMap<String, String>>>,
        request_id: String,
        correlation_id: Option<String>,
    }

    impl RequestContext {
        /// Create a new request context
        pub fn new(request_id: String) -> Self {
            Self {
                data: Arc::new(RwLock::new(HashMap::new())),
                request_id,
                correlation_id: None,
            }
        }

        /// Set correlation ID
        pub fn set_correlation_id(&mut self, id: String) {
            self.correlation_id = Some(id);
        }

        /// Get request ID
        pub fn request_id(&self) -> &str {
            &self.request_id
        }

        /// Get correlation ID
        pub fn correlation_id(&self) -> Option<&str> {
            self.correlation_id.as_deref()
        }

        /// Set context data
        pub async fn set_data(&self, key: String, value: String) {
            let mut data = self.data.write().await;
            data.insert(key, value);
        }

        /// Get context data
        pub async fn get_data(&self, key: &str) -> Option<String> {
            let data = self.data.read().await;
            data.get(key).cloned()
        }
    }

    /// User context service for authentication/authorization
    #[derive(Clone)]
    pub struct UserContext {
        user_id: Option<String>,
        roles: Vec<String>,
        permissions: Vec<String>,
        claims: HashMap<String, String>,
    }

    impl UserContext {
        /// Create a new user context
        pub fn new() -> Self {
            Self {
                user_id: None,
                roles: Vec::new(),
                permissions: Vec::new(),
                claims: HashMap::new(),
            }
        }

        /// Set authenticated user
        pub fn set_user(&mut self, user_id: String) {
            self.user_id = Some(user_id);
        }

        /// Check if user is authenticated
        pub fn is_authenticated(&self) -> bool {
            self.user_id.is_some()
        }

        /// Add role
        pub fn add_role(&mut self, role: String) {
            if !self.roles.contains(&role) {
                self.roles.push(role);
            }
        }

        /// Check if user has role
        pub fn has_role(&self, role: &str) -> bool {
            self.roles.iter().any(|r| r == role)
        }

        /// Add permission
        pub fn add_permission(&mut self, permission: String) {
            if !self.permissions.contains(&permission) {
                self.permissions.push(permission);
            }
        }

        /// Check if user has permission
        pub fn has_permission(&self, permission: &str) -> bool {
            self.permissions.iter().any(|p| p == permission)
        }

        /// Set claim
        pub fn set_claim(&mut self, key: String, value: String) {
            self.claims.insert(key, value);
        }

        /// Get claim
        pub fn get_claim(&self, key: &str) -> Option<&str> {
            self.claims.get(key).map(|s| s.as_str())
        }
    }

    impl Default for UserContext {
        fn default() -> Self {
            Self::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::patterns::*;

    #[tokio::test]
    async fn test_request_context() {
        let mut context = RequestContext::new("req-123".to_string());
        context.set_correlation_id("corr-456".to_string());
        
        assert_eq!(context.request_id(), "req-123");
        assert_eq!(context.correlation_id(), Some("corr-456"));
        
        context.set_data("key1".to_string(), "value1".to_string()).await;
        assert_eq!(context.get_data("key1").await, Some("value1".to_string()));
    }

    #[test]
    fn test_user_context() {
        let mut context = UserContext::new();
        
        assert!(!context.is_authenticated());
        
        context.set_user("user123".to_string());
        assert!(context.is_authenticated());
        
        context.add_role("admin".to_string());
        context.add_permission("read:users".to_string());
        
        assert!(context.has_role("admin"));
        assert!(!context.has_role("user"));
        assert!(context.has_permission("read:users"));
        assert!(!context.has_permission("write:users"));
        
        context.set_claim("email".to_string(), "user@example.com".to_string());
        assert_eq!(context.get_claim("email"), Some("user@example.com"));
    }
}