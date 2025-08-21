//! Multi-tenant Axum application example using ferrous-di
//!
//! This example demonstrates:
//! - Per-tenant scoped dependency injection
//! - Tenant-specific database connections
//! - Isolated service instances per tenant
//! - Clean separation of concerns

use axum::{
    extract::Path,
    response::Json,
    routing::{get, post},
};
use ferrous_di::{
    axum_integration::{create_app_with_di, DiScope},
    ServiceCollection, Resolver,
};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use tokio::net::TcpListener;

// Domain models
#[derive(Clone, Serialize, Deserialize)]
struct Product {
    #[serde(default)]
    id: u32,
    name: String,
    price: f64,
    #[serde(default)]
    tenant_id: String,
}

// Tenant-specific configuration
#[derive(Clone)]
struct TenantConfig {
    tenant_id: String,
    #[allow(dead_code)] // Will be used in real multi-tenant implementation
    db_url: String,
    #[allow(dead_code)] // Will be used in real multi-tenant implementation
    api_key: String,
    features: TenantFeatures,
}

#[derive(Clone)]
struct TenantFeatures {
    max_products: usize,
    premium_features: bool,
}

// Repository trait for data access
trait ProductRepository: Send + Sync {
    fn get_all(&self) -> Vec<Product>;
    fn get_by_id(&self, id: u32) -> Option<Product>;
    fn create(&self, product: Product) -> Product;
}

// Mock implementation of ProductRepository with state
struct MockProductRepository {
    tenant_id: String,
    products: Arc<Mutex<Vec<Product>>>,
}

impl MockProductRepository {
    fn new(tenant_id: String) -> Self {
        let initial_products = vec![
            Product {
                id: 1,
                name: format!("Product A ({})", tenant_id),
                price: 99.99,
                tenant_id: tenant_id.clone(),
            },
            Product {
                id: 2,
                name: format!("Product B ({})", tenant_id),
                price: 149.99,
                tenant_id: tenant_id.clone(),
            },
        ];
        
        Self {
            tenant_id,
            products: Arc::new(Mutex::new(initial_products)),
        }
    }
}

impl ProductRepository for MockProductRepository {
    fn get_all(&self) -> Vec<Product> {
        self.products.lock().unwrap().clone()
    }
    
    fn get_by_id(&self, id: u32) -> Option<Product> {
        self.products.lock().unwrap().iter().find(|p| p.id == id).cloned()
    }
    
    fn create(&self, mut product: Product) -> Product {
        let mut products = self.products.lock().unwrap();
        
        // Auto-assign tenant_id if not provided
        if product.tenant_id.is_empty() {
            product.tenant_id = self.tenant_id.clone();
        }
        
        // Auto-assign ID if not provided
        if product.id == 0 {
            let max_id = products.iter().map(|p| p.id).max().unwrap_or(0);
            product.id = max_id + 1;
        }
        
        // Add to our in-memory store
        products.push(product.clone());
        
        product
    }
}

// Business service that depends on repository
struct ProductService {
    repository: Arc<dyn ProductRepository>,
    config: Arc<TenantConfig>,
}

impl ProductService {
    fn new(repository: Arc<dyn ProductRepository>, config: Arc<TenantConfig>) -> Self {
        Self { repository, config }
    }
    
    fn list_products(&self) -> Vec<Product> {
        let products = self.repository.get_all();
        
        // Apply tenant-specific business rules
        if self.config.features.premium_features {
            products
        } else {
            // Free tier only sees limited products
            products.into_iter().take(self.config.features.max_products).collect()
        }
    }
    
    fn get_product(&self, id: u32) -> Option<Product> {
        self.repository.get_by_id(id)
    }
    
    fn create_product(&self, product: Product) -> Result<Product, String> {
        // Check tenant limits
        let current_count = self.repository.get_all().len();
        if current_count >= self.config.features.max_products {
            return Err(format!(
                "Tenant {} has reached product limit of {}",
                self.config.tenant_id, self.config.features.max_products
            ));
        }
        
        Ok(self.repository.create(product))
    }
}

// API Handlers

async fn list_products(scope: DiScope) -> Json<Vec<Product>> {
    let service = scope.get_required::<ProductService>();
    Json(service.list_products())
}

async fn get_product(
    Path(id): Path<u32>,
    scope: DiScope,
) -> Result<Json<Product>, String> {
    let service = scope.get_required::<ProductService>();
    service
        .get_product(id)
        .map(Json)
        .ok_or_else(|| "Product not found".to_string())
}

async fn create_product(
    scope: DiScope,
    Json(product): Json<Product>,
) -> Result<Json<Product>, String> {
    let service = scope.get_required::<ProductService>();
    service.create_product(product).map(Json)
}

// Multi-tenant handlers would go here
// In a real app, you'd extract tenant from JWT/headers and configure
// the scope accordingly

// Application setup

fn configure_services() -> ServiceCollection {
    let mut services = ServiceCollection::new();
    
    // Register default/shared services
    services.add_singleton(Arc::new("v1.0.0".to_string()) as Arc<String>);
    
    // For demo: register tenant-specific services
    // In real app, these would be registered per-request based on tenant
    services.add_scoped_factory::<TenantConfig, _>(|_| {
        // This would normally come from tenant extraction
        TenantConfig {
            tenant_id: "default".to_string(),
            db_url: "postgres://localhost/default".to_string(),
            api_key: "default-key".to_string(),
            features: TenantFeatures {
                max_products: 10,
                premium_features: false,
            },
        }
    });
    
    // Register the trait implementation as a singleton to maintain state across requests
    services.add_singleton_trait(Arc::new(MockProductRepository::new("default".to_string())) as Arc<dyn ProductRepository>);
    
    services.add_scoped_factory::<ProductService, _>(|resolver| {
        let repository = resolver.get_required_trait::<dyn ProductRepository>();
        let config = resolver.get_required::<TenantConfig>();
        ProductService::new(repository, config)
    });
    
    services
}

#[tokio::main]
async fn main() {
    println!("🚀 Starting Multi-tenant Axum Server with ferrous-di");
    
    // Build DI container
    let services = configure_services();
    let provider = Arc::new(services.build());
    
    // Build Axum app with DI integration
    let app = create_app_with_di(provider, |router| {
        router
            // Regular endpoints (using default tenant)
            .route("/products", get(list_products))
            .route("/products/:id", get(get_product))
            .route("/products", post(create_product))
            
            // Health check
            .route("/health", get(|| async { "OK" }))
    });
    
    // Start server
    let listener = TcpListener::bind("127.0.0.1:3000")
        .await
        .expect("Failed to bind address");
    
    println!("📡 Server running on http://127.0.0.1:3000");
    println!("Try:");
    println!("  curl http://127.0.0.1:3000/products");
    println!("  curl http://127.0.0.1:3000/products/1");
    println!("  curl -X POST http://127.0.0.1:3000/products \\");
    println!("    -H 'Content-Type: application/json' \\");
    println!("    -d '{{\"id\": 0, \"name\": \"Product C\", \"price\": 199.99, \"tenant_id\": \"\"}}'");
    println!("  # Note: id and tenant_id are auto-assigned if not provided");
    
    // Now using the corrected Axum 0.7 Extension-based DI pattern
    println!("✅ Using Axum 0.7 Extension-based DI with into_make_service()");
    
    axum::serve(listener, app.into_make_service())
        .await
        .expect("Server failed to start");
}