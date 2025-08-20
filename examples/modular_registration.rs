/// Example: Modular service registration using the Module pattern
/// 
/// This example demonstrates how to organize services into modules
/// similar to .NET's extension methods for dependency injection.

use ferrous_di::{ServiceCollection, ServiceModule, ServiceCollectionExt, ServiceCollectionModuleExt, Resolver, DiResult};
use std::sync::Arc;

// ===== Shared Configuration =====

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub database_url: String,
    pub api_key: String,
    pub max_connections: usize,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            database_url: "postgresql://localhost:5432/app".to_string(),
            api_key: "dev-api-key".to_string(),
            max_connections: 10,
        }
    }
}

// ===== Database Module =====

#[derive(Debug)]
pub struct Database {
    pub connection_string: String,
    pub max_connections: usize,
}

impl Database {
    pub fn new(config: Arc<AppConfig>) -> Self {
        Self {
            connection_string: config.database_url.clone(),
            max_connections: config.max_connections,
        }
    }
    
    pub fn connect(&self) -> String {
        format!("Connected to {}", self.connection_string)
    }
}

pub struct DatabaseModule;

impl ServiceModule for DatabaseModule {
    fn register_services(self, services: &mut ServiceCollection) -> DiResult<()> {
        // Register database as singleton
        services.add_singleton_factory::<Database, _>(|r| {
            let config = r.get_required::<AppConfig>();
            Database::new(config)
        });
        Ok(())
    }
}

// Alternative: Extension trait approach (like your original idea)
pub trait DatabaseModuleExt {
    fn add_database_services(self) -> Self;
}

impl DatabaseModuleExt for ServiceCollection {
    fn add_database_services(mut self) -> Self {
        self.add_singleton_factory::<Database, _>(|r| {
            let config = r.get_required::<AppConfig>();
            Database::new(config)
        });
        self
    }
}

// ===== User Module =====

#[derive(Debug)]
pub struct UserRepository {
    pub database: Arc<Database>,
}

impl UserRepository {
    pub fn new(database: Arc<Database>) -> Self {
        Self { database }
    }
    
    pub fn find_user(&self, id: u32) -> String {
        format!("User {} from {}", id, self.database.connect())
    }
}

#[derive(Debug)]  
pub struct UserService {
    pub repository: Arc<UserRepository>,
}

impl UserService {
    pub fn new(repository: Arc<UserRepository>) -> Self {
        Self { repository }
    }
    
    pub fn get_user_profile(&self, id: u32) -> String {
        format!("Profile: {}", self.repository.find_user(id))
    }
}

pub struct UserModule;

impl ServiceModule for UserModule {
    fn register_services(self, services: &mut ServiceCollection) -> DiResult<()> {
        // Repository as scoped (per-request)
        services.add_scoped_factory::<UserRepository, _>(|r| {
            let database = r.get_required::<Database>();
            UserRepository::new(database)
        });
        
        // Service as transient (new instance every time)
        services.add_transient_factory::<UserService, _>(|r| {
            let repository = r.get_required::<UserRepository>();
            UserService::new(repository)
        });
        
        Ok(())
    }
}

// Extension trait version
pub trait UserModuleExt {
    fn add_user_services(self) -> Self;
}

impl UserModuleExt for ServiceCollection {
    fn add_user_services(mut self) -> Self {
        self.add_scoped_factory::<UserRepository, _>(|r| {
            let database = r.get_required::<Database>();
            UserRepository::new(database)
        });
        self.add_transient_factory::<UserService, _>(|r| {
            let repository = r.get_required::<UserRepository>();
            UserService::new(repository)
        });
        self
    }
}

// ===== API Module =====

#[derive(Debug)]
pub struct ApiClient {
    pub api_key: String,
}

impl ApiClient {
    pub fn new(config: Arc<AppConfig>) -> Self {
        Self {
            api_key: config.api_key.clone(),
        }
    }
    
    pub fn call_api(&self) -> String {
        format!("API call with key: {}", self.api_key)
    }
}

pub struct ApiModule;

impl ServiceModule for ApiModule {
    fn register_services(self, services: &mut ServiceCollection) -> DiResult<()> {
        services.add_singleton_factory::<ApiClient, _>(|r| {
            let config = r.get_required::<AppConfig>();
            ApiClient::new(config)
        });
        Ok(())
    }
}

// Extension trait version
pub trait ApiModuleExt {
    fn add_api_services(self) -> Self;
}

impl ApiModuleExt for ServiceCollection {
    fn add_api_services(mut self) -> Self {
        self.add_singleton_factory::<ApiClient, _>(|r| {
            let config = r.get_required::<AppConfig>();
            ApiClient::new(config)
        });
        self
    }
}

// ===== Application Service (uses everything) =====

#[derive(Debug)]
pub struct AppService {
    pub user_service: Arc<UserService>,
    pub api_client: Arc<ApiClient>,
}

impl AppService {
    pub fn new(user_service: Arc<UserService>, api_client: Arc<ApiClient>) -> Self {
        Self {
            user_service,
            api_client,
        }
    }
    
    pub fn process_request(&self, user_id: u32) -> String {
        let user_profile = self.user_service.get_user_profile(user_id);
        let api_result = self.api_client.call_api();
        format!("{} | {}", user_profile, api_result)
    }
}

pub struct AppModule;

impl ServiceModule for AppModule {
    fn register_services(self, services: &mut ServiceCollection) -> DiResult<()> {
        services.add_scoped_factory::<AppService, _>(|r| {
            let user_service = r.get_required::<UserService>();
            let api_client = r.get_required::<ApiClient>();
            AppService::new(user_service, api_client)
        });
        Ok(())
    }
}

// ===== Main Function =====

fn main() -> DiResult<()> {
    println!("=== Ferrous DI Modular Registration Example ===\n");
    
    // Method 1: Using ServiceModule trait with &mut Self pattern  
    println!("1. Building container with ServiceModule trait (mut):");
    let mut services = ServiceCollection::new();
    services.add_singleton(AppConfig::default());
    services.add_module_mut(DatabaseModule)?;
    services.add_module_mut(UserModule)?;
    services.add_module_mut(ApiModule)?;
    services.add_module_mut(AppModule)?;
    let provider = services.build();
    
    // Use the services
    let scope = provider.create_scope();
    let app_service = scope.get_required::<AppService>();
    println!("   Result: {}\n", app_service.process_request(123));
    
    // Method 1b: Using ServiceModule trait with owned Self pattern
    println!("1b. Building container with ServiceModule trait (owned):");
    let mut services1b = ServiceCollection::new();
    services1b.add_singleton(AppConfig {
        database_url: "sqlite:///tmp/app.db".to_string(),
        api_key: "owned-api-key".to_string(),
        max_connections: 5,
    });
    let provider1b = services1b
        .add_module(DatabaseModule)?
        .add_module(UserModule)?
        .add_module(ApiModule)?
        .add_module(AppModule)?
        .build();
    
    let scope1b = provider1b.create_scope();
    let app_service1b = scope1b.get_required::<AppService>();
    println!("   Result: {}\n", app_service1b.process_request(111));
    
    // Method 2: Using extension traits (original idea)
    println!("2. Building container with extension traits:");
    
    // Import the extension traits
    use DatabaseModuleExt;
    use UserModuleExt; 
    use ApiModuleExt;
    
    // Note: Extension traits work well when chaining from empty collection
    let mut services2 = ServiceCollection::new();
    services2 = services2
        .add_database_services()  // Extension method!
        .add_user_services()      // Extension method!
        .add_api_services();      // Extension method!
    
    // Then add configuration and app service
    services2.add_singleton(AppConfig {
        database_url: "mysql://localhost:3306/app".to_string(),
        api_key: "prod-api-key".to_string(),
        max_connections: 20,
    });
    services2.add_scoped_factory::<AppService, _>(|r| {
        let user_service = r.get_required::<UserService>();
        let api_client = r.get_required::<ApiClient>();
        AppService::new(user_service, api_client)
    });
    
    let provider2 = services2.build();
    let scope2 = provider2.create_scope();
    let app_service2 = scope2.get_required::<AppService>();
    println!("   Result: {}\n", app_service2.process_request(456));
    
    // Method 3: Mixed approach - modules can be combined
    println!("3. Mixed approach (trait + extension):");
    let mut services3 = ServiceCollection::new();
    services3.add_singleton(AppConfig::default());
    services3.add_module_mut(DatabaseModule)?;  // ServiceModule (mut)
    services3 = services3.add_user_services();  // Extension trait (moving)
    services3.add_module_mut(ApiModule)?;       // ServiceModule (mut) 
    services3.add_module_mut(AppModule)?;       // ServiceModule (mut)
    let provider3 = services3.build();
    
    let scope3 = provider3.create_scope();
    let app_service3 = scope3.get_required::<AppService>();
    println!("   Result: {}\n", app_service3.process_request(789));
    
    println!("=== Summary ===");
    println!("✅ ServiceModule trait: Clean, testable, composable");
    println!("✅ Extension traits: Fluent, .NET-like, discoverable");  
    println!("✅ Both approaches chain naturally and work together");
    println!("✅ Modules encapsulate related service registrations");
    
    Ok(())
}