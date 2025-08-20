use ferrous_di::{ServiceCollection, Resolver};
use std::sync::{Arc, Mutex};

#[test]
fn test_scoped_lifetime() {
    #[derive(Debug, Clone)]
    struct RequestContext {
        id: String,
    }
    
    let counter = Arc::new(Mutex::new(0));
    let counter_clone = counter.clone();
    
    let mut sc = ServiceCollection::new();
    sc.add_scoped_factory::<RequestContext, _>(move |_| {
        let mut c = counter_clone.lock().unwrap();
        *c += 1;
        RequestContext {
            id: format!("req-{}", *c),
        }
    });
    
    let sp = sc.build();
    
    // Create two scopes
    let scope1 = sp.create_scope();
    let scope2 = sp.create_scope();
    
    // Get instances from scope1
    let ctx1a = scope1.get_required::<RequestContext>();
    let ctx1b = scope1.get_required::<RequestContext>();
    
    // Get instances from scope2
    let ctx2a = scope2.get_required::<RequestContext>();
    let ctx2b = scope2.get_required::<RequestContext>();
    
    // Same instance within same scope
    assert!(Arc::ptr_eq(&ctx1a, &ctx1b));
    assert!(Arc::ptr_eq(&ctx2a, &ctx2b));
    
    // Different instances across scopes
    assert!(!Arc::ptr_eq(&ctx1a, &ctx2a));
    
    // Values should be different
    assert_eq!(ctx1a.id, "req-1");
    assert_eq!(ctx2a.id, "req-2");
}

#[test]
fn test_cannot_resolve_scoped_from_root() {
    struct ScopedService;
    
    let mut sc = ServiceCollection::new();
    sc.add_scoped_factory::<ScopedService, _>(|_| ScopedService);
    
    let sp = sc.build();
    
    // Should return error when trying to resolve scoped service from root
    let result = sp.get::<ScopedService>();
    assert!(result.is_err(), "Expected error when resolving scoped service from root");
}

#[test]
fn test_scoped_with_singleton_dependency() {
    struct Database {
        connection: String,
    }
    
    struct Repository {
        db: Arc<Database>,
        scope_id: String,
    }
    
    let counter = Arc::new(Mutex::new(0));
    let counter_clone = counter.clone();
    
    let mut sc = ServiceCollection::new();
    
    // Singleton database
    sc.add_singleton(Database {
        connection: "postgres://localhost".to_string(),
    });
    
    // Scoped repository
    sc.add_scoped_factory::<Repository, _>(move |r| {
        let mut c = counter_clone.lock().unwrap();
        *c += 1;
        Repository {
            db: r.get_required::<Database>(),
            scope_id: format!("scope-{}", *c),
        }
    });
    
    let sp = sc.build();
    
    let scope1 = sp.create_scope();
    let scope2 = sp.create_scope();
    
    let repo1 = scope1.get_required::<Repository>();
    let repo2 = scope2.get_required::<Repository>();
    
    // Different repository instances
    assert!(!Arc::ptr_eq(&repo1, &repo2));
    assert_eq!(repo1.scope_id, "scope-1");
    assert_eq!(repo2.scope_id, "scope-2");
    
    // Same database instance (singleton)
    assert!(Arc::ptr_eq(&repo1.db, &repo2.db));
    assert_eq!(repo1.db.connection, "postgres://localhost");
}

#[test]
fn test_scoped_depending_on_scoped() {
    struct UserContext {
        user_id: String,
    }
    
    struct RequestHandler {
        context: Arc<UserContext>,
        handler_id: String,
    }
    
    let user_counter = Arc::new(Mutex::new(0));
    let user_counter_clone = user_counter.clone();
    
    let handler_counter = Arc::new(Mutex::new(0));
    let handler_counter_clone = handler_counter.clone();
    
    let mut sc = ServiceCollection::new();
    
    sc.add_scoped_factory::<UserContext, _>(move |_| {
        let mut c = user_counter_clone.lock().unwrap();
        *c += 1;
        UserContext {
            user_id: format!("user-{}", *c),
        }
    });
    
    sc.add_scoped_factory::<RequestHandler, _>(move |r| {
        let mut c = handler_counter_clone.lock().unwrap();
        *c += 1;
        RequestHandler {
            context: r.get_required::<UserContext>(),
            handler_id: format!("handler-{}", *c),
        }
    });
    
    let sp = sc.build();
    let scope = sp.create_scope();
    
    let handler1 = scope.get_required::<RequestHandler>();
    let handler2 = scope.get_required::<RequestHandler>();
    let context = scope.get_required::<UserContext>();
    
    // Same handler instance (scoped)
    assert!(Arc::ptr_eq(&handler1, &handler2));
    
    // Same context instance used by handler
    assert!(Arc::ptr_eq(&handler1.context, &context));
    
    // Values check
    assert_eq!(handler1.handler_id, "handler-1");
    assert_eq!(handler1.context.user_id, "user-1");
}

#[test]
fn test_mixed_lifetimes_in_scope() {
    struct Singleton {
        value: String,
    }
    
    struct Scoped {
        singleton: Arc<Singleton>,
        id: String,
    }
    
    struct Transient {
        scoped: Arc<Scoped>,
        count: i32,
    }
    
    let scoped_counter = Arc::new(Mutex::new(0));
    let scoped_counter_clone = scoped_counter.clone();
    
    let transient_counter = Arc::new(Mutex::new(0));
    let transient_counter_clone = transient_counter.clone();
    
    let mut sc = ServiceCollection::new();
    
    sc.add_singleton(Singleton {
        value: "shared".to_string(),
    });
    
    sc.add_scoped_factory::<Scoped, _>(move |r| {
        let mut c = scoped_counter_clone.lock().unwrap();
        *c += 1;
        Scoped {
            singleton: r.get_required::<Singleton>(),
            id: format!("scoped-{}", *c),
        }
    });
    
    sc.add_transient_factory::<Transient, _>(move |r| {
        let mut c = transient_counter_clone.lock().unwrap();
        *c += 1;
        Transient {
            scoped: r.get_required::<Scoped>(),
            count: *c,
        }
    });
    
    let sp = sc.build();
    let scope = sp.create_scope();
    
    let t1 = scope.get_required::<Transient>();
    let t2 = scope.get_required::<Transient>();
    
    // Different transient instances
    assert!(!Arc::ptr_eq(&t1, &t2));
    assert_eq!(t1.count, 1);
    assert_eq!(t2.count, 2);
    
    // Same scoped instance
    assert!(Arc::ptr_eq(&t1.scoped, &t2.scoped));
    assert_eq!(t1.scoped.id, "scoped-1");
    
    // Same singleton instance
    assert!(Arc::ptr_eq(&t1.scoped.singleton, &t2.scoped.singleton));
    assert_eq!(t1.scoped.singleton.value, "shared");
}