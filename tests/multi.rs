use ferrous_di::{ServiceCollection, Resolver, Lifetime};
use std::sync::Arc;

#[test]
fn test_multi_binding_basics() {
    trait Plugin: Send + Sync {
        fn name(&self) -> &str;
    }
    
    struct PluginA;
    impl Plugin for PluginA {
        fn name(&self) -> &str { "PluginA" }
    }
    
    struct PluginB;
    impl Plugin for PluginB {
        fn name(&self) -> &str { "PluginB" }
    }
    
    struct PluginC;
    impl Plugin for PluginC {
        fn name(&self) -> &str { "PluginC" }
    }
    
    let mut sc = ServiceCollection::new();
    
    sc.add_trait_implementation(Arc::new(PluginA) as Arc<dyn Plugin>, Lifetime::Singleton);
    sc.add_trait_implementation(Arc::new(PluginB) as Arc<dyn Plugin>, Lifetime::Singleton);
    sc.add_trait_implementation(Arc::new(PluginC) as Arc<dyn Plugin>, Lifetime::Singleton);
    
    let sp = sc.build();
    let plugins = sp.get_all_trait::<dyn Plugin>().unwrap();
    
    assert_eq!(plugins.len(), 3);
    assert_eq!(plugins[0].name(), "PluginA");
    assert_eq!(plugins[1].name(), "PluginB");
    assert_eq!(plugins[2].name(), "PluginC");
    
    // Get all again - should return same instances for singletons
    let plugins2 = sp.get_all_trait::<dyn Plugin>().unwrap();
    assert!(Arc::ptr_eq(&plugins[0], &plugins2[0]));
    assert!(Arc::ptr_eq(&plugins[1], &plugins2[1]));
    assert!(Arc::ptr_eq(&plugins[2], &plugins2[2]));
}

#[test]
fn test_multi_binding_mixed_lifetimes() {
    trait Handler: Send + Sync {
        fn id(&self) -> i32;
    }
    
    struct SingletonHandler;
    impl Handler for SingletonHandler {
        fn id(&self) -> i32 { 1 }
    }
    
    struct TransientHandler {
        count: i32,
    }
    impl Handler for TransientHandler {
        fn id(&self) -> i32 { self.count }
    }
    
    let counter = std::sync::Arc::new(std::sync::Mutex::new(100));
    let counter_clone = counter.clone();
    
    let mut sc = ServiceCollection::new();
    
    // Add singleton handler
    sc.add_trait_implementation(Arc::new(SingletonHandler) as Arc<dyn Handler>, Lifetime::Singleton);
    
    // Add transient handler factory
    sc.add_trait_factory::<dyn Handler, _>(Lifetime::Transient, move |_| {
        let mut c = counter_clone.lock().unwrap();
        *c += 1;
        Arc::new(TransientHandler { count: *c }) as Arc<dyn Handler>
    });
    
    let sp = sc.build();
    
    // First call
    let handlers1 = sp.get_all_trait::<dyn Handler>().unwrap();
    assert_eq!(handlers1.len(), 2);
    assert_eq!(handlers1[0].id(), 1); // Singleton
    assert_eq!(handlers1[1].id(), 101); // Transient
    
    // Second call
    let handlers2 = sp.get_all_trait::<dyn Handler>().unwrap();
    assert_eq!(handlers2.len(), 2);
    assert_eq!(handlers2[0].id(), 1); // Same singleton
    assert_eq!(handlers2[1].id(), 102); // New transient
    
    // Singleton should be same instance
    assert!(Arc::ptr_eq(&handlers1[0], &handlers2[0]));
    // Transient should be different instance
    assert!(!Arc::ptr_eq(&handlers1[1], &handlers2[1]));
}

#[test]
fn test_multi_binding_in_scopes() {
    trait Middleware: Send + Sync {
        fn name(&self) -> &str;
    }
    
    struct AuthMiddleware {
        scope_id: String,
    }
    impl Middleware for AuthMiddleware {
        fn name(&self) -> &str { &self.scope_id }
    }
    
    struct LoggingMiddleware;
    impl Middleware for LoggingMiddleware {
        fn name(&self) -> &str { "logging" }
    }
    
    let counter = std::sync::Arc::new(std::sync::Mutex::new(0));
    let counter_clone = counter.clone();
    
    let mut sc = ServiceCollection::new();
    
    // Singleton middleware
    sc.add_trait_implementation(Arc::new(LoggingMiddleware) as Arc<dyn Middleware>, Lifetime::Singleton);
    
    // Scoped middleware
    sc.add_trait_factory::<dyn Middleware, _>(Lifetime::Scoped, move |_| {
        let mut c = counter_clone.lock().unwrap();
        *c += 1;
        Arc::new(AuthMiddleware {
            scope_id: format!("auth-{}", *c),
        }) as Arc<dyn Middleware>
    });
    
    let sp = sc.build();
    
    let scope1 = sp.create_scope();
    let scope2 = sp.create_scope();
    
    let middlewares1a = scope1.get_all_trait::<dyn Middleware>().unwrap();
    let middlewares1b = scope1.get_all_trait::<dyn Middleware>().unwrap();
    let middlewares2 = scope2.get_all_trait::<dyn Middleware>().unwrap();
    
    assert_eq!(middlewares1a.len(), 2);
    assert_eq!(middlewares1b.len(), 2);
    assert_eq!(middlewares2.len(), 2);
    
    // Singleton should be same across all
    assert!(Arc::ptr_eq(&middlewares1a[0], &middlewares1b[0]));
    assert!(Arc::ptr_eq(&middlewares1a[0], &middlewares2[0]));
    assert_eq!(middlewares1a[0].name(), "logging");
    
    // Scoped should be same within scope
    assert!(Arc::ptr_eq(&middlewares1a[1], &middlewares1b[1]));
    assert_eq!(middlewares1a[1].name(), "auth-1");
    
    // But different across scopes
    assert!(!Arc::ptr_eq(&middlewares1a[1], &middlewares2[1]));
    assert_eq!(middlewares2[1].name(), "auth-2");
}

#[test]
fn test_multi_binding_empty() {
    trait EmptyTrait: Send + Sync {}
    
    let sc = ServiceCollection::new();
    let sp = sc.build();
    
    let items = sp.get_all_trait::<dyn EmptyTrait>().unwrap();
    assert_eq!(items.len(), 0);
}

#[test]
fn test_single_binding_fallback_to_multi() {
    trait Service: Send + Sync {
        fn value(&self) -> i32;
    }
    
    struct ServiceImpl {
        val: i32,
    }
    impl Service for ServiceImpl {
        fn value(&self) -> i32 { self.val }
    }
    
    let mut sc = ServiceCollection::new();
    
    // Only register as multi-binding (no single binding)
    sc.add_trait_implementation(Arc::new(ServiceImpl { val: 10 }) as Arc<dyn Service>, Lifetime::Singleton);
    sc.add_trait_implementation(Arc::new(ServiceImpl { val: 20 }) as Arc<dyn Service>, Lifetime::Singleton);
    
    let sp = sc.build();
    
    // get_trait should return the last multi-binding
    let single = sp.get_trait::<dyn Service>().unwrap();
    assert_eq!(single.value(), 20);
    
    // get_all_trait should return all
    let all = sp.get_all_trait::<dyn Service>().unwrap();
    assert_eq!(all.len(), 2);
    assert_eq!(all[0].value(), 10);
    assert_eq!(all[1].value(), 20);
}

#[test]
fn test_multi_binding_with_dependencies() {
    struct Config {
        prefix: String,
    }
    
    trait Processor: Send + Sync {
        fn process(&self, input: &str) -> String;
    }
    
    struct PrefixProcessor {
        config: Arc<Config>,
    }
    impl Processor for PrefixProcessor {
        fn process(&self, input: &str) -> String {
            format!("{}: {}", self.config.prefix, input)
        }
    }
    
    struct UppercaseProcessor;
    impl Processor for UppercaseProcessor {
        fn process(&self, input: &str) -> String {
            input.to_uppercase()
        }
    }
    
    let mut sc = ServiceCollection::new();
    
    sc.add_singleton(Config {
        prefix: ">>".to_string(),
    });
    
    sc.add_trait_factory::<dyn Processor, _>(Lifetime::Singleton, |r| {
        Arc::new(PrefixProcessor {
            config: r.get_required::<Config>(),
        }) as Arc<dyn Processor>
    });
    
    sc.add_trait_implementation(Arc::new(UppercaseProcessor) as Arc<dyn Processor>, Lifetime::Singleton);
    
    let sp = sc.build();
    let processors = sp.get_all_trait::<dyn Processor>().unwrap();
    
    assert_eq!(processors.len(), 2);
    assert_eq!(processors[0].process("hello"), ">>: hello");
    assert_eq!(processors[1].process("hello"), "HELLO");
}