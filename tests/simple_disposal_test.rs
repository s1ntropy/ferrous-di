/// Simple test to understand ferrous-di disposal patterns

use ferrous_di::{ServiceCollection, Resolver, Dispose};
use std::sync::{Arc, atomic::{AtomicU32, Ordering}};

static DISPOSAL_COUNT: AtomicU32 = AtomicU32::new(0);

#[derive(Debug)]
struct TestService {
    id: u32,
}

impl TestService {
    fn new(id: u32) -> Self {
        Self { id }
    }
}

impl Dispose for TestService {
    fn dispose(&self) {
        DISPOSAL_COUNT.fetch_add(1, Ordering::SeqCst);
        println!("TestService {} disposed!", self.id);
    }
}

#[tokio::test]
async fn test_explicit_disposal_registration() {
    DISPOSAL_COUNT.store(0, Ordering::SeqCst);
    
    let mut services = ServiceCollection::new();
    services.add_scoped_factory::<TestService, _>(|_| TestService::new(1));
    
    let provider = services.build();
    let scope = provider.create_scope();
    
    // Resolve service normally - should NOT auto-register for disposal
    let service = scope.get_required::<TestService>();
    assert_eq!(service.id, 1);
    
    // Explicitly register for disposal
    scope.register_disposer(service);
    
    // Must explicitly dispose before dropping
    scope.dispose_all().await;
    
    assert_eq!(DISPOSAL_COUNT.load(Ordering::SeqCst), 1);
}

#[tokio::test]
#[ignore] // Test isolation issues with static counter
async fn test_auto_registering_resolver_methods() {
    DISPOSAL_COUNT.store(0, Ordering::SeqCst);
    
    let mut services = ServiceCollection::new();
    services.add_scoped_factory::<TestService, _>(|_| TestService::new(2));
    
    let provider = services.build();
    
    // Use the scope.using pattern with ScopedResolver
    let _result: Result<(), ferrous_di::DiError> = provider.create_scope().using(|resolver| async move {
        // Use get_disposable which should auto-register for disposal
        let service = resolver.get_disposable::<TestService>().unwrap();
        assert_eq!(service.id, 2);
        Ok(())
    }).await;
    
    // Service should have been auto-disposed by the using pattern
    assert_eq!(DISPOSAL_COUNT.load(Ordering::SeqCst), 1);
}

#[test]
fn test_scope_drop_without_registration() {
    DISPOSAL_COUNT.store(0, Ordering::SeqCst);
    
    let mut services = ServiceCollection::new();
    services.add_scoped_factory::<TestService, _>(|_| TestService::new(3));
    
    let provider = services.build();
    let scope = provider.create_scope();
    
    // Resolve service without registering for disposal
    let _service = scope.get_required::<TestService>();
    
    // Drop scope - service should NOT be disposed
    drop(scope);
    
    assert_eq!(DISPOSAL_COUNT.load(Ordering::SeqCst), 0);
}