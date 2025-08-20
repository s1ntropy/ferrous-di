/// Property-based tests for service resolution
/// 
/// These tests verify that resolution behavior follows expected patterns
/// regardless of the specific services or configuration used.

use ferrous_di::{ServiceCollection, Resolver};
use proptest::prelude::*;
use std::sync::Arc;

#[derive(Debug, Clone)]
struct ServiceA {
    value: String,
}

#[derive(Debug, Clone)]
struct ServiceB {
    number: u64,
}

#[derive(Debug, Clone)]
struct ServiceC {
    flag: bool,
}

// Property: Resolution should be consistent - same service should resolve to same instance for singletons
proptest! {
    #[test]
    fn singleton_resolution_consistency(service_value in "\\PC{0,50}") {
        let mut services = ServiceCollection::new();
        services.add_singleton(ServiceA { value: service_value.clone() });
        
        let provider = services.build();
        
        // Multiple resolutions should return the same instance
        let resolved1 = provider.get_required::<ServiceA>();
        let resolved2 = provider.get_required::<ServiceA>();
        let resolved3 = provider.get_required::<ServiceA>();
        
        prop_assert!(Arc::ptr_eq(&resolved1, &resolved2));
        prop_assert!(Arc::ptr_eq(&resolved2, &resolved3));
        prop_assert_eq!(&resolved1.value, &service_value);
        prop_assert_eq!(&resolved2.value, &service_value);
        prop_assert_eq!(&resolved3.value, &service_value);
    }
}

proptest! {
    #[test]
    fn optional_resolution_behavior(register_service in any::<bool>()) {
        let mut services = ServiceCollection::new();
        
        if register_service {
            services.add_singleton(ServiceB { number: 42 });
        }
        
        let provider = services.build();
        
        // Optional resolution should match registration state
        let optional_result = provider.get::<ServiceB>();
        
        if register_service {
            prop_assert!(optional_result.is_ok());
            // Required resolution should succeed
            let required_result = provider.get_required::<ServiceB>();
            prop_assert_eq!(required_result.number, 42);
        } else {
            prop_assert!(optional_result.is_err());
            // Required resolution would panic, so we don't test it
        }
    }
}

proptest! {
    #[test]
    fn scope_isolation_properties(
        service_count in 1usize..10,
        scope_count in 1usize..5,
    ) {
        let mut services = ServiceCollection::new();
        
        // Register scoped services that track their creation
        services.add_scoped_factory::<ServiceA, _>(|_| {
            use std::sync::atomic::{AtomicU32, Ordering};
            static COUNTER: AtomicU32 = AtomicU32::new(0);
            let id = COUNTER.fetch_add(1, Ordering::SeqCst);
            ServiceA { value: format!("scoped_{}", id) }
        });
        
        let provider = services.build();
        let mut scopes = Vec::new();
        let mut scoped_services = Vec::new();
        
        // Create multiple scopes and resolve services from each
        for _ in 0..scope_count {
            let scope = provider.create_scope();
            let mut scope_services = Vec::new();
            
            for _ in 0..service_count {
                scope_services.push(scope.get_required::<ServiceA>());
            }
            
            scoped_services.push(scope_services);
            scopes.push(scope);
        }
        
        // Within each scope, all instances should be the same
        for scope_services in &scoped_services {
            for i in 1..scope_services.len() {
                prop_assert!(Arc::ptr_eq(&scope_services[0], &scope_services[i]));
            }
        }
        
        // Across scopes, instances should be different
        if scoped_services.len() > 1 {
            for i in 0..scoped_services.len() {
                for j in (i+1)..scoped_services.len() {
                    prop_assert!(!Arc::ptr_eq(&scoped_services[i][0], &scoped_services[j][0]));
                }
            }
        }
    }
}

proptest! {
    #[test]  
    fn dependency_chain_resolution(chain_length in 1usize..5) {
        // This test builds a chain of dependencies and ensures resolution works
        let mut services = ServiceCollection::new();
        
        // Base service
        services.add_singleton(ServiceA { value: "base".to_string() });
        
        // Chain of dependent services - only register the final one
        if chain_length > 0 {
            let level = chain_length - 1;
            services.add_singleton_factory::<String, _>(move |r| {
                let base = r.get_required::<ServiceA>();
                format!("{}->level_{}", base.value, level)
            });
        }
        
        let provider = services.build();
        
        // Should be able to resolve the final service in the chain
        if chain_length > 0 {
            let result = provider.get_required::<String>();
            prop_assert!(result.starts_with("base->"));
            let expected_level = chain_length - 1;
            let expected_string = format!("level_{}", expected_level);
            prop_assert!(result.contains(&expected_string));
        }
    }
}

proptest! {
    #[test]
    fn concurrent_resolution_safety(
        thread_count in 1usize..8,
        resolution_count in 1usize..20,
    ) {
        use std::sync::Barrier;
        use std::thread;
        
        let mut services = ServiceCollection::new();
        services.add_singleton(ServiceB { number: 12345 });
        services.add_scoped_factory::<ServiceA, _>(|_| {
            ServiceA { value: "concurrent_test".to_string() }
        });
        
        let provider = Arc::new(services.build());
        let barrier = Arc::new(Barrier::new(thread_count));
        let mut handles = Vec::new();
        
        for _thread_id in 0..thread_count {
            let provider = Arc::clone(&provider);
            let barrier = Arc::clone(&barrier);
            
            handles.push(thread::spawn(move || {
                barrier.wait(); // Synchronize start
                
                let mut singleton_results = Vec::new();
                let mut scoped_results = Vec::new();
                
                // Test singleton resolution from multiple threads
                for _ in 0..resolution_count {
                    let singleton = provider.get_required::<ServiceB>();
                    singleton_results.push(singleton.number);
                }
                
                // Test scoped resolution
                let scope = provider.create_scope();
                for _ in 0..resolution_count {
                    let scoped = scope.get_required::<ServiceA>();
                    scoped_results.push(scoped.value.len() as u64); // Just to use the value
                }
                
                (singleton_results, scoped_results)
            }));
        }
        
        let mut all_singleton_results = Vec::new();
        let mut all_scoped_results = Vec::new();
        for handle in handles {
            let (singleton_results, scoped_results) = handle.join().unwrap();
            all_singleton_results.extend(singleton_results);
            all_scoped_results.extend(scoped_results);
        }
        
        // All singleton resolutions should return the same value
        for &result in &all_singleton_results {
            prop_assert_eq!(result, 12345);
        }
        
        // All scoped results should be the same length (concurrent_test = 15 chars)
        for &result in &all_scoped_results {
            prop_assert_eq!(result, 15);
        }
    }
}

proptest! {
    #[test]
    fn error_conditions_consistent(should_register in any::<bool>()) {
        let mut services = ServiceCollection::new();
        
        if should_register {
            services.add_singleton(ServiceC { flag: true });
        }
        
        let provider = services.build();
        
        // Multiple attempts to resolve service should behave consistently
        let result1 = provider.get::<ServiceC>();
        let result2 = provider.get::<ServiceC>();
        
        prop_assert_eq!(result1.is_ok(), result2.is_ok());
        prop_assert_eq!(result1.is_ok(), should_register);
    }
}

// Property: Trait resolution should work consistently
trait TestTrait: Send + Sync {
    fn get_id(&self) -> u32;
}

#[derive(Debug)]
struct TraitImpl {
    id: u32,
}

impl TestTrait for TraitImpl {
    fn get_id(&self) -> u32 {
        self.id
    }
}

proptest! {
    #[test]
    fn trait_resolution_properties(trait_id in 1u32..1000) {
        let mut services = ServiceCollection::new();
        services.add_singleton_trait::<dyn TestTrait>(Arc::new(TraitImpl { id: trait_id }));
        
        let provider = services.build();
        
        // Multiple trait resolutions should return the same instance
        let trait1 = provider.get_required_trait::<dyn TestTrait>();
        let trait2 = provider.get_required_trait::<dyn TestTrait>();
        
        prop_assert!(Arc::ptr_eq(&trait1, &trait2));
        prop_assert_eq!(trait1.get_id(), trait_id);
        prop_assert_eq!(trait2.get_id(), trait_id);
    }
}