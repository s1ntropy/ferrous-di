use ferrous_di::{ServiceCollection, Resolver};
use std::sync::{Arc, Mutex};

#[test]
fn test_concrete_singleton() {
    let mut sc = ServiceCollection::new();
    sc.add_singleton(42usize);
    sc.add_singleton("hello".to_string());
    
    let sp = sc.build();
    
    let num1 = sp.get_required::<usize>();
    let num2 = sp.get_required::<usize>();
    let str1 = sp.get_required::<String>();
    let str2 = sp.get_required::<String>();
    
    assert_eq!(*num1, 42);
    assert_eq!(*str1, "hello");
    assert!(Arc::ptr_eq(&num1, &num2)); // Same instance
    assert!(Arc::ptr_eq(&str1, &str2)); // Same instance
}

#[test]
fn test_factory_with_dependencies() {
    #[derive(Debug)]
    struct Config {
        port: u16,
    }
    
    #[derive(Debug)]
    struct Server {
        config: Arc<Config>,
        name: String,
    }
    
    let mut sc = ServiceCollection::new();
    sc.add_singleton(Config { port: 8080 });
    sc.add_singleton_factory::<Server, _>(|r| {
        Server {
            config: r.get_required::<Config>(),
            name: "MyServer".to_string(),
        }
    });
    
    let sp = sc.build();
    let server = sp.get_required::<Server>();
    
    assert_eq!(server.config.port, 8080);
    assert_eq!(server.name, "MyServer");
}

#[test]
fn test_transient_creates_new_instances() {
    let counter = Arc::new(Mutex::new(0));
    let counter_clone = counter.clone();
    
    let mut sc = ServiceCollection::new();
    sc.add_transient_factory::<String, _>(move |_| {
        let mut c = counter_clone.lock().unwrap();
        *c += 1;
        format!("instance-{}", *c)
    });
    
    let sp = sc.build();
    
    let a = sp.get_required::<String>();
    let b = sp.get_required::<String>();
    let c = sp.get_required::<String>();
    
    assert_eq!(*a, "instance-1");
    assert_eq!(*b, "instance-2");
    assert_eq!(*c, "instance-3");
    
    // All different instances
    assert!(!Arc::ptr_eq(&a, &b));
    assert!(!Arc::ptr_eq(&b, &c));
    assert!(!Arc::ptr_eq(&a, &c));
}

#[test]
fn test_not_found_error() {
    struct UnregisteredType;
    
    let sc = ServiceCollection::new();
    let sp = sc.build();
    
    // Should return error when trying to resolve unregistered type
    let result = sp.get::<UnregisteredType>();
    assert!(result.is_err(), "Expected error when resolving unregistered type");
}

#[test]
fn test_replace_semantics() {
    let mut sc = ServiceCollection::new();
    
    // Register first value
    sc.add_singleton(1usize);
    // Replace with second value
    sc.add_singleton(2usize);
    
    let sp = sc.build();
    let value = sp.get_required::<usize>();
    
    // Should get the last registered value
    assert_eq!(*value, 2);
}

#[test]
fn test_complex_dependency_graph() {
    struct A {
        value: i32,
    }
    
    struct B {
        a: Arc<A>,
    }
    
    struct C {
        a: Arc<A>,
        b: Arc<B>,
    }
    
    let mut sc = ServiceCollection::new();
    
    sc.add_singleton(A { value: 100 });
    
    sc.add_singleton_factory::<B, _>(|r| B {
        a: r.get_required::<A>(),
    });
    
    sc.add_singleton_factory::<C, _>(|r| C {
        a: r.get_required::<A>(),
        b: r.get_required::<B>(),
    });
    
    let sp = sc.build();
    let c = sp.get_required::<C>();
    
    assert_eq!(c.a.value, 100);
    assert_eq!(c.b.a.value, 100);
    // A is singleton, so should be same instance
    assert!(Arc::ptr_eq(&c.a, &c.b.a));
}