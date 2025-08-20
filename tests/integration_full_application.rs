/// Full application integration tests
/// 
/// These tests simulate real-world application scenarios using ferrous-di
/// to ensure the entire dependency injection system works end-to-end.

use ferrous_di::{ServiceCollection, ServiceModule, ServiceCollectionModuleExt, Resolver, DiResult};
use std::sync::{Arc, Mutex};

// ===== Application Domain Models =====

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub database_url: String,
    pub cache_size: usize,
    pub log_level: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            database_url: "postgres://localhost/testdb".to_string(),
            cache_size: 1000,
            log_level: "INFO".to_string(),
        }
    }
}

// ===== Logging Infrastructure =====

pub trait Logger: Send + Sync + std::fmt::Debug {
    fn log(&self, level: &str, message: &str);
    fn get_logs(&self) -> Vec<String>;
}

#[derive(Debug)]
pub struct InMemoryLogger {
    logs: Mutex<Vec<String>>,
}

impl InMemoryLogger {
    pub fn new() -> Self {
        Self {
            logs: Mutex::new(Vec::new()),
        }
    }
}

impl Logger for InMemoryLogger {
    fn log(&self, level: &str, message: &str) {
        let mut logs = self.logs.lock().unwrap();
        logs.push(format!("[{}] {}", level, message));
    }
    
    fn get_logs(&self) -> Vec<String> {
        self.logs.lock().unwrap().clone()
    }
}

// ===== Data Layer =====

pub trait Repository<T>: Send + Sync + std::fmt::Debug {
    fn find_by_id(&self, id: u64) -> Option<T>;
    fn save(&self, entity: T) -> u64;
    fn find_all(&self) -> Vec<T>;
}

#[derive(Debug, Clone, PartialEq)]
pub struct User {
    pub id: u64,
    pub name: String,
    pub email: String,
}

#[derive(Debug)]
pub struct InMemoryUserRepository {
    users: Mutex<Vec<User>>,
    next_id: Mutex<u64>,
    logger: Arc<dyn Logger>,
}

impl InMemoryUserRepository {
    pub fn new(logger: Arc<dyn Logger>) -> Self {
        logger.log("INFO", "Initializing user repository");
        Self {
            users: Mutex::new(Vec::new()),
            next_id: Mutex::new(1),
            logger,
        }
    }
}

impl Repository<User> for InMemoryUserRepository {
    fn find_by_id(&self, id: u64) -> Option<User> {
        let users = self.users.lock().unwrap();
        users.iter().find(|u| u.id == id).cloned()
    }
    
    fn save(&self, mut entity: User) -> u64 {
        let mut users = self.users.lock().unwrap();
        let mut next_id = self.next_id.lock().unwrap();
        
        if entity.id == 0 {
            entity.id = *next_id;
            *next_id += 1;
        }
        
        users.push(entity.clone());
        self.logger.log("INFO", &format!("Saved user: {}", entity.name));
        entity.id
    }
    
    fn find_all(&self) -> Vec<User> {
        self.users.lock().unwrap().clone()
    }
}

// ===== Cache Layer =====

pub trait Cache<K, V>: Send + Sync + std::fmt::Debug {
    fn get(&self, key: &K) -> Option<V>;
    fn set(&self, key: K, value: V);
    fn clear(&self);
}

#[derive(Debug)]
pub struct InMemoryCache<K: Clone, V: Clone> {
    data: Mutex<std::collections::HashMap<K, V>>,
    config: Arc<AppConfig>,
    logger: Arc<dyn Logger>,
}

impl<K: Clone, V: Clone> InMemoryCache<K, V> {
    pub fn new(config: Arc<AppConfig>, logger: Arc<dyn Logger>) -> Self {
        logger.log("INFO", &format!("Initializing cache with size: {}", config.cache_size));
        Self {
            data: Mutex::new(std::collections::HashMap::new()),
            config,
            logger,
        }
    }
}

impl Cache<u64, User> for InMemoryCache<u64, User> {
    fn get(&self, key: &u64) -> Option<User> {
        let data = self.data.lock().unwrap();
        let result = data.get(key).cloned();
        if result.is_some() {
            self.logger.log("DEBUG", &format!("Cache hit for user ID: {}", key));
        } else {
            self.logger.log("DEBUG", &format!("Cache miss for user ID: {}", key));
        }
        result
    }
    
    fn set(&self, key: u64, value: User) {
        let mut data = self.data.lock().unwrap();
        data.insert(key, value);
        self.logger.log("DEBUG", &format!("Cached user ID: {}", key));
    }
    
    fn clear(&self) {
        let mut data = self.data.lock().unwrap();
        data.clear();
        self.logger.log("INFO", "Cache cleared");
    }
}

// ===== Business Logic =====

#[derive(Debug)]
pub struct UserService {
    repository: Arc<Arc<dyn Repository<User>>>,
    cache: Arc<Arc<dyn Cache<u64, User>>>,
    logger: Arc<dyn Logger>,
}

impl UserService {
    pub fn new(
        repository: Arc<Arc<dyn Repository<User>>>,
        cache: Arc<Arc<dyn Cache<u64, User>>>,
        logger: Arc<dyn Logger>,
    ) -> Self {
        logger.log("INFO", "Initializing user service");
        Self {
            repository,
            cache,
            logger,
        }
    }
    
    pub fn get_user(&self, id: u64) -> Option<User> {
        // Try cache first
        if let Some(user) = self.cache.get(&id) {
            return Some(user);
        }
        
        // Fallback to repository
        if let Some(user) = self.repository.find_by_id(id) {
            self.cache.set(id, user.clone());
            Some(user)
        } else {
            self.logger.log("WARN", &format!("User not found: {}", id));
            None
        }
    }
    
    pub fn create_user(&self, name: String, email: String) -> u64 {
        let user = User { id: 0, name, email };
        let id = self.repository.save(user.clone());
        self.cache.set(id, User { id, ..user });
        self.logger.log("INFO", &format!("Created user: {}", id));
        id
    }
    
    pub fn list_users(&self) -> Vec<User> {
        self.repository.find_all()
    }
}

// ===== Application Controller =====

#[derive(Debug)]
pub struct UserController {
    user_service: Arc<UserService>,
    logger: Arc<dyn Logger>,
}

impl UserController {
    pub fn new(user_service: Arc<UserService>, logger: Arc<dyn Logger>) -> Self {
        logger.log("INFO", "Initializing user controller");
        Self {
            user_service,
            logger,
        }
    }
    
    pub fn handle_get_user(&self, id: u64) -> Result<User, String> {
        self.logger.log("INFO", &format!("Handling get user request: {}", id));
        
        match self.user_service.get_user(id) {
            Some(user) => Ok(user),
            None => Err(format!("User {} not found", id)),
        }
    }
    
    pub fn handle_create_user(&self, name: String, email: String) -> Result<u64, String> {
        self.logger.log("INFO", &format!("Handling create user request: {}", name));
        
        if name.is_empty() || email.is_empty() {
            return Err("Name and email are required".to_string());
        }
        
        let id = self.user_service.create_user(name, email);
        Ok(id)
    }
    
    pub fn handle_list_users(&self) -> Vec<User> {
        self.logger.log("INFO", "Handling list users request");
        self.user_service.list_users()
    }
}

// ===== Application Modules =====

pub struct LoggingModule;

impl ServiceModule for LoggingModule {
    fn register_services(self, services: &mut ServiceCollection) -> DiResult<()> {
        services.add_singleton_trait::<dyn Logger>(Arc::new(InMemoryLogger::new()));
        Ok(())
    }
}

pub struct DataModule;

impl ServiceModule for DataModule {
    fn register_services(self, services: &mut ServiceCollection) -> DiResult<()> {
        // Repository
        services.add_singleton_factory::<Arc<dyn Repository<User>>, _>(|r| {
            let logger = r.get_required_trait::<dyn Logger>();
            Arc::new(InMemoryUserRepository::new(logger)) as Arc<dyn Repository<User>>
        });
        
        // Cache
        services.add_singleton_factory::<Arc<dyn Cache<u64, User>>, _>(|r| {
            let config = r.get_required::<AppConfig>();
            let logger = r.get_required_trait::<dyn Logger>();
            Arc::new(InMemoryCache::new(config, logger)) as Arc<dyn Cache<u64, User>>
        });
        
        Ok(())
    }
}

pub struct BusinessModule;

impl ServiceModule for BusinessModule {
    fn register_services(self, services: &mut ServiceCollection) -> DiResult<()> {
        services.add_singleton_factory::<UserService, _>(|r| {
            let repository = r.get_required::<Arc<dyn Repository<User>>>();
            let cache = r.get_required::<Arc<dyn Cache<u64, User>>>();
            let logger = r.get_required_trait::<dyn Logger>();
            UserService::new(repository, cache, logger)
        });
        Ok(())
    }
}

pub struct ControllerModule;

impl ServiceModule for ControllerModule {
    fn register_services(self, services: &mut ServiceCollection) -> DiResult<()> {
        services.add_scoped_factory::<UserController, _>(|r| {
            let user_service = r.get_required::<UserService>();
            let logger = r.get_required_trait::<dyn Logger>();
            UserController::new(user_service, logger)
        });
        Ok(())
    }
}

// ===== Integration Tests =====

#[test]
fn test_full_application_flow() {
    // Arrange - Bootstrap the entire application
    let mut services = ServiceCollection::new();
    services.add_singleton(AppConfig::default());
    services.add_module_mut(LoggingModule).unwrap();
    services.add_module_mut(DataModule).unwrap();
    services.add_module_mut(BusinessModule).unwrap();
    services.add_module_mut(ControllerModule).unwrap();
    
    let provider = services.build();
    let scope = provider.create_scope();
    
    // Act & Assert - Simulate application requests
    let controller = scope.get_required::<UserController>();
    
    // Test user creation
    let user_id = controller.handle_create_user(
        "John Doe".to_string(),
        "john@example.com".to_string()
    ).unwrap();
    assert_eq!(user_id, 1);
    
    // Test user retrieval (should hit cache)
    let user = controller.handle_get_user(user_id).unwrap();
    assert_eq!(user.name, "John Doe");
    assert_eq!(user.email, "john@example.com");
    
    // Test user listing
    let users = controller.handle_list_users();
    assert_eq!(users.len(), 1);
    assert_eq!(users[0].name, "John Doe");
    
    // Test error cases
    let not_found = controller.handle_get_user(999);
    assert!(not_found.is_err());
    
    let validation_error = controller.handle_create_user("".to_string(), "test@example.com".to_string());
    assert!(validation_error.is_err());
}

#[test]
fn test_application_logging() {
    // Arrange
    let mut services = ServiceCollection::new();
    services.add_singleton(AppConfig::default());
    services.add_module_mut(LoggingModule).unwrap();
    services.add_module_mut(DataModule).unwrap();
    services.add_module_mut(BusinessModule).unwrap();
    services.add_module_mut(ControllerModule).unwrap();
    
    let provider = services.build();
    
    // Act - Perform some operations
    let scope = provider.create_scope();
    let controller = scope.get_required::<UserController>();
    let _user_id = controller.handle_create_user("Jane Doe".to_string(), "jane@example.com".to_string()).unwrap();
    let _user = controller.handle_get_user(1).unwrap();
    
    // Assert - Check that logging occurred
    let logger = provider.get_required_trait::<dyn Logger>();
    let logs = logger.get_logs();
    
    assert!(logs.iter().any(|log| log.contains("Initializing")));
    assert!(logs.iter().any(|log| log.contains("Created user")));
    assert!(logs.iter().any(|log| log.contains("Cache hit") || log.contains("Cache miss")));
}

#[test]
fn test_multiple_scopes_independence() {
    // Arrange
    let mut services = ServiceCollection::new();
    services.add_singleton(AppConfig::default());
    services.add_module_mut(LoggingModule).unwrap();
    services.add_module_mut(DataModule).unwrap();
    services.add_module_mut(BusinessModule).unwrap();
    services.add_module_mut(ControllerModule).unwrap();
    
    let provider = services.build();
    
    // Act - Create multiple scopes (simulating different requests)
    let scope1 = provider.create_scope();
    let scope2 = provider.create_scope();
    
    let controller1 = scope1.get_required::<UserController>();
    let controller2 = scope2.get_required::<UserController>();
    
    // Assert - Controllers are different instances (scoped)
    assert!(!Arc::ptr_eq(&controller1, &controller2));
    
    // But they share the same singleton services
    let user_service1 = &controller1.user_service;
    let user_service2 = &controller2.user_service;
    assert!(Arc::ptr_eq(user_service1, user_service2));
}

#[test]
fn test_configuration_propagation() {
    // Arrange - Custom configuration
    let custom_config = AppConfig {
        database_url: "postgres://custom/db".to_string(),
        cache_size: 5000,
        log_level: "DEBUG".to_string(),
    };
    
    let mut services = ServiceCollection::new();
    services.add_singleton(custom_config);
    services.add_module_mut(LoggingModule).unwrap();
    services.add_module_mut(DataModule).unwrap();
    
    let provider = services.build();
    
    // Act - Get configured services
    let cache = provider.get_required::<Arc<dyn Cache<u64, User>>>();
    
    // Assert - Configuration was properly injected
    // We can't directly test cache_size, but we can verify it was created
    // This test mainly ensures the dependency injection graph was built correctly
    // Just verify we got a valid service by using it
    let test_user = User { id: 1, name: "Test".to_string(), email: "test@test.com".to_string() };
    cache.set(1, test_user.clone());
    let retrieved = cache.get(&1);
    assert_eq!(retrieved, Some(test_user));
}

#[test]
fn test_cache_behavior_integration() {
    // Arrange
    let mut services = ServiceCollection::new();
    services.add_singleton(AppConfig::default());
    services.add_module_mut(LoggingModule).unwrap();
    services.add_module_mut(DataModule).unwrap();
    services.add_module_mut(BusinessModule).unwrap();
    
    let provider = services.build();
    let user_service = provider.get_required::<UserService>();
    
    // Act - Create a user and access it multiple times
    let user_id = user_service.create_user("Cache Test".to_string(), "cache@test.com".to_string());
    
    // First access - should populate cache
    let user1 = user_service.get_user(user_id);
    
    // Second access - should hit cache
    let user2 = user_service.get_user(user_id);
    
    // Assert
    assert_eq!(user1, user2);
    assert!(user1.is_some());
    assert_eq!(user1.as_ref().unwrap().name, "Cache Test");
}