/// Unit tests for ServiceDescriptor methods
/// These tests specifically target mutations found by cargo-mutants

use ferrous_di::{ServiceDescriptor, Key, Lifetime};
use std::any::TypeId;

#[test]
fn test_service_descriptor_service_name_unnamed() {
    let descriptor = ServiceDescriptor {
        key: Key::Type(TypeId::of::<String>(), "alloc::string::String"),
        lifetime: Lifetime::Singleton,
        impl_type_id: None,
        impl_type_name: None,
        has_metadata: false,
    };
    
    assert_eq!(descriptor.service_name(), None);
    
    // Specifically test against mutation values
    assert!(descriptor.service_name().is_none());
    assert_ne!(descriptor.service_name(), Some(""));
    assert_ne!(descriptor.service_name(), Some("xyzzy"));
}

#[test]
fn test_service_descriptor_service_name_named() {
    let descriptor = ServiceDescriptor {
        key: Key::TypeNamed(TypeId::of::<u32>(), "u32", "database_port"),
        lifetime: Lifetime::Singleton,
        impl_type_id: Some(TypeId::of::<u32>()),
        impl_type_name: Some("u32"),
        has_metadata: false,
    };
    
    assert_eq!(descriptor.service_name(), Some("database_port"));
    
    // Test exact value
    assert_eq!(descriptor.service_name().unwrap(), "database_port");
    assert_ne!(descriptor.service_name(), None);
    assert_ne!(descriptor.service_name(), Some(""));
    assert_ne!(descriptor.service_name(), Some("xyzzy"));
}

#[test]
fn test_service_descriptor_service_name_empty_string() {
    let descriptor = ServiceDescriptor {
        key: Key::TypeNamed(TypeId::of::<u32>(), "u32", ""),
        lifetime: Lifetime::Singleton,
        impl_type_id: None,
        impl_type_name: None,
        has_metadata: false,
    };
    
    assert_eq!(descriptor.service_name(), Some(""));
    
    // Should be Some(""), not None or Some("xyzzy")
    assert!(descriptor.service_name().is_some());
    assert_eq!(descriptor.service_name().unwrap(), "");
    assert_ne!(descriptor.service_name(), None);
    assert_ne!(descriptor.service_name(), Some("xyzzy"));
}

#[test]
fn test_service_descriptor_type_name() {
    let descriptor = ServiceDescriptor {
        key: Key::Type(TypeId::of::<String>(), "alloc::string::String"),
        lifetime: Lifetime::Singleton,
        impl_type_id: Some(TypeId::of::<String>()),
        impl_type_name: Some("alloc::string::String"),
        has_metadata: false,
    };
    
    assert_eq!(descriptor.type_name(), "alloc::string::String");
    
    // Test against mutation values
    assert!(!descriptor.type_name().is_empty());
    assert_ne!(descriptor.type_name(), "");
    assert_ne!(descriptor.type_name(), "xyzzy");
}

#[test]
fn test_service_descriptor_type_name_trait() {
    let descriptor = ServiceDescriptor {
        key: Key::Trait("dyn core::fmt::Debug"),
        lifetime: Lifetime::Scoped,
        impl_type_id: None,
        impl_type_name: Some("MyDebugImpl"),
        has_metadata: false,
    };
    
    assert_eq!(descriptor.type_name(), "dyn core::fmt::Debug");
    
    assert!(!descriptor.type_name().is_empty());
    assert_ne!(descriptor.type_name(), "");
    assert_ne!(descriptor.type_name(), "xyzzy");
}

#[test]
fn test_service_descriptor_type_name_named() {
    let descriptor = ServiceDescriptor {
        key: Key::TypeNamed(TypeId::of::<u32>(), "u32", "port"),
        lifetime: Lifetime::Singleton,
        impl_type_id: Some(TypeId::of::<u32>()),
        impl_type_name: Some("u32"),
        has_metadata: false,
    };
    
    assert_eq!(descriptor.type_name(), "u32");
    
    assert!(!descriptor.type_name().is_empty());
    assert_ne!(descriptor.type_name(), "");
    assert_ne!(descriptor.type_name(), "xyzzy");
}

#[test]
fn test_service_descriptor_is_named_false() {
    let descriptor = ServiceDescriptor {
        key: Key::Type(TypeId::of::<String>(), "alloc::string::String"),
        lifetime: Lifetime::Singleton,
        impl_type_id: None,
        impl_type_name: None,
        has_metadata: false,
    };
    
    assert_eq!(descriptor.is_named(), false);
    
    // Specifically test against mutation - should not be true
    assert!(!descriptor.is_named());
    assert_ne!(descriptor.is_named(), true);
}

#[test]
fn test_service_descriptor_is_named_true() {
    let descriptor = ServiceDescriptor {
        key: Key::TypeNamed(TypeId::of::<u32>(), "u32", "database_port"),
        lifetime: Lifetime::Singleton,
        impl_type_id: None,
        impl_type_name: None,
        has_metadata: false,
    };
    
    assert_eq!(descriptor.is_named(), true);
    
    // Specifically test against mutation - should not be false
    assert!(descriptor.is_named());
    assert_ne!(descriptor.is_named(), false);
}

#[test]
fn test_service_descriptor_is_named_trait() {
    let unnamed_trait = ServiceDescriptor {
        key: Key::Trait("dyn core::fmt::Debug"),
        lifetime: Lifetime::Scoped,
        impl_type_id: None,
        impl_type_name: None,
        has_metadata: false,
    };
    
    let named_trait = ServiceDescriptor {
        key: Key::TraitNamed("dyn core::fmt::Debug", "console_debug"),
        lifetime: Lifetime::Scoped,
        impl_type_id: None,
        impl_type_name: None,
        has_metadata: false,
    };
    
    assert!(!unnamed_trait.is_named());
    assert!(named_trait.is_named());
    
    // Test opposites
    assert_ne!(unnamed_trait.is_named(), true);
    assert_ne!(named_trait.is_named(), false);
}

#[test]
fn test_service_descriptor_is_named_multi_trait() {
    let unnamed_multi = ServiceDescriptor {
        key: Key::MultiTrait("dyn myapp::Plugin", 0),
        lifetime: Lifetime::Singleton,
        impl_type_id: None,
        impl_type_name: None,
        has_metadata: false,
    };
    
    let named_multi = ServiceDescriptor {
        key: Key::MultiTraitNamed("dyn myapp::Plugin", "auth_plugin", 0),
        lifetime: Lifetime::Singleton,
        impl_type_id: None,
        impl_type_name: None,
        has_metadata: false,
    };
    
    assert!(!unnamed_multi.is_named());
    assert!(named_multi.is_named());
    
    assert_ne!(unnamed_multi.is_named(), true);
    assert_ne!(named_multi.is_named(), false);
}

#[test]
fn test_service_descriptor_debug() {
    let descriptor = ServiceDescriptor {
        key: Key::Type(TypeId::of::<String>(), "alloc::string::String"),
        lifetime: Lifetime::Singleton,
        impl_type_id: Some(TypeId::of::<String>()),
        impl_type_name: Some("alloc::string::String"),
        has_metadata: true,
    };
    
    let debug_str = format!("{:?}", descriptor);
    
    // Debug should include the struct name and key fields
    assert!(debug_str.contains("ServiceDescriptor"));
    assert!(debug_str.contains("key"));
    assert!(debug_str.contains("lifetime"));
}

#[test]
fn test_service_descriptor_clone() {
    let descriptor = ServiceDescriptor {
        key: Key::TypeNamed(TypeId::of::<u32>(), "u32", "test_port"),
        lifetime: Lifetime::Scoped,
        impl_type_id: Some(TypeId::of::<u32>()),
        impl_type_name: Some("u32"),
        has_metadata: false,
    };
    
    let cloned = descriptor.clone();
    
    // All methods should return the same values
    assert_eq!(descriptor.service_name(), cloned.service_name());
    assert_eq!(descriptor.type_name(), cloned.type_name());
    assert_eq!(descriptor.is_named(), cloned.is_named());
    assert_eq!(descriptor.lifetime, cloned.lifetime);
    assert_eq!(descriptor.has_metadata, cloned.has_metadata);
}

#[test]
fn test_service_descriptor_all_lifetimes() {
    let singleton = ServiceDescriptor {
        key: Key::Type(TypeId::of::<String>(), "alloc::string::String"),
        lifetime: Lifetime::Singleton,
        impl_type_id: None,
        impl_type_name: None,
        has_metadata: false,
    };
    
    let scoped = ServiceDescriptor {
        key: Key::Type(TypeId::of::<String>(), "alloc::string::String"),
        lifetime: Lifetime::Scoped,
        impl_type_id: None,
        impl_type_name: None,
        has_metadata: false,
    };
    
    let transient = ServiceDescriptor {
        key: Key::Type(TypeId::of::<String>(), "alloc::string::String"),
        lifetime: Lifetime::Transient,
        impl_type_id: None,
        impl_type_name: None,
        has_metadata: false,
    };
    
    // All should have same key methods but different lifetimes
    assert_eq!(singleton.type_name(), scoped.type_name());
    assert_eq!(scoped.type_name(), transient.type_name());
    assert_eq!(singleton.is_named(), scoped.is_named());
    assert_eq!(scoped.is_named(), transient.is_named());
    
    assert_ne!(singleton.lifetime, scoped.lifetime);
    assert_ne!(scoped.lifetime, transient.lifetime);
    assert_ne!(singleton.lifetime, transient.lifetime);
}

#[test]
fn test_service_descriptor_metadata_flag() {
    let without_metadata = ServiceDescriptor {
        key: Key::Type(TypeId::of::<String>(), "alloc::string::String"),
        lifetime: Lifetime::Singleton,
        impl_type_id: None,
        impl_type_name: None,
        has_metadata: false,
    };
    
    let with_metadata = ServiceDescriptor {
        key: Key::Type(TypeId::of::<String>(), "alloc::string::String"),
        lifetime: Lifetime::Singleton,
        impl_type_id: None,
        impl_type_name: None,
        has_metadata: true,
    };
    
    assert!(!without_metadata.has_metadata);
    assert!(with_metadata.has_metadata);
}