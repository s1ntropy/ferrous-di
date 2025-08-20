#![no_main]

use libfuzzer_sys::fuzz_target;
use ferrous_di::{ServiceCollection, Resolver};
use std::sync::Arc;

fuzz_target!(|data: &[u8]| {
    if data.len() < 8 {
        return;
    }
    
    let mut services = ServiceCollection::new();
    
    // Use first 4 bytes to determine service registration pattern
    let pattern = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
    
    // Use next 4 bytes for service values
    let value = i32::from_le_bytes([data[4], data[5], data[6], data[7]]);
    
    // Test different registration patterns
    match pattern % 6 {
        0 => {
            // Singleton registration
            services.add_singleton(TestService { value });
            
            if let Ok(provider) = std::panic::catch_unwind(|| services.build()) {
                let _ = std::panic::catch_unwind(|| {
                    let service = provider.get_required::<TestService>();
                    assert_eq!(service.value, value);
                });
            }
        },
        1 => {
            // Factory registration
            services.add_singleton_factory::<TestService, _>(move |_| {
                TestService { value }
            });
            
            if let Ok(provider) = std::panic::catch_unwind(|| services.build()) {
                let _ = std::panic::catch_unwind(|| {
                    let service = provider.get_required::<TestService>();
                    assert_eq!(service.value, value);
                });
            }
        },
        2 => {
            // Scoped registration
            services.add_scoped_factory::<TestService, _>(move |_| {
                TestService { value }
            });
            
            if let Ok(provider) = std::panic::catch_unwind(|| services.build()) {
                let _ = std::panic::catch_unwind(|| {
                    let scope = provider.create_scope();
                    let service = scope.get_required::<TestService>();
                    assert_eq!(service.value, value);
                });
            }
        },
        3 => {
            // Transient registration
            services.add_transient_factory::<TestService, _>(move |_| {
                TestService { value }
            });
            
            if let Ok(provider) = std::panic::catch_unwind(|| services.build()) {
                let _ = std::panic::catch_unwind(|| {
                    let service1 = provider.get_required::<TestService>();
                    let service2 = provider.get_required::<TestService>();
                    // Transient services should be different instances
                    assert!(!Arc::ptr_eq(&service1, &service2));
                    assert_eq!(service1.value, value);
                    assert_eq!(service2.value, value);
                });
            }
        },
        4 => {
            // Multiple registrations of the same type (last wins)
            services.add_singleton(TestService { value: value / 2 });
            services.add_singleton(TestService { value });
            
            if let Ok(provider) = std::panic::catch_unwind(|| services.build()) {
                let _ = std::panic::catch_unwind(|| {
                    let service = provider.get_required::<TestService>();
                    assert_eq!(service.value, value); // Should be the last registered value
                });
            }
        },
        5 => {
            // Trait registration
            services.add_singleton_trait::<dyn TestTrait>(Arc::new(TestServiceImpl { value }));
            
            if let Ok(provider) = std::panic::catch_unwind(|| services.build()) {
                let _ = std::panic::catch_unwind(|| {
                    let service = provider.get_required_trait::<dyn TestTrait>();
                    assert_eq!(service.get_value(), value);
                });
            }
        },
        _ => unreachable!(),
    }
});

#[derive(Debug, Clone)]
struct TestService {
    value: i32,
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