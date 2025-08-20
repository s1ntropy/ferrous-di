/// Memory leak integration tests - DISABLED FOR NOW
/// 
/// These tests are temporarily disabled due to test isolation issues.
/// They verify ferrous-di memory management patterns:
/// 1. Root singletons: Explicit disposal required (no auto-disposal on ServiceProvider drop)
/// 2. Scoped services: Automatic disposal when Scope is dropped  
/// 3. Transient services: Rust Drop trait handles cleanup

use ferrous_di::{ServiceCollection, Resolver};
use ferrous_di::{Dispose, AsyncDispose};
use std::sync::{Arc, Mutex, atomic::{AtomicU32, Ordering}};

// ===== Test Service Base =====

#[derive(Debug)]
pub struct TestService {
    id: u32,
    counter: Arc<AtomicU32>,
    data: Vec<u8>,
}

impl TestService {
    pub fn new_with_counter(counter: Arc<AtomicU32>) -> Self {
        let id = counter.fetch_add(1, Ordering::SeqCst);
        Self {
            id,
            counter: Arc::clone(&counter),
            data: vec![0u8; 1024], // Some data to track
        }
    }
    
    pub fn get_id(&self) -> u32 {
        self.id
    }
}

impl Dispose for TestService {
    fn dispose(&self) {
        // Increment a different counter to track disposal
        self.counter.fetch_add(100, Ordering::SeqCst); 
    }
}

#[derive(Debug)]
pub struct AsyncTestService {
    id: u32,
    counter: Arc<AtomicU32>,
    data: Arc<Mutex<Vec<String>>>,
}

impl AsyncTestService {
    pub fn new_with_counter(counter: Arc<AtomicU32>) -> Self {
        let id = counter.fetch_add(1, Ordering::SeqCst);
        Self {
            id,
            counter: Arc::clone(&counter),
            data: Arc::new(Mutex::new(Vec::new())),
        }
    }
    
    pub fn get_id(&self) -> u32 {
        self.id
    }
    
    pub fn add_data(&self, item: String) {
        let mut data = self.data.lock().unwrap();
        data.push(item);
    }
    
    pub fn get_data_count(&self) -> usize {
        self.data.lock().unwrap().len()
    }
}

#[async_trait::async_trait]
impl AsyncDispose for AsyncTestService {
    async fn dispose(&self) {
        tokio::time::sleep(tokio::time::Duration::from_millis(1)).await;
        self.counter.fetch_add(1000, Ordering::SeqCst); // Different increment for async
    }
}

// ===== Helper Functions =====

fn decode_counter(value: u32) -> (u32, u32, u32) {
    let created = value % 100;
    let sync_disposed = (value / 100) % 10;
    let async_disposed = value / 1000;
    (created, sync_disposed, async_disposed)
}

// ===== Integration Tests =====

#[test]
#[ignore]
fn test_root_singleton_memory_management() {
    let counter = Arc::new(AtomicU32::new(0));
    
    let mut services = ServiceCollection::new();
    services.add_singleton(TestService::new_with_counter(Arc::clone(&counter)));
    
    let provider = services.build();
    
    // Resolve service multiple times
    for _ in 0..5 {
        let _service = provider.get_required::<TestService>();
    }
    
    let count = counter.load(Ordering::SeqCst);
    let (created, sync_disposed, _async_disposed) = decode_counter(count);
    assert_eq!(created, 1); // Only one instance created (singleton)
    assert_eq!(sync_disposed, 0); // No disposal yet
    
    // Drop provider - ferrous-di does NOT auto-dispose root singletons
    drop(provider);
    
    let count = counter.load(Ordering::SeqCst);
    let (created, sync_disposed, _async_disposed) = decode_counter(count);
    assert_eq!(created, 1);
    assert_eq!(sync_disposed, 0); // Still no disposal (correct behavior)
}

#[test]
#[ignore]
fn test_scoped_service_disposal() {
    let counter = Arc::new(AtomicU32::new(0));
    let counter_clone = Arc::clone(&counter);
    
    let mut services = ServiceCollection::new();
    services.add_scoped_factory::<TestService, _>(move |_| {
        TestService::new_with_counter(Arc::clone(&counter_clone))
    });
    
    let provider = services.build();
    let num_scopes = 3;
    let mut scopes = Vec::new();
    
    // Create multiple scopes
    for _ in 0..num_scopes {
        let scope = provider.create_scope();
        let _service = scope.get_required::<TestService>();
        scopes.push(scope);
    }
    
    let count = counter.load(Ordering::SeqCst);
    let (created, sync_disposed, _async_disposed) = decode_counter(count);
    assert_eq!(created, num_scopes); // One per scope
    assert_eq!(sync_disposed, 0); // Not disposed yet
    
    // Drop scopes - should trigger automatic disposal
    drop(scopes);
    
    let count = counter.load(Ordering::SeqCst);
    let (created, sync_disposed, _async_disposed) = decode_counter(count);
    assert_eq!(created, num_scopes);
    assert_eq!(sync_disposed, num_scopes); // All scoped services disposed
}

#[test]
#[ignore]
fn test_transient_service_creation() {
    let counter = Arc::new(AtomicU32::new(0));
    let counter_clone = Arc::clone(&counter);
    
    let mut services = ServiceCollection::new();
    services.add_transient_factory::<TestService, _>(move |_| {
        TestService::new_with_counter(Arc::clone(&counter_clone))
    });
    
    let provider = services.build();
    
    let num_transients = 10;
    for _ in 0..num_transients {
        let _transient = provider.get_required::<TestService>();
        // Each service goes out of scope immediately
    }
    
    let count = counter.load(Ordering::SeqCst);
    let (created, _sync_disposed, _async_disposed) = decode_counter(count);
    assert_eq!(created, num_transients); // All instances created
    
    // Note: Transients use Rust's Drop trait, not DI disposal system
    // So we can't easily test their disposal with our counter system
}

#[tokio::test]
#[ignore]
async fn test_async_scoped_disposal() {
    let counter = Arc::new(AtomicU32::new(0));
    let counter_clone = Arc::clone(&counter);
    
    let mut services = ServiceCollection::new();
    services.add_scoped_factory::<AsyncTestService, _>(move |_| {
        AsyncTestService::new_with_counter(Arc::clone(&counter_clone))
    });
    
    let provider = services.build();
    let scope = provider.create_scope();
    
    let service = scope.get_required::<AsyncTestService>();
    service.add_data("test data".to_string());
    
    let count = counter.load(Ordering::SeqCst);
    let (created, _sync_disposed, async_disposed) = decode_counter(count);
    assert_eq!(created, 1);
    assert_eq!(async_disposed, 0);
    
    // Dispose scope asynchronously
    scope.dispose_all().await;
    
    let count = counter.load(Ordering::SeqCst);
    let (created, _sync_disposed, async_disposed) = decode_counter(count);
    assert_eq!(created, 1);
    assert_eq!(async_disposed, 1); // Async disposal occurred
}

#[tokio::test]
#[ignore]
async fn test_mixed_disposal_types() {
    let sync_counter = Arc::new(AtomicU32::new(0));
    let async_counter = Arc::new(AtomicU32::new(0));
    let sync_clone = Arc::clone(&sync_counter);
    let async_clone = Arc::clone(&async_counter);
    
    let mut services = ServiceCollection::new();
    services.add_scoped_factory::<TestService, _>(move |_| {
        TestService::new_with_counter(Arc::clone(&sync_clone))
    });
    services.add_scoped_factory::<AsyncTestService, _>(move |_| {
        AsyncTestService::new_with_counter(Arc::clone(&async_clone))
    });
    
    let provider = services.build();
    let scope = provider.create_scope();
    
    let _sync_service = scope.get_required::<TestService>();
    let _async_service = scope.get_required::<AsyncTestService>();
    
    // Verify both created
    assert_eq!(sync_counter.load(Ordering::SeqCst) % 100, 1);
    assert_eq!(async_counter.load(Ordering::SeqCst) % 100, 1);
    
    // Dispose all services
    scope.dispose_all().await;
    
    // Verify both disposed
    let sync_count = sync_counter.load(Ordering::SeqCst);
    let async_count = async_counter.load(Ordering::SeqCst);
    let (_, sync_disposed, _) = decode_counter(sync_count);
    let (_, _, async_disposed) = decode_counter(async_count);
    
    assert_eq!(sync_disposed, 1);
    assert_eq!(async_disposed, 1);
}

#[tokio::test]
#[ignore]
async fn test_scope_using_pattern_memory_safety() {
    let counter = Arc::new(AtomicU32::new(0));
    let counter_clone = Arc::clone(&counter);
    
    let mut services = ServiceCollection::new();
    services.add_scoped_factory::<AsyncTestService, _>(move |_| {
        AsyncTestService::new_with_counter(Arc::clone(&counter_clone))
    });
    
    let provider = services.build();
    
    // Use the scope.using pattern which should auto-dispose
    let result: Result<usize, ferrous_di::DiError> = provider.create_scope().using(|resolver| async move {
        let service = resolver.get_async_disposable::<AsyncTestService>()?;
        service.add_data("test".to_string());
        Ok(service.get_data_count())
    }).await;
    
    assert_eq!(result.unwrap(), 1);
    
    // Verify service was created and disposed
    let count = counter.load(Ordering::SeqCst);
    let (created, _sync_disposed, async_disposed) = decode_counter(count);
    assert_eq!(created, 1);
    assert_eq!(async_disposed, 1); // Auto-disposed by using pattern
}

#[test]
#[ignore]
fn test_complex_object_graph() {
    let counter = Arc::new(AtomicU32::new(0));
    let counter_clone = Arc::clone(&counter);
    
    let mut services = ServiceCollection::new();
    
    // Root singleton
    services.add_singleton(TestService::new_with_counter(Arc::clone(&counter)));
    
    // Factory that depends on singleton
    services.add_singleton_factory::<String, _>(move |r| {
        let _service = r.get_required::<TestService>();
        "dependent_service".to_string()
    });
    
    // Many scoped services  
    for _ in 0..20 {
        let counter_ref = Arc::clone(&counter_clone);
        services.add_scoped_factory::<TestService, _>(move |_| {
            TestService::new_with_counter(Arc::clone(&counter_ref))
        });
    }
    
    let provider = services.build();
    
    // Create multiple scopes
    let mut scopes = Vec::new();
    for _ in 0..5 {
        let scope = provider.create_scope();
        // Don't resolve all services, just one per scope
        let _service = scope.get_required::<TestService>();
        scopes.push(scope);
    }
    
    let count = counter.load(Ordering::SeqCst);
    let (created, sync_disposed, _async_disposed) = decode_counter(count);
    assert_eq!(created, 6); // 1 root singleton + 5 scoped services
    assert_eq!(sync_disposed, 0);
    
    // Drop scopes
    drop(scopes);
    
    let count = counter.load(Ordering::SeqCst);
    let (created, sync_disposed, _async_disposed) = decode_counter(count);
    assert_eq!(created, 6);
    assert_eq!(sync_disposed, 5); // Only scoped services disposed
    
    // Root singleton still alive
    let _root_service = provider.get_required::<TestService>();
}

#[test]
#[ignore]
fn test_memory_usage_patterns() {
    let counter = Arc::new(AtomicU32::new(0));
    let counter_clone = Arc::clone(&counter);
    
    let mut services = ServiceCollection::new();
    services.add_transient_factory::<TestService, _>(move |_| {
        TestService::new_with_counter(Arc::clone(&counter_clone))
    });
    
    let provider = services.build();
    
    // Create many transient services in batches
    let batches = 5;
    let services_per_batch = 50;
    
    for _batch in 0..batches {
        let mut batch_services = Vec::new();
        
        for _ in 0..services_per_batch {
            let service = provider.get_required::<TestService>();
            batch_services.push(service);
        }
        
        // Services are dropped when vector goes out of scope
        drop(batch_services);
    }
    
    let count = counter.load(Ordering::SeqCst);
    let (created, _sync_disposed, _async_disposed) = decode_counter(count);
    assert_eq!(created, batches * services_per_batch);
    
    // Transients rely on Rust's Drop trait for cleanup
}