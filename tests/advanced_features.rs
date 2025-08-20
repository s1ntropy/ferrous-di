use ferrous_di::{ServiceCollection, Resolver, Lifetime, Key};
use std::sync::{Arc, Mutex};
use std::any::TypeId;

// ===== Service Descriptor and Metadata Tests =====

#[derive(Debug, PartialEq)]
struct ServiceMetadata {
    description: String,
    version: String,
    priority: i32,
}

#[test]
fn test_service_descriptors_basic() {
    let mut services = ServiceCollection::new();
    
    // Add various service types
    services.add_singleton(42usize);
    services.add_scoped_factory::<String, _>(|_| "scoped".to_string());
    services.add_transient_factory::<i32, _>(|_| 100);
    
    let descriptors = services.get_service_descriptors();
    assert_eq!(descriptors.len(), 3);
    
    // Find the usize singleton
    let usize_desc = descriptors.iter()
        .find(|d| d.type_name().contains("usize"))
        .expect("Should find usize service");
    
    assert_eq!(usize_desc.lifetime, Lifetime::Singleton);
    assert_eq!(usize_desc.impl_type_id, Some(TypeId::of::<usize>()));
    assert!(!usize_desc.has_metadata);
    assert!(!usize_desc.is_named());
    assert_eq!(usize_desc.service_name(), None);
    
    // Find the String scoped service
    let string_desc = descriptors.iter()
        .find(|d| d.type_name().contains("String"))
        .expect("Should find String service");
    
    assert_eq!(string_desc.lifetime, Lifetime::Scoped);
    assert_eq!(string_desc.impl_type_id, Some(TypeId::of::<String>()));
}

#[test]
fn test_service_metadata() {
    let mut services = ServiceCollection::new();
    
    let metadata = ServiceMetadata {
        description: "Critical service".to_string(),
        version: "2.1.0".to_string(),
        priority: 10,
    };
    
    // Register service with metadata
    services.add_with_metadata(42usize, Lifetime::Singleton, metadata);
    
    // Check service descriptor reflects metadata
    let descriptors = services.get_service_descriptors();
    let desc = descriptors.iter()
        .find(|d| d.type_name().contains("usize"))
        .expect("Should find usize service");
    
    assert!(desc.has_metadata);
    
    // Retrieve metadata
    let key = Key::Type(TypeId::of::<usize>(), "usize");
    let retrieved_metadata = services.get_metadata::<ServiceMetadata>(&key)
        .expect("Should have metadata");
    
    assert_eq!(retrieved_metadata.description, "Critical service");
    assert_eq!(retrieved_metadata.version, "2.1.0");
    assert_eq!(retrieved_metadata.priority, 10);
    
    // Wrong type should return None
    assert!(services.get_metadata::<String>(&key).is_none());
}

#[test]
fn test_service_descriptors_with_traits() {
    trait TestService: Send + Sync {
        fn name(&self) -> &str;
    }
    
    struct ServiceImpl {
        name: String,
    }
    
    impl TestService for ServiceImpl {
        fn name(&self) -> &str {
            &self.name
        }
    }
    
    let mut services = ServiceCollection::new();
    
    // Add single trait binding
    services.add_singleton_trait(Arc::new(ServiceImpl { 
        name: "impl1".to_string() 
    }) as Arc<dyn TestService>);
    
    // Add multi-trait bindings
    services.add_trait_implementation(Arc::new(ServiceImpl { 
        name: "impl2".to_string() 
    }) as Arc<dyn TestService>, Lifetime::Singleton);
    
    services.add_trait_implementation(Arc::new(ServiceImpl { 
        name: "impl3".to_string() 
    }) as Arc<dyn TestService>, Lifetime::Scoped);
    
    let descriptors = services.get_service_descriptors();
    
    // Should have 1 single-binding + 2 multi-binding = 3 total
    assert_eq!(descriptors.len(), 3);
    
    // Find single trait binding
    let single_trait = descriptors.iter()
        .find(|d| matches!(d.key, Key::Trait(_)))
        .expect("Should find single trait binding");
    
    assert_eq!(single_trait.lifetime, Lifetime::Singleton);
    assert!(!single_trait.has_metadata);
    
    // Find multi-trait bindings
    let multi_traits: Vec<_> = descriptors.iter()
        .filter(|d| matches!(d.key, Key::MultiTrait(_, _)))
        .collect();
    
    assert_eq!(multi_traits.len(), 2);
    assert!(multi_traits.iter().any(|d| d.lifetime == Lifetime::Singleton));
    assert!(multi_traits.iter().any(|d| d.lifetime == Lifetime::Scoped));
}

// ===== Conditional Registration (TryAdd*) Tests =====

#[test]
fn test_try_add_singleton_conditional() {
    let mut services = ServiceCollection::new();
    
    // First registration should succeed
    let added1 = services.try_add_singleton(42usize);
    assert!(added1);
    
    // Second registration should fail (already exists)
    let added2 = services.try_add_singleton(100usize);
    assert!(!added2);
    
    // Verify original value is preserved
    let provider = services.build();
    let value = provider.get_required::<usize>();
    assert_eq!(*value, 42);
}

#[test]
fn test_try_add_factory_conditional() {
    let counter = Arc::new(Mutex::new(0));
    let counter_clone = counter.clone();
    
    let mut services = ServiceCollection::new();
    
    // First factory registration should succeed
    let added1 = services.try_add_singleton_factory::<String, _>(move |_| {
        let mut c = counter_clone.lock().unwrap();
        *c += 1;
        format!("factory1-{}", *c)
    });
    assert!(added1);
    
    // Second factory registration should fail
    let added2 = services.try_add_singleton_factory::<String, _>(|_| "factory2".to_string());
    assert!(!added2);
    
    // Verify first factory is used
    let provider = services.build();
    let value = provider.get_required::<String>();
    assert_eq!(*value, "factory1-1");
    
    // Second call to same factory should reuse singleton
    let value2 = provider.get_required::<String>();
    assert_eq!(*value2, "factory1-1"); // Same instance, counter not incremented
}

#[test]
fn test_try_add_different_lifetimes() {
    let mut services = ServiceCollection::new();
    
    // Different lifetime methods for same type should be independent
    let added_singleton = services.try_add_singleton_factory::<i32, _>(|_| 42);
    assert!(added_singleton);
    
    let added_scoped = services.try_add_scoped_factory::<i32, _>(|_| 100);
    assert!(!added_scoped); // Should fail, singleton already registered
    
    let added_transient = services.try_add_transient_factory::<i32, _>(|_| 200);
    assert!(!added_transient); // Should fail, singleton already registered
}

#[test]
fn test_try_add_trait_conditional() {
    trait TestTrait: Send + Sync {
        fn value(&self) -> i32;
    }
    
    struct Impl1;
    impl TestTrait for Impl1 {
        fn value(&self) -> i32 { 1 }
    }
    
    struct Impl2;
    impl TestTrait for Impl2 {
        fn value(&self) -> i32 { 2 }
    }
    
    let mut services = ServiceCollection::new();
    
    // First trait registration should succeed
    let added1 = services.try_add_singleton_trait(Arc::new(Impl1) as Arc<dyn TestTrait>);
    assert!(added1);
    
    // Second trait registration should fail
    let added2 = services.try_add_singleton_trait(Arc::new(Impl2) as Arc<dyn TestTrait>);
    assert!(!added2);
    
    // Verify first implementation is used
    let provider = services.build();
    let service = provider.get_required_trait::<dyn TestTrait>();
    assert_eq!(service.value(), 1);
}

#[test]
fn test_try_add_enumerable_always_adds() {
    trait TestTrait: Send + Sync {
        fn id(&self) -> i32;
    }
    
    struct Impl1;
    impl TestTrait for Impl1 {
        fn id(&self) -> i32 { 1 }
    }
    
    struct Impl2;
    impl TestTrait for Impl2 {
        fn id(&self) -> i32 { 2 }
    }
    
    let mut services = ServiceCollection::new();
    
    // Both enumerable registrations should succeed (they don't conflict)
    services.try_add_enumerable(Arc::new(Impl1) as Arc<dyn TestTrait>, Lifetime::Singleton);
    services.try_add_enumerable(Arc::new(Impl2) as Arc<dyn TestTrait>, Lifetime::Singleton);
    
    let provider = services.build();
    let implementations = provider.get_all_trait::<dyn TestTrait>().unwrap();
    assert_eq!(implementations.len(), 2);
    
    let ids: Vec<i32> = implementations.iter().map(|i| i.id()).collect();
    assert!(ids.contains(&1));
    assert!(ids.contains(&2));
}

// ===== Named Services Tests =====

#[test]
fn test_named_singleton_registration_and_resolution() {
    let mut services = ServiceCollection::new();
    
    // Register multiple named services
    services.add_named_singleton("primary", 42usize);
    services.add_named_singleton("secondary", 100usize);
    services.add_named_singleton("fallback", 200usize);
    
    let provider = services.build();
    
    // Try to resolve
    let primary = provider.get_named::<usize>("primary").unwrap();
    let secondary = provider.get_named::<usize>("secondary").unwrap();
    let fallback = provider.get_named::<usize>("fallback").unwrap();
    
    assert_eq!(*primary, 42);
    assert_eq!(*secondary, 100);
    assert_eq!(*fallback, 200);
    
    // Non-existent name should fail
    assert!(provider.get_named::<usize>("nonexistent").is_err());
}

#[test]
fn test_named_factory_different_lifetimes() {
    let counter = Arc::new(Mutex::new(0));
    
    let mut services = ServiceCollection::new();
    
    // Named singleton factory
    {
        let counter_clone = counter.clone();
        services.add_named_singleton_factory("singleton", move |_| {
            let mut c = counter_clone.lock().unwrap();
            *c += 1;
            format!("singleton-{}", *c)
        });
    }
    
    // Named scoped factory
    {
        let counter_clone = counter.clone();
        services.add_named_scoped_factory("scoped", move |_| {
            let mut c = counter_clone.lock().unwrap();
            *c += 100;
            format!("scoped-{}", *c)
        });
    }
    
    // Named transient factory
    {
        let counter_clone = counter.clone();
        services.add_named_transient_factory("transient", move |_| {
            let mut c = counter_clone.lock().unwrap();
            *c += 1000;
            format!("transient-{}", *c)
        });
    }
    
    let provider = services.build();
    let scope = provider.create_scope();
    
    // Test singleton behavior (called once)
    let singleton1 = scope.get_named::<String>("singleton").unwrap();
    let singleton2 = scope.get_named::<String>("singleton").unwrap();
    assert_eq!(*singleton1, "singleton-1");
    assert!(Arc::ptr_eq(&singleton1, &singleton2)); // Same instance
    
    // Test scoped behavior (once per scope)
    let scoped1 = scope.get_named::<String>("scoped").unwrap();
    let scoped2 = scope.get_named::<String>("scoped").unwrap();
    assert_eq!(*scoped1, "scoped-101");
    assert!(Arc::ptr_eq(&scoped1, &scoped2)); // Same instance within scope
    
    // Test transient behavior (new each time)
    let transient1 = scope.get_named::<String>("transient").unwrap();
    let transient2 = scope.get_named::<String>("transient").unwrap();
    assert_eq!(*transient1, "transient-1101");
    assert_eq!(*transient2, "transient-2101");
    assert!(!Arc::ptr_eq(&transient1, &transient2)); // Different instances
}

#[test]
fn test_named_traits() {
    trait Database: Send + Sync {
        fn connection_string(&self) -> &str;
    }
    
    struct PostgresDb {
        conn: String,
    }
    
    impl Database for PostgresDb {
        fn connection_string(&self) -> &str {
            &self.conn
        }
    }
    
    struct MySqlDb {
        conn: String,
    }
    
    impl Database for MySqlDb {
        fn connection_string(&self) -> &str {
            &self.conn
        }
    }
    
    let mut services = ServiceCollection::new();
    
    // Register named trait implementations
    services.add_named_singleton_trait(
        "postgres",
        Arc::new(PostgresDb { conn: "postgresql://localhost".to_string() }) as Arc<dyn Database>
    );
    
    services.add_named_singleton_trait(
        "mysql", 
        Arc::new(MySqlDb { conn: "mysql://localhost".to_string() }) as Arc<dyn Database>
    );
    
    let provider = services.build();
    
    // Resolve by name
    let postgres = provider.get_named_trait::<dyn Database>("postgres").unwrap();
    let mysql = provider.get_named_trait::<dyn Database>("mysql").unwrap();
    
    assert_eq!(postgres.connection_string(), "postgresql://localhost");
    assert_eq!(mysql.connection_string(), "mysql://localhost");
    
    // Test required variants
    let postgres_required = provider.get_named_trait_required::<dyn Database>("postgres");
    assert_eq!(postgres_required.connection_string(), "postgresql://localhost");
}

#[test]
fn test_named_services_in_descriptors() {
    let mut services = ServiceCollection::new();
    
    // Add named and unnamed services
    services.add_singleton(999usize); // Unnamed
    services.add_named_singleton("primary", 42usize);
    services.add_named_singleton("secondary", 100usize);
    
    let descriptors = services.get_service_descriptors();
    assert_eq!(descriptors.len(), 3);
    
    // Find unnamed service
    let unnamed = descriptors.iter()
        .find(|d| !d.is_named())
        .expect("Should find unnamed service");
    assert_eq!(unnamed.service_name(), None);
    assert!(matches!(unnamed.key, Key::Type(_, _)));
    
    // Find named services
    let named_services: Vec<_> = descriptors.iter()
        .filter(|d| d.is_named())
        .collect();
    assert_eq!(named_services.len(), 2);
    
    let primary = named_services.iter()
        .find(|d| d.service_name() == Some("primary"))
        .expect("Should find primary service");
    assert!(matches!(primary.key, Key::TypeNamed(_, _, "primary")));
    
    let secondary = named_services.iter()
        .find(|d| d.service_name() == Some("secondary"))
        .expect("Should find secondary service");
    assert!(matches!(secondary.key, Key::TraitNamed(_, "secondary") | Key::TypeNamed(_, _, "secondary")));
}

#[test]
fn test_named_vs_unnamed_services_independence() {
    let mut services = ServiceCollection::new();
    
    // Register both named and unnamed versions of same type
    services.add_singleton(999usize); // Unnamed
    services.add_named_singleton("named", 42usize); // Named
    
    let provider = services.build();
    
    // Both should be resolvable independently
    let unnamed = provider.get_required::<usize>();
    let named = provider.get_named_required::<usize>("named");
    
    assert_eq!(*unnamed, 999);
    assert_eq!(*named, 42);
    assert!(!Arc::ptr_eq(&unnamed, &named)); // Different instances
}

#[test] 
fn test_named_multi_trait_implementation() {
    trait Logger: Send + Sync {
        fn log(&self, msg: &str) -> String;
    }
    
    struct FileLogger;
    impl Logger for FileLogger {
        fn log(&self, msg: &str) -> String {
            format!("FILE: {}", msg)
        }
    }
    
    struct ConsoleLogger;
    impl Logger for ConsoleLogger {
        fn log(&self, msg: &str) -> String {
            format!("CONSOLE: {}", msg)
        }
    }
    
    let mut services = ServiceCollection::new();
    
    // Add named multi-trait implementations
    services.add_named_trait_implementation(
        "file",
        Arc::new(FileLogger) as Arc<dyn Logger>,
        Lifetime::Singleton
    );
    
    services.add_named_trait_implementation(
        "console", 
        Arc::new(ConsoleLogger) as Arc<dyn Logger>,
        Lifetime::Singleton
    );
    
    let descriptors = services.get_service_descriptors();
    assert_eq!(descriptors.len(), 2);
    
    // Both should be multi-trait registrations with different names
    let file_desc = descriptors.iter()
        .find(|d| matches!(d.key, Key::MultiTrait(name, _) if name.contains("file")))
        .expect("Should find file logger");
    
    let console_desc = descriptors.iter()
        .find(|d| matches!(d.key, Key::MultiTrait(name, _) if name.contains("console")))
        .expect("Should find console logger");
    
    assert_eq!(file_desc.lifetime, Lifetime::Singleton);
    assert_eq!(console_desc.lifetime, Lifetime::Singleton);
}

// ===== Integration Tests =====

#[test]
fn test_all_features_together() {
    #[derive(Debug, PartialEq)]
    struct ServiceConfig {
        timeout_ms: u64,
        retry_count: u32,
    }
    
    trait ApiClient: Send + Sync {
        fn name(&self) -> &str;
    }
    
    struct HttpClient {
        name: String,
    }
    
    impl ApiClient for HttpClient {
        fn name(&self) -> &str {
            &self.name
        }
    }
    
    let mut services = ServiceCollection::new();
    
    // 1. Service with metadata
    services.add_with_metadata(
        42usize,
        Lifetime::Singleton,
        ServiceConfig { timeout_ms: 5000, retry_count: 3 }
    );
    
    // 2. Named services
    services.add_named_singleton("primary-client", "primary-url".to_string());
    services.add_named_singleton("backup-client", "backup-url".to_string());
    
    // 3. Conditional registration (first should succeed, second should fail)
    let added1 = services.try_add_singleton_factory::<i32, _>(|_| 100);
    let added2 = services.try_add_singleton_factory::<i32, _>(|_| 200);
    assert!(added1);
    assert!(!added2);
    
    // 4. Named trait with metadata
    services.add_named_singleton_trait(
        "http",
        Arc::new(HttpClient { name: "HttpClient".to_string() }) as Arc<dyn ApiClient>
    );
    
    // Service descriptors show all registrations
    let descriptors = services.get_service_descriptors();
    
    // Access metadata before building
    let key = Key::Type(TypeId::of::<usize>(), "usize");
    let config = services.get_metadata::<ServiceConfig>(&key).unwrap();
    assert_eq!(config.timeout_ms, 5000);
    assert_eq!(config.retry_count, 3);
    
    let provider = services.build();
    
    // Test all features work together
    assert_eq!(descriptors.len(), 5); // usize + 2 named strings + i32 + named trait
    
    // Find service with metadata
    let usize_desc = descriptors.iter()
        .find(|d| d.type_name().contains("usize"))
        .expect("Should find usize");
    assert!(usize_desc.has_metadata);
    
    // Resolve named services
    let primary = provider.get_named_required::<String>("primary-client");
    let backup = provider.get_named_required::<String>("backup-client");
    assert_eq!(*primary, "primary-url");
    assert_eq!(*backup, "backup-url");
    
    // Conditional registration worked
    let number = provider.get_required::<i32>();
    assert_eq!(*number, 100); // First registration won
    
    // Named trait resolution
    let http_client = provider.get_named_trait_required::<dyn ApiClient>("http");
    assert_eq!(http_client.name(), "HttpClient");
}