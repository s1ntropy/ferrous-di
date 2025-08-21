//! Minimal Axum integration test to identify core issues

use axum::{response::Json, routing::get, Router};
use ferrous_di::{
    axum_integration::{create_app_with_di, DiScope},
    ServiceCollection, Resolver,
};
use serde::Serialize;
use std::sync::Arc;
use tokio::net::TcpListener;

#[derive(Clone, Serialize)]
struct TestService {
    message: String,
}

#[derive(Clone, Serialize)]
struct TestResponse {
    service_message: String,
    app_version: String,
}

// Simple handler that uses DI
async fn test_handler(scope: DiScope) -> Json<TestResponse> {
    let service = scope.get_required::<TestService>();
    let version = scope.get_required::<String>();
    
    Json(TestResponse {
        service_message: service.message.clone(),
        app_version: version.as_str().to_string(), // Dereference Arc<String>
    })
}

#[tokio::main]
async fn main() {
    println!("🔍 Testing minimal Axum + ferrous-di integration");
    
    // Build DI container
    let mut services = ServiceCollection::new();
    
    // Register simple services
    services.add_singleton(TestService {
        message: "Hello from DI!".to_string(),
    });
    
    services.add_singleton("v1.0.0".to_string());
    
    let provider = Arc::new(services.build());
    
    // Build Axum app
    let app = create_app_with_di(provider, |router| {
        router.route("/test", get(test_handler))
    });
    
    // Start server
    let listener = TcpListener::bind("127.0.0.1:3001")
        .await
        .expect("Failed to bind address");
    
    println!("🚀 Minimal test server running on http://127.0.0.1:3001");
    println!("Try: curl http://127.0.0.1:3001/test");
    
    // Now using the correct Axum 0.7 pattern with Extension-based DI
    println!("Router type: {}", std::any::type_name::<Router>());
    println!("✅ Using Axum 0.7 Extension-based DI pattern");
    
    // In Axum 0.7, Router<()> has into_make_service() method
    println!("✅ Using into_make_service() method (now available with unit state)");
    
    // Try serving with the standard Axum 0.7 pattern
    println!("🚀 Starting server...");
    axum::serve(listener, app.into_make_service())
        .await
        .expect("Server failed to start");
}