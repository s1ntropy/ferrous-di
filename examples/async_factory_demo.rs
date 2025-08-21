//! Async Factory Demo - Showcasing ferrous-di's async service initialization capabilities
//!
//! This example demonstrates:
//! - Async service factories for database connections, network handshakes, etc.
//! - Proper error handling in async factories
//! - Integration with the service collection
//! - Runtime context detection and management
//! - Smart runtime handling (avoids blocking within async contexts)

use ferrous_di::*;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

/// Simulated database connection that requires async initialization
#[derive(Clone)]
struct DatabaseConnection {
    connection_string: String,
    is_connected: bool,
    connection_time: std::time::Instant,
}

impl DatabaseConnection {
    async fn connect(connection_string: String) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        // Simulate async connection setup (handshake, auth, etc.)
        println!("🔌 Connecting to database: {}", connection_string);
        sleep(Duration::from_millis(100)).await;
        
        // Simulate potential connection failure
        if connection_string.contains("invalid") {
            return Err("Invalid connection string".into());
        }
        
        println!("✅ Database connected successfully");
        Ok(Self {
            connection_string,
            is_connected: true,
            connection_time: std::time::Instant::now(),
        })
    }
    
    fn execute_query(&self, query: &str) -> String {
        if !self.is_connected {
            return "ERROR: Not connected".to_string();
        }
        format!("Query '{}' executed on {}", query, self.connection_string)
    }
    
    fn connection_age(&self) -> Duration {
        self.connection_time.elapsed()
    }
}

/// Async factory for database connections
struct DatabaseConnectionFactory {
    connection_string: String,
}

#[async_trait::async_trait]
impl AsyncFactory<DatabaseConnection> for DatabaseConnectionFactory {
    async fn create(&self, _resolver: &dyn ResolverCore) -> Result<Arc<DatabaseConnection>, Box<dyn std::error::Error + Send + Sync>> {
        DatabaseConnection::connect(self.connection_string.clone()).await.map(Arc::new)
    }
}

/// Simulated HTTP client that requires async initialization
#[derive(Clone)]
struct HttpClient {
    base_url: String,
    timeout: Duration,
    is_initialized: bool,
}

impl HttpClient {
    async fn initialize(base_url: String, timeout: Duration) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        println!("🌐 Initializing HTTP client for: {}", base_url);
        
        // Simulate async initialization (DNS lookup, connection pool setup, etc.)
        sleep(Duration::from_millis(50)).await;
        
        // Simulate potential initialization failure
        if base_url.contains("unreachable") {
            return Err("Host unreachable".into());
        }
        
        println!("✅ HTTP client initialized successfully");
        Ok(Self {
            base_url,
            timeout,
            is_initialized: true,
        })
    }
    
    fn make_request(&self, endpoint: &str) -> String {
        if !self.is_initialized {
            return "ERROR: Client not initialized".to_string();
        }
        format!("GET {}{} (timeout: {:?})", self.base_url, endpoint, self.timeout)
    }
}

/// Async factory for HTTP clients
struct HttpClientFactory {
    base_url: String,
    timeout: Duration,
}

#[async_trait::async_trait]
impl AsyncFactory<HttpClient> for HttpClientFactory {
    async fn create(&self, _resolver: &dyn ResolverCore) -> Result<Arc<HttpClient>, Box<dyn std::error::Error + Send + Sync>> {
        HttpClient::initialize(self.base_url.clone(), self.timeout).await.map(Arc::new)
    }
}

/// Service that depends on both database and HTTP client
#[derive(Clone)]
struct DataService {
    db: Arc<DatabaseConnection>,
    http: Arc<HttpClient>,
}

impl DataService {
    fn new(db: Arc<DatabaseConnection>, http: Arc<HttpClient>) -> Self {
        Self { db, http }
    }
    
    fn process_data(&self, data: &str) -> String {
        let query_result = self.db.execute_query(&format!("SELECT * FROM data WHERE value = '{}'", data));
        let api_result = self.http.make_request("/api/process");
        
        format!("Data processed: {} | DB: {} | API: {}", data, query_result, api_result)
    }
}

/// Async factory for DataService that resolves dependencies
struct DataServiceFactory;

#[async_trait::async_trait]
impl AsyncFactory<DataService> for DataServiceFactory {
    async fn create(&self, _resolver: &dyn ResolverCore) -> Result<Arc<DataService>, Box<dyn std::error::Error + Send + Sync>> {
        // In a real implementation, this would resolve dependencies from the resolver
        // For this demo, we'll create mock dependencies
        let db = Arc::new(DatabaseConnection {
            connection_string: "mock://localhost:5432/mydb".to_string(),
            is_connected: true,
            connection_time: std::time::Instant::now(),
        });
        
        let http = Arc::new(HttpClient {
            base_url: "https://mock.example.com".to_string(),
            timeout: Duration::from_secs(30),
            is_initialized: true,
        });
        
        Ok(Arc::new(DataService::new(db, http)))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 Async Factory Demo - ferrous-di");
    println!("=====================================\n");
    
    // Build service collection with async factories
    let mut services = ServiceCollection::new();
    
    // Register async services
    services
        .add_singleton_async::<DatabaseConnection, _>(
            DatabaseConnectionFactory {
                connection_string: "postgres://localhost:5432/mydb".to_string(),
            }
        )
        .add_singleton_async::<HttpClient, _>(
            HttpClientFactory {
                base_url: "https://api.example.com".to_string(),
                timeout: Duration::from_secs(30),
            }
        )
        .add_singleton_async::<DataService, _>(DataServiceFactory);
    
    println!("📦 Building service provider...");
    let provider = services.build();
    println!("✅ Service provider built successfully\n");
    
    // Resolve and use services
    println!("🔍 Resolving services...");
    
    let db = provider.get_required::<Arc<DatabaseConnection>>();
    println!("📊 Database connection age: {:?}", db.connection_age());
    
    let http = provider.get_required::<Arc<HttpClient>>();
    println!("🌐 HTTP client ready: {}", http.base_url);
    
    let data_service = provider.get_required::<Arc<DataService>>();
    println!("🔄 Processing data...");
    
    let result = data_service.process_data("sample_data");
    println!("📋 Result: {}\n", result);
    
    // Demonstrate error handling with invalid connection
    println!("🧪 Testing error handling...");
    let mut error_services = ServiceCollection::new();
    error_services.add_singleton_async::<DatabaseConnection, _>(
        DatabaseConnectionFactory {
            connection_string: "postgres://invalid:5432/mydb".to_string(),
        }
    );
    
    let error_provider = error_services.build();
    match error_provider.get::<DatabaseConnection>() {
        Ok(_) => println!("❌ Unexpected success"),
        Err(e) => println!("✅ Expected error: {}", e),
    }
    
    println!("\n🎉 Async Factory Demo completed successfully!");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_database_connection_factory() {
        let factory = DatabaseConnectionFactory {
            connection_string: "postgres://test:5432/testdb".to_string(),
        };
        
        // Create a mock resolver
        struct MockResolver;
        impl ResolverCore for MockResolver {
            fn resolve_any(&self, _key: &Key) -> DiResult<Arc<dyn std::any::Any + Send + Sync>> {
                Err(DiError::NotFound("Mock"))
            }
            
            fn resolve_many(&self, _key: &Key) -> DiResult<Vec<Arc<dyn std::any::Any + Send + Sync>>> {
                Ok(vec![])
            }
            
            fn push_sync_disposer(&self, _f: Box<dyn FnOnce() + Send>) {}
            
            fn push_async_disposer(&self, _f: Box<dyn FnOnce() -> crate::internal::BoxFutureUnit + Send>) {}
        }
        
        let resolver = MockResolver;
        let connection = factory.create(&resolver).await.unwrap();
        
        assert!(connection.is_connected);
        assert_eq!(connection.connection_string, "postgres://test:5432/testdb");
    }
    
    #[tokio::test]
    async fn test_http_client_factory() {
        let factory = HttpClientFactory {
            base_url: "https://test.example.com".to_string(),
            timeout: Duration::from_secs(10),
        };
        
        struct MockResolver;
        impl ResolverCore for MockResolver {
            fn resolve_any(&self, _key: &Key) -> DiResult<Arc<dyn std::any::Any + Send + Sync>> {
                Err(DiError::NotFound("Mock"))
            }
            
            fn resolve_many(&self, _key: &Key) -> DiResult<Vec<Arc<dyn std::any::Any + Send + Sync>>> {
                Ok(vec![])
            }
            
            fn push_sync_disposer(&self, _f: Box<dyn FnOnce() + Send>) {}
            
            fn push_async_disposer(&self, _f: Box<dyn FnOnce() -> crate::internal::BoxFutureUnit + Send>) {}
        }
        
        let resolver = MockResolver;
        let client = factory.create(&resolver).await.unwrap();
        
        assert!(client.is_initialized);
        assert_eq!(client.base_url, "https://test.example.com");
        assert_eq!(client.timeout, Duration::from_secs(10));
    }
}
