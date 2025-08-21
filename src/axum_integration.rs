//! Axum web framework integration for ferrous-di.
//!
//! This module provides seamless integration with Axum, enabling:
//! - Per-request scoped dependency injection
//! - Multi-tenant support through scoped containers
//! - Automatic service lifecycle management
//! - Clean extraction of DI-managed services in handlers

use crate::{DiResult, Scope, ServiceProvider};
use crate::traits::Resolver;
use axum::{
    async_trait,
    extract::{FromRef, FromRequestParts},
    http::{request::Parts, StatusCode},
    response::{IntoResponse, Response},
    Router,
};
use std::sync::Arc;

/// Extension trait for ServiceProvider to create Axum-compatible state
pub trait AxumServiceProvider {
    /// Wraps the ServiceProvider for use as Axum application state
    fn into_axum_state(self: Arc<Self>) -> DiAxumState;
}

impl AxumServiceProvider for ServiceProvider {
    fn into_axum_state(self: Arc<Self>) -> DiAxumState {
        DiAxumState {
            provider: self,
        }
    }
}

/// Axum application state wrapper for the DI container
#[derive(Clone)]
pub struct DiAxumState {
    provider: Arc<ServiceProvider>,
}

impl std::fmt::Debug for DiAxumState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DiAxumState")
            .field("provider", &"ServiceProvider")
            .finish()
    }
}

// Ensure DiAxumState implements all required traits for Axum 0.7
unsafe impl Send for DiAxumState {}
unsafe impl Sync for DiAxumState {}

// Additional trait implementations that might be required
impl std::fmt::Display for DiAxumState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DiAxumState")
    }
}

impl std::panic::RefUnwindSafe for DiAxumState {}
impl std::panic::UnwindSafe for DiAxumState {}

impl FromRef<DiAxumState> for Arc<ServiceProvider> {
    fn from_ref(state: &DiAxumState) -> Self {
        state.provider.clone()
    }
}


/// Extractor for request-scoped DI container
///
/// This creates a new scope for each request, enabling:
/// - Scoped service lifetime management
/// - Per-request service instances
/// - Automatic cleanup after request completion
pub struct DiScope {
    scope: Scope,
}

impl DiScope {
    /// Get a required service from the scoped container
    pub fn get_required<T: Send + Sync + 'static>(&self) -> Arc<T> {
        self.scope.get_required()
    }
    
    /// Try to get an optional service from the scoped container
    pub fn get<T: Send + Sync + 'static>(&self) -> DiResult<Arc<T>> {
        self.scope.get()
    }
    
    /// Get the underlying scope for advanced usage
    pub fn scope(&self) -> &Scope {
        &self.scope
    }
}

#[async_trait]
impl<S> FromRequestParts<S> for DiScope 
where
    S: Send + Sync,
{
    type Rejection = DiRejection;

    async fn from_request_parts(
        parts: &mut Parts,
        _state: &S,
    ) -> Result<Self, Self::Rejection> {
        // Extract the ServiceProvider from the Extension layer
        let provider = parts
            .extensions
            .get::<Arc<ServiceProvider>>()
            .ok_or_else(|| DiRejection::Configuration(
                "ServiceProvider not found in extensions. Make sure to use create_app_with_di()".to_string()
            ))?;
            
        Ok(DiScope {
            scope: provider.create_scope(),
        })
    }
}

/// Tenant-aware scope extractor for multi-tenant applications
///
/// This extractor creates a scope and configures it based on tenant information
/// extracted from the request (e.g., from JWT claims, headers, or path).
pub struct TenantScope {
    scope: Scope,
    tenant_id: String,
}

impl TenantScope {
    /// Get the tenant ID for this request
    pub fn tenant_id(&self) -> &str {
        &self.tenant_id
    }
    
    /// Get a required service from the tenant-scoped container
    pub fn get_required<T: Send + Sync + 'static>(&self) -> Arc<T> {
        self.scope.get_required()
    }
    
    /// Try to get an optional service from the tenant-scoped container
    pub fn get<T: Send + Sync + 'static>(&self) -> DiResult<Arc<T>> {
        self.scope.get()
    }
    
    /// Get the underlying scope for advanced usage
    pub fn scope(&self) -> &Scope {
        &self.scope
    }
}

/// Trait for extracting tenant information from requests
///
/// Implement this to define how tenant IDs are extracted from incoming requests.
#[async_trait]
pub trait TenantExtractor: Send + Sync {
    /// Extract tenant ID from request parts
    async fn extract_tenant(&self, parts: &Parts) -> Result<String, TenantExtractionError>;
    
    /// Configure the scope for the extracted tenant
    ///
    /// This is called after scope creation to allow tenant-specific configuration
    /// injection, such as database connection strings, API endpoints, etc.
    fn configure_scope(&self, scope: &Scope, tenant_id: &str) -> DiResult<()>;
}

/// Error type for tenant extraction failures
#[derive(Debug)]
pub struct TenantExtractionError {
    pub message: String,
}

impl IntoResponse for TenantExtractionError {
    fn into_response(self) -> Response {
        (StatusCode::BAD_REQUEST, self.message).into_response()
    }
}

/// Example JWT-based tenant extractor
///
/// This demonstrates extracting tenant ID from JWT claims in the Authorization header.
pub struct JwtTenantExtractor {
    // In real implementation, this would include JWT validation configuration
}

#[async_trait]
impl TenantExtractor for JwtTenantExtractor {
    async fn extract_tenant(&self, parts: &Parts) -> Result<String, TenantExtractionError> {
        // Extract Authorization header
        let auth_header = parts
            .headers
            .get("authorization")
            .and_then(|h| h.to_str().ok())
            .ok_or_else(|| TenantExtractionError {
                message: "Missing authorization header".to_string(),
            })?;
        
        // Parse Bearer token
        let token = auth_header
            .strip_prefix("Bearer ")
            .ok_or_else(|| TenantExtractionError {
                message: "Invalid authorization format".to_string(),
            })?;
        
        // In real implementation: validate JWT and extract claims
        // For demo purposes, we'll just use a simple extraction
        if token.starts_with("tenant-") {
            Ok(token.strip_prefix("tenant-").unwrap().to_string())
        } else {
            Err(TenantExtractionError {
                message: "Invalid tenant token".to_string(),
            })
        }
    }
    
    fn configure_scope(&self, scope: &Scope, tenant_id: &str) -> DiResult<()> {
        // In real implementation, inject tenant-specific configuration
        // For example:
        // - Database connection string for tenant
        // - API endpoints for tenant
        // - Feature flags for tenant
        // - Rate limits for tenant
        
        // Example: Register tenant config as a scoped service
        #[derive(Clone)]
        struct TenantConfig {
            tenant_id: String,
            db_url: String,
            api_endpoint: String,
        }
        
        let config = TenantConfig {
            tenant_id: tenant_id.to_string(),
            db_url: format!("postgres://localhost/tenant_{}", tenant_id),
            api_endpoint: format!("https://api.example.com/tenant/{}", tenant_id),
        };
        
        // This would need a way to register services directly into a scope
        // For now, this is a conceptual example
        let _ = (scope, config);
        
        Ok(())
    }
}

/// State wrapper that includes tenant extractor
pub struct DiAxumStateWithTenants<E: TenantExtractor> {
    provider: Arc<ServiceProvider>,
    tenant_extractor: Arc<E>,
}

impl<E: TenantExtractor> Clone for DiAxumStateWithTenants<E> {
    fn clone(&self) -> Self {
        Self {
            provider: self.provider.clone(),
            tenant_extractor: self.tenant_extractor.clone(),
        }
    }
}

#[async_trait]
impl<E> FromRequestParts<DiAxumStateWithTenants<E>> for TenantScope
where
    E: TenantExtractor + 'static,
{
    type Rejection = DiRejection;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &DiAxumStateWithTenants<E>,
    ) -> Result<Self, Self::Rejection> {
        // Extract tenant ID
        let tenant_id = state
            .tenant_extractor
            .extract_tenant(parts)
            .await
            .map_err(|e| DiRejection::TenantExtraction(e))?;
        
        // Create scope for this request
        let scope = state.provider.create_scope();
        
        // Configure scope for tenant
        state
            .tenant_extractor
            .configure_scope(&scope, &tenant_id)
            .map_err(|e| DiRejection::Configuration(e.to_string()))?;
        
        Ok(TenantScope { scope, tenant_id })
    }
}

/// Rejection type for DI extraction failures
#[derive(Debug)]
pub enum DiRejection {
    TenantExtraction(TenantExtractionError),
    Configuration(String),
}

impl IntoResponse for DiRejection {
    fn into_response(self) -> Response {
        match self {
            DiRejection::TenantExtraction(e) => e.into_response(),
            DiRejection::Configuration(msg) => {
                (StatusCode::INTERNAL_SERVER_ERROR, msg).into_response()
            }
        }
    }
}

/// Helper function to create an Axum app with DI support (Axum 0.7 compatible)
///
/// This is the idiomatic way to use ferrous-di with Axum 0.7:
/// ```rust
/// let provider = Arc::new(services.build());
/// let app = ferrous_di::axum_integration::create_app_with_di(provider, |router| {
///     router
///         .route("/users", get(list_users))
///         .route("/users/:id", get(get_user))
/// });
/// ```
pub fn create_app_with_di<F>(
    provider: Arc<ServiceProvider>,
    configure: F,
) -> Router
where
    F: FnOnce(Router) -> Router,
{
    // In Axum 0.7, we need to use a different approach
    // Store the provider in a way that can be accessed by extractors
    let router = Router::new();
    
    // Configure the router first, then add the extension
    let router = configure(router);
    
    // Add the provider as an extension layer so extractors can access it
    router.layer(axum::Extension(provider))
}

/// Helper function to create an Axum app with tenant-aware DI support
pub fn create_app_with_tenant_di<E, F>(
    provider: Arc<ServiceProvider>,
    tenant_extractor: E,
    configure: F,
) -> Router<DiAxumStateWithTenants<E>>
where
    E: TenantExtractor + 'static,
    F: FnOnce(Router<DiAxumStateWithTenants<E>>) -> Router<DiAxumStateWithTenants<E>>,
{
    let state = DiAxumStateWithTenants {
        provider,
        tenant_extractor: Arc::new(tenant_extractor),
    };
    let router = Router::new().with_state(state);
    configure(router)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ServiceCollection;
    
    #[tokio::test]
    async fn test_basic_di_scope_extraction() {
        // Setup DI container
        let mut services = ServiceCollection::new();
        
        #[derive(Clone)]
        struct TestService {
            value: String,
        }
        
        services.add_singleton(TestService {
            value: "test".to_string(),
        });
        
        let provider = Arc::new(services.build());
        let state = provider.into_axum_state();
        
        // Simulate extraction
        let scope = DiScope {
            scope: state.provider.create_scope(),
        };
        
        // Verify service resolution
        let service = scope.get_required::<TestService>();
        assert_eq!(service.value, "test");
    }
    
    #[tokio::test]
    async fn test_tenant_extraction() {
        let extractor = JwtTenantExtractor {};
        
        // Create mock request parts with tenant token
        let mut parts = Parts::default();
        parts.headers.insert(
            "authorization",
            "Bearer tenant-acme-corp".parse().unwrap(),
        );
        
        let tenant_id = extractor.extract_tenant(&parts).await.unwrap();
        assert_eq!(tenant_id, "acme-corp");
    }
}