/// Property-based tests for service registration
/// 
/// These tests use proptest to generate random inputs and verify invariants
/// that should hold for all valid service registrations.

use ferrous_di::{ServiceCollection, Resolver, ServiceModule, ServiceCollectionModuleExt};
use proptest::prelude::*;
use std::sync::Arc;

// Test data structures
#[derive(Debug, Clone)]
struct TestService {
    id: u32,
    name: String,
}

#[derive(Debug, Clone)]  
struct ConfigService {
    value: i32,
}

// Property: Any sequence of singleton registrations should result in the last registration winning
proptest! {
    #[test]
    fn singleton_last_registration_wins(ids in prop::collection::vec(0u32..1000, 1..10)) {
        let mut services = ServiceCollection::new();
        
        // Register multiple singletons with the same type but different values
        for id in &ids {
            services.add_singleton(TestService {
                id: *id,
                name: format!("service_{}", id),
            });
        }
        
        let provider = services.build();
        let resolved = provider.get_required::<TestService>();
        
        // Should get the last registered value
        prop_assert_eq!(resolved.id, *ids.last().unwrap());
        prop_assert_eq!(&resolved.name, &format!("service_{}", ids.last().unwrap()));
    }
}

proptest! {
    #[test]
    fn singleton_factory_deterministic(seed in 0u32..1000) {
        let mut services = ServiceCollection::new();
        
        // Register singleton factory with deterministic behavior
        services.add_singleton_factory::<TestService, _>(move |_| {
            TestService {
                id: seed,
                name: format!("factory_{}", seed),
            }
        });
        
        let provider = services.build();
        
        // Multiple resolutions should return the same instance
        let service1 = provider.get_required::<TestService>();
        let service2 = provider.get_required::<TestService>();
        
        prop_assert!(Arc::ptr_eq(&service1, &service2));
        prop_assert_eq!(service1.id, seed);
    }
}

proptest! {
    #[test]
    fn scoped_services_isolated_across_scopes(values in prop::collection::vec(1i32..1000, 1..10)) {
        let mut services = ServiceCollection::new();
        
        // Register scoped factory
        services.add_scoped_factory::<ConfigService, _>(move |_| {
            ConfigService { value: values[0] } // Use first value for consistency
        });
        
        let provider = services.build();
        let scope1 = provider.create_scope();
        let scope2 = provider.create_scope();
        
        // Each scope should get its own instance
        let config1a = scope1.get_required::<ConfigService>();
        let config1b = scope1.get_required::<ConfigService>();
        let config2 = scope2.get_required::<ConfigService>();
        
        // Same instance within scope
        prop_assert!(Arc::ptr_eq(&config1a, &config1b));
        // Different instances across scopes  
        prop_assert!(!Arc::ptr_eq(&config1a, &config2));
    }
}

proptest! {
    #[test]
    fn transient_services_always_new(count in 1usize..20) {
        let mut services = ServiceCollection::new();
        
        services.add_transient_factory::<TestService, _>(|_| {
            // Use thread-safe counter for transient services
            use std::sync::atomic::{AtomicU32, Ordering};
            static COUNTER: AtomicU32 = AtomicU32::new(0);
            let id = COUNTER.fetch_add(1, Ordering::SeqCst) + 1;
            TestService {
                id,
                name: format!("transient_{}", id),
            }
        });
        
        let provider = services.build();
        let mut instances = Vec::new();
        
        // Resolve multiple times
        for _ in 0..count {
            instances.push(provider.get_required::<TestService>());
        }
        
        // All instances should be different
        for i in 0..instances.len() {
            for j in (i+1)..instances.len() {
                prop_assert!(!Arc::ptr_eq(&instances[i], &instances[j]));
            }
        }
        
        // Should have unique IDs (though not necessarily sequential due to other tests)
        for i in 0..instances.len() {
            for j in (i+1)..instances.len() {
                prop_assert_ne!(instances[i].id, instances[j].id);
            }
        }
    }
}

// Property: Module registration should be commutative (order shouldn't matter for independent modules)
proptest! {
    #[test]
    fn module_registration_order_independence(
        service1_id in 1u32..100,
        service2_value in 1i32..100,
    ) {
        struct Module1 {
            id: u32,
        }
        
        impl ServiceModule for Module1 {
            fn register_services(self, services: &mut ServiceCollection) -> ferrous_di::DiResult<()> {
                services.add_singleton(TestService {
                    id: self.id,
                    name: format!("module1_{}", self.id),
                });
                Ok(())
            }
        }
        
        struct Module2 {
            value: i32,
        }
        
        impl ServiceModule for Module2 {
            fn register_services(self, services: &mut ServiceCollection) -> ferrous_di::DiResult<()> {
                services.add_singleton(ConfigService {
                    value: self.value,
                });
                Ok(())
            }
        }
        
        // Register in order A -> B
        let mut services1 = ServiceCollection::new();
        services1.add_module_mut(Module1 { id: service1_id }).unwrap();
        services1.add_module_mut(Module2 { value: service2_value }).unwrap();
        let provider1 = services1.build();
        
        // Register in order B -> A  
        let mut services2 = ServiceCollection::new();
        services2.add_module_mut(Module2 { value: service2_value }).unwrap();
        services2.add_module_mut(Module1 { id: service1_id }).unwrap();
        let provider2 = services2.build();
        
        // Should resolve to the same values regardless of registration order
        let test1 = provider1.get_required::<TestService>();
        let config1 = provider1.get_required::<ConfigService>();
        let test2 = provider2.get_required::<TestService>();
        let config2 = provider2.get_required::<ConfigService>();
        
        prop_assert_eq!(test1.id, test2.id);
        prop_assert_eq!(&test1.name, &test2.name);
        prop_assert_eq!(config1.value, config2.value);
    }
}

proptest! {
    #[test]
    fn service_collection_builder_invariants(
        singleton_count in 0usize..10,
        scoped_count in 0usize..10,
        transient_count in 0usize..10,
    ) {
        let mut services = ServiceCollection::new();
        let mut _expected_total = 0;
        
        // Add various service types
        for i in 0..singleton_count {
            services.add_singleton(TestService {
                id: i as u32,
                name: format!("singleton_{}", i),
            });
            _expected_total += 1;
        }
        
        for i in 0..scoped_count {
            services.add_scoped_factory::<ConfigService, _>(move |_| {
                ConfigService { value: i as i32 }
            });
            _expected_total += 1;
        }
        
        for i in 0..transient_count {
            services.add_transient_factory::<String, _>(move |_| {
                format!("transient_{}", i)
            });
            _expected_total += 1;
        }
        
        // ServiceProvider should be buildable and contain all services
        let provider = services.build();
        
        // This is a basic invariant - we should be able to create scopes
        let _scope = provider.create_scope();
        
        // If we registered any singletons of TestService, we should be able to resolve one
        if singleton_count > 0 {
            let service = provider.get_required::<TestService>();
            // Should get the last registered singleton (invariant from earlier test)
            prop_assert_eq!(service.id, (singleton_count - 1) as u32);
        }
    }
}