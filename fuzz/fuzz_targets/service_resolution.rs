#![no_main]

use libfuzzer_sys::fuzz_target;
use ferrous_di::{ServiceCollection, Resolver};
use std::sync::Arc;

fuzz_target!(|data: &[u8]| {
    if data.len() < 4 {
        return;
    }
    
    let mut services = ServiceCollection::new();
    
    // Use first 4 bytes to determine resolution pattern
    let pattern = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
    let register_service = pattern % 2 == 0;
    
    if register_service {
        services.add_singleton(TestService { id: 42 });
        services.add_singleton_trait::<dyn TestTrait>(Arc::new(TestServiceImpl { value: 100 }));
        services.add_scoped_factory::<ScopedService, _>(|_| ScopedService { data: "scoped".to_string() });
    }
    
    if let Ok(provider) = std::panic::catch_unwind(|| services.build()) {
        let resolution_type = (pattern >> 1) % 8;
        
        let _ = std::panic::catch_unwind(|| {
            match resolution_type {
                0 => {
                    // Test required resolution when service exists
                    if register_service {
                        let service = provider.get_required::<TestService>();
                        assert_eq!(service.id, 42);
                    }
                },
                1 => {
                    // Test optional resolution
                    let service = provider.get::<TestService>();
                    if register_service {
                        assert!(service.is_ok());
                    } else {
                        assert!(service.is_err());
                    }
                },
                2 => {
                    // Test trait resolution
                    if register_service {
                        let trait_obj = provider.get_required_trait::<dyn TestTrait>();
                        assert_eq!(trait_obj.get_value(), 100);
                    }
                },
                3 => {
                    // Test scoped resolution
                    if register_service {
                        let scope = provider.create_scope();
                        let scoped1 = scope.get_required::<ScopedService>();
                        let scoped2 = scope.get_required::<ScopedService>();
                        // Same instance within scope
                        assert!(Arc::ptr_eq(&scoped1, &scoped2));
                        assert_eq!(scoped1.data, "scoped");
                    }
                },
                4 => {
                    // Test multiple scopes
                    if register_service {
                        let scope1 = provider.create_scope();
                        let scope2 = provider.create_scope();
                        let service1 = scope1.get_required::<ScopedService>();
                        let service2 = scope2.get_required::<ScopedService>();
                        // Different instances across scopes
                        assert!(!Arc::ptr_eq(&service1, &service2));
                    }
                },
                5 => {
                    // Test singleton consistency across scopes
                    if register_service {
                        let scope1 = provider.create_scope();
                        let scope2 = provider.create_scope();
                        let singleton1 = scope1.get_required::<TestService>();
                        let singleton2 = scope2.get_required::<TestService>();
                        // Same singleton instance across scopes
                        assert!(Arc::ptr_eq(&singleton1, &singleton2));
                    }
                },
                6 => {
                    // Test trait optional resolution
                    let trait_result = provider.get_trait::<dyn TestTrait>();
                    if register_service {
                        assert!(trait_result.is_ok());
                    } else {
                        assert!(trait_result.is_err());
                    }
                },
                7 => {
                    // Test get_all (empty in this case)
                    let all_services = provider.get_all::<TestService>();
                    if register_service {
                        assert_eq!(all_services.len(), 1);
                    } else {
                        assert_eq!(all_services.len(), 0);
                    }
                },
                _ => unreachable!(),
            }
        });
    }
});

#[derive(Debug, Clone)]
struct TestService {
    id: u32,
}

#[derive(Debug, Clone)]
struct ScopedService {
    data: String,
}

trait TestTrait: Send + Sync {
    fn get_value(&self) -> i32;
}

#[derive(Debug)]
struct TestServiceImpl {
    value: i32,
}

impl TestTrait for TestServiceImpl {
    fn get_value(&self) -> i32 {
        self.value
    }
}