/// Concurrent access integration tests
/// 
/// These tests verify that ferrous-di behaves correctly under concurrent access,
/// testing thread safety, singleton consistency, and scope isolation.

use ferrous_di::{ServiceCollection, Resolver};
use std::sync::{Arc, Barrier, Mutex, atomic::{AtomicU32, Ordering}};
use std::thread;
use std::time::Duration;

// ===== Test Services =====

#[derive(Debug)]
pub struct CounterService {
    count: AtomicU32,
    thread_id: String,
}

impl CounterService {
    pub fn new() -> Self {
        Self {
            count: AtomicU32::new(0),
            thread_id: format!("created-by-{:?}", thread::current().id()),
        }
    }
    
    pub fn increment(&self) -> u32 {
        self.count.fetch_add(1, Ordering::SeqCst) + 1
    }
    
    pub fn get_count(&self) -> u32 {
        self.count.load(Ordering::SeqCst)
    }
    
    pub fn get_thread_id(&self) -> &str {
        &self.thread_id
    }
}

#[derive(Debug)]
pub struct SharedResource {
    data: Mutex<Vec<String>>,
    access_count: AtomicU32,
}

impl SharedResource {
    pub fn new() -> Self {
        Self {
            data: Mutex::new(Vec::new()),
            access_count: AtomicU32::new(0),
        }
    }
    
    pub fn add_entry(&self, entry: String) {
        let mut data = self.data.lock().unwrap();
        data.push(entry);
        self.access_count.fetch_add(1, Ordering::SeqCst);
    }
    
    pub fn get_entries(&self) -> Vec<String> {
        self.access_count.fetch_add(1, Ordering::SeqCst);
        self.data.lock().unwrap().clone()
    }
    
    pub fn get_access_count(&self) -> u32 {
        self.access_count.load(Ordering::SeqCst)
    }
}

#[derive(Debug)]
pub struct ThreadLocalService {
    id: u32,
    creation_thread: String,
}

impl ThreadLocalService {
    pub fn new() -> Self {
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        Self {
            id: COUNTER.fetch_add(1, Ordering::SeqCst),
            creation_thread: format!("{:?}", thread::current().id()),
        }
    }
    
    pub fn get_id(&self) -> u32 {
        self.id
    }
    
    pub fn get_creation_thread(&self) -> &str {
        &self.creation_thread
    }
}

// ===== Integration Tests =====

#[test]
fn test_singleton_thread_safety() {
    let mut services = ServiceCollection::new();
    services.add_singleton(CounterService::new());
    services.add_singleton(SharedResource::new());
    
    let provider = Arc::new(services.build());
    let thread_count = 8;
    let operations_per_thread = 100;
    let barrier = Arc::new(Barrier::new(thread_count));
    
    let handles: Vec<_> = (0..thread_count)
        .map(|thread_id| {
            let provider = Arc::clone(&provider);
            let barrier = Arc::clone(&barrier);
            
            thread::spawn(move || {
                barrier.wait(); // Synchronize start
                
                // Get services
                let counter = provider.get_required::<CounterService>();
                let shared = provider.get_required::<SharedResource>();
                
                // Perform operations
                for i in 0..operations_per_thread {
                    counter.increment();
                    shared.add_entry(format!("thread-{}-op-{}", thread_id, i));
                }
                
                // Return some data for verification
                (counter.get_count(), shared.get_access_count())
            })
        })
        .collect();
    
    // Collect results
    let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    
    // Verify results
    let provider_ref = &*provider;
    let final_counter = provider_ref.get_required::<CounterService>();
    let final_shared = provider_ref.get_required::<SharedResource>();
    
    // Counter should have been incremented by all threads
    assert_eq!(final_counter.get_count(), (thread_count * operations_per_thread) as u32);
    
    // Shared resource should have entries from all threads
    let entries = final_shared.get_entries();
    assert_eq!(entries.len(), (thread_count * operations_per_thread) as usize);
    
    // All operations should have been counted
    // Each thread: operations_per_thread add_entry calls + 1 final get_entries call
    let total_expected_access = (thread_count * operations_per_thread) as u32 + 1;
    assert_eq!(final_shared.get_access_count(), total_expected_access);
}

#[test]
fn test_scoped_service_isolation() {
    let mut services = ServiceCollection::new();
    services.add_singleton(CounterService::new());
    services.add_scoped_factory::<ThreadLocalService, _>(|_| ThreadLocalService::new());
    
    let provider = Arc::new(services.build());
    let thread_count = 10;
    let barrier = Arc::new(Barrier::new(thread_count));
    
    let handles: Vec<_> = (0..thread_count)
        .map(|thread_id| {
            let provider = Arc::clone(&provider);
            let barrier = Arc::clone(&barrier);
            
            thread::spawn(move || {
                barrier.wait();
                
                // Create scope and get services
                let scope = provider.create_scope();
                let counter = scope.get_required::<CounterService>();
                let local_service1 = scope.get_required::<ThreadLocalService>();
                let local_service2 = scope.get_required::<ThreadLocalService>();
                
                // Verify singleton is shared
                let root_counter = provider.get_required::<CounterService>();
                assert!(Arc::ptr_eq(&counter, &root_counter));
                
                // Verify scoped services are same within scope
                assert!(Arc::ptr_eq(&local_service1, &local_service2));
                
                (thread_id, local_service1.get_id(), local_service1.get_creation_thread().to_string())
            })
        })
        .collect();
    
    let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    
    // Verify each thread got a different scoped service instance
    let ids: Vec<u32> = results.iter().map(|(_, id, _)| *id).collect();
    let mut sorted_ids = ids.clone();
    sorted_ids.sort();
    sorted_ids.dedup();
    assert_eq!(sorted_ids.len(), thread_count); // All IDs should be unique
}

#[test]
fn test_concurrent_scope_creation() {
    let mut services = ServiceCollection::new();
    services.add_singleton(SharedResource::new());
    services.add_scoped_factory::<ThreadLocalService, _>(|_| ThreadLocalService::new());
    
    let provider = Arc::new(services.build());
    let thread_count = 20;
    let scopes_per_thread = 5;
    let barrier = Arc::new(Barrier::new(thread_count));
    
    let handles: Vec<_> = (0..thread_count)
        .map(|thread_id| {
            let provider = Arc::clone(&provider);
            let barrier = Arc::clone(&barrier);
            
            thread::spawn(move || {
                barrier.wait();
                
                let mut scope_ids = Vec::new();
                
                for scope_num in 0..scopes_per_thread {
                    let scope = provider.create_scope();
                    let local_service = scope.get_required::<ThreadLocalService>();
                    let shared = scope.get_required::<SharedResource>();
                    
                    // Record this access
                    shared.add_entry(format!("thread-{}-scope-{}", thread_id, scope_num));
                    scope_ids.push(local_service.get_id());
                }
                
                scope_ids
            })
        })
        .collect();
    
    let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    
    // Collect all scope service IDs
    let all_ids: Vec<u32> = results.into_iter().flatten().collect();
    let mut unique_ids = all_ids.clone();
    unique_ids.sort();
    unique_ids.dedup();
    
    // Each scope should have gotten a unique service instance
    assert_eq!(unique_ids.len(), (thread_count * scopes_per_thread) as usize);
    
    // Verify shared resource received all entries
    let shared = provider.get_required::<SharedResource>();
    let entries = shared.get_entries();
    assert_eq!(entries.len(), (thread_count * scopes_per_thread) as usize);
}

#[test]
#[ignore] // Temporarily disabled due to timing sensitivity
fn test_stress_concurrent_resolution() {
    let mut services = ServiceCollection::new();
    services.add_singleton_factory::<CounterService, _>(|_| CounterService::new());
    services.add_transient_factory::<ThreadLocalService, _>(|_| ThreadLocalService::new());
    
    let provider = Arc::new(services.build());
    let thread_count = 16;
    let resolutions_per_thread = 200;
    let barrier = Arc::new(Barrier::new(thread_count));
    
    let handles: Vec<_> = (0..thread_count)
        .map(|_| {
            let provider = Arc::clone(&provider);
            let barrier = Arc::clone(&barrier);
            
            thread::spawn(move || {
                barrier.wait();
                
                let mut transient_ids = Vec::new();
                
                for _ in 0..resolutions_per_thread {
                    // Get singleton (should be same instance)
                    let counter = provider.get_required::<CounterService>();
                    counter.increment();
                    
                    // Get transient (should be different each time)
                    let transient = provider.get_required::<ThreadLocalService>();
                    transient_ids.push(transient.get_id());
                    
                    // Small delay to increase contention
                    thread::sleep(Duration::from_nanos(1));
                }
                
                transient_ids
            })
        })
        .collect();
    
    let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    
    // Verify singleton behavior
    let counter = provider.get_required::<CounterService>();
    assert_eq!(counter.get_count(), (thread_count * resolutions_per_thread) as u32);
    
    // Verify transient behavior - all should be unique
    let all_transient_ids: Vec<u32> = results.into_iter().flatten().collect();
    let mut unique_transient_ids = all_transient_ids.clone();
    unique_transient_ids.sort();
    unique_transient_ids.dedup();
    assert_eq!(unique_transient_ids.len(), all_transient_ids.len());
}

#[test]
fn test_mixed_lifetime_concurrent_access() {
    let mut services = ServiceCollection::new();
    
    // Singleton - shared across all threads and scopes
    services.add_singleton(SharedResource::new());
    
    // Scoped - one per scope
    services.add_scoped_factory::<CounterService, _>(|_| CounterService::new());
    
    // Transient - new instance every time
    services.add_transient_factory::<ThreadLocalService, _>(|_| ThreadLocalService::new());
    
    let provider = Arc::new(services.build());
    let thread_count = 8;
    let barrier = Arc::new(Barrier::new(thread_count));
    
    let handles: Vec<_> = (0..thread_count)
        .map(|thread_id| {
            let provider = Arc::clone(&provider);
            let barrier = Arc::clone(&barrier);
            
            thread::spawn(move || {
                barrier.wait();
                
                let scope = provider.create_scope();
                
                // Get singleton (should be same across all threads)
                let shared = scope.get_required::<SharedResource>();
                
                // Get scoped service (should be same within this scope, different across threads)
                let scoped_counter1 = scope.get_required::<CounterService>();
                let scoped_counter2 = scope.get_required::<CounterService>();
                
                // Get transient services (should be different each time)
                let transient1 = scope.get_required::<ThreadLocalService>();
                let transient2 = scope.get_required::<ThreadLocalService>();
                
                // Verify scoped behavior
                assert!(Arc::ptr_eq(&scoped_counter1, &scoped_counter2));
                
                // Verify transient behavior  
                assert!(!Arc::ptr_eq(&transient1, &transient2));
                assert_ne!(transient1.get_id(), transient2.get_id());
                
                // Record access
                shared.add_entry(format!("thread-{}", thread_id));
                
                (thread_id, scoped_counter1.get_thread_id().to_string())
            })
        })
        .collect();
    
    let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    
    // Verify singleton was shared
    let shared = provider.get_required::<SharedResource>();
    let entries = shared.get_entries();
    assert_eq!(entries.len(), thread_count);
    
    // Verify each thread got its own scoped service
    let thread_ids: Vec<String> = results.into_iter().map(|(_, thread_id)| thread_id).collect();
    let mut unique_thread_ids = thread_ids.clone();
    unique_thread_ids.sort();
    unique_thread_ids.dedup();
    assert_eq!(unique_thread_ids.len(), thread_count);
}

#[test] 
fn test_concurrent_provider_access() {
    // Test multiple threads accessing the same provider simultaneously
    let mut services = ServiceCollection::new();
    services.add_singleton(CounterService::new());
    
    let provider = Arc::new(services.build());
    let thread_count = 32;
    let barrier = Arc::new(Barrier::new(thread_count));
    
    let handles: Vec<_> = (0..thread_count)
        .map(|_| {
            let provider = Arc::clone(&provider);
            let barrier = Arc::clone(&barrier);
            
            thread::spawn(move || {
                barrier.wait();
                
                // Rapid-fire service resolution
                for _ in 0..100 {
                    let counter = provider.get_required::<CounterService>();
                    counter.increment();
                }
            })
        })
        .collect();
    
    // Wait for all threads
    for handle in handles {
        handle.join().unwrap();
    }
    
    // Verify final state
    let counter = provider.get_required::<CounterService>();
    assert_eq!(counter.get_count(), (thread_count * 100) as u32);
}