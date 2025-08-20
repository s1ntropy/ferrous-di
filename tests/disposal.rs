use ferrous_di::{ServiceCollection, Resolver, Dispose, AsyncDispose};
use async_trait::async_trait;
use std::sync::{Arc, Mutex};

#[test]
fn test_sync_disposal_lifo_order() {
    // Track disposal order
    let disposal_order = Arc::new(Mutex::new(Vec::new()));
    
    struct ServiceA {
        name: String,
        order: Arc<Mutex<Vec<String>>>,
    }
    
    impl Dispose for ServiceA {
        fn dispose(&self) {
            self.order.lock().unwrap().push(self.name.clone());
        }
    }

    struct ServiceB {
        name: String,
        order: Arc<Mutex<Vec<String>>>,
    }
    
    impl Dispose for ServiceB {
        fn dispose(&self) {
            self.order.lock().unwrap().push(self.name.clone());
        }
    }

    struct ServiceC {
        name: String,
        order: Arc<Mutex<Vec<String>>>,
    }
    
    impl Dispose for ServiceC {
        fn dispose(&self) {
            self.order.lock().unwrap().push(self.name.clone());
        }
    }
    
    let mut sc = ServiceCollection::new();
    
    // Register different service types in order: First, Second, Third
    let order_clone1 = disposal_order.clone();
    sc.add_singleton_factory::<ServiceA, _>(move |r| {
        let service = Arc::new(ServiceA {
            name: "First".to_string(),
            order: order_clone1.clone(),
        });
        r.register_disposer(service.clone());
        ServiceA {
            name: "First".to_string(),
            order: order_clone1.clone(),
        }
    });
    
    let order_clone2 = disposal_order.clone();
    sc.add_singleton_factory::<ServiceB, _>(move |r| {
        let service = Arc::new(ServiceB {
            name: "Second".to_string(),
            order: order_clone2.clone(),
        });
        r.register_disposer(service.clone());
        ServiceB {
            name: "Second".to_string(),
            order: order_clone2.clone(),
        }
    });
    
    let order_clone3 = disposal_order.clone();
    sc.add_transient_factory::<ServiceC, _>(move |r| {
        let service = Arc::new(ServiceC {
            name: "Third".to_string(),
            order: order_clone3.clone(),
        });
        r.register_disposer(service.clone());
        ServiceC {
            name: "Third".to_string(),
            order: order_clone3.clone(),
        }
    });
    
    let sp = sc.build();
    
    // Resolve services (triggers registration of disposers)
    let _first = sp.get_required::<ServiceA>();
    let _second = sp.get_required::<ServiceB>();
    let _third = sp.get_required::<ServiceC>(); // Transient
    
    // Create a runtime for the async dispose_all
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        sp.dispose_all().await;
    });
    
    let order = disposal_order.lock().unwrap();
    // Should dispose in LIFO order: Third, Second, First
    assert_eq!(*order, vec!["Third", "Second", "First"]);
}

#[tokio::test]
async fn test_async_disposal_before_sync() {
    let disposal_order = Arc::new(Mutex::new(Vec::new()));
    
    struct AsyncService {
        name: String,
        order: Arc<Mutex<Vec<String>>>,
    }
    
    #[async_trait]
    impl AsyncDispose for AsyncService {
        async fn dispose(&self) {
            // Simulate some async work
            tokio::time::sleep(tokio::time::Duration::from_millis(1)).await;
            self.order.lock().unwrap().push(format!("async-{}", self.name));
        }
    }
    
    struct SyncService {
        name: String,
        order: Arc<Mutex<Vec<String>>>,
    }
    
    impl Dispose for SyncService {
        fn dispose(&self) {
            self.order.lock().unwrap().push(format!("sync-{}", self.name));
        }
    }
    
    let mut sc = ServiceCollection::new();
    
    // Register both async and sync disposers
    let order_clone1 = disposal_order.clone();
    sc.add_singleton_factory::<AsyncService, _>(move |r| {
        let service = Arc::new(AsyncService {
            name: "A".to_string(),
            order: order_clone1.clone(),
        });
        r.register_async_disposer(service.clone());
        AsyncService {
            name: "A".to_string(),
            order: order_clone1.clone(),
        }
    });
    
    let order_clone2 = disposal_order.clone();
    sc.add_singleton_factory::<SyncService, _>(move |r| {
        let service = Arc::new(SyncService {
            name: "B".to_string(),
            order: order_clone2.clone(),
        });
        r.register_disposer(service.clone());
        SyncService {
            name: "B".to_string(),
            order: order_clone2.clone(),
        }
    });
    
    let sp = sc.build();
    
    // Resolve services
    let _async_service = sp.get_required::<AsyncService>();
    let _sync_service = sp.get_required::<SyncService>();
    
    sp.dispose_all().await;
    
    let order = disposal_order.lock().unwrap();
    // Async disposers should run before sync disposers
    // Order: async-A, then sync-B
    assert_eq!(*order, vec!["async-A", "sync-B"]);
}

#[tokio::test]
async fn test_scoped_disposal_isolation() {
    let disposal_order = Arc::new(Mutex::new(Vec::new()));
    
    struct ScopedService {
        name: String,
        order: Arc<Mutex<Vec<String>>>,
    }
    
    impl Dispose for ScopedService {
        fn dispose(&self) {
            self.order.lock().unwrap().push(self.name.clone());
        }
    }
    
    let mut sc = ServiceCollection::new();
    
    let order_clone = disposal_order.clone();
    sc.add_scoped_factory::<ScopedService, _>(move |r| {
        let counter = std::sync::atomic::AtomicUsize::new(0);
        let id = counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let service = Arc::new(ScopedService {
            name: format!("scoped-{}", id),
            order: order_clone.clone(),
        });
        r.register_disposer(service.clone());
        ScopedService {
            name: format!("scoped-{}", id),
            order: order_clone.clone(),
        }
    });
    
    let sp = sc.build();
    let scope1 = sp.create_scope();
    let scope2 = sp.create_scope();
    
    // Resolve services in different scopes
    let _service1 = scope1.get_required::<ScopedService>();
    let _service2 = scope2.get_required::<ScopedService>();
    
    // Dispose scope1 only
    scope1.dispose_all().await;
    
    let order = disposal_order.lock().unwrap();
    // Only scope1's service should be disposed
    assert_eq!(order.len(), 1);
    assert!(order[0].starts_with("scoped-"));
}