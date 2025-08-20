/// Unit tests for Key type methods
/// These tests specifically target mutations found by cargo-mutants

use ferrous_di::Key;
use std::any::TypeId;

#[test]
fn test_key_display_name_type() {
    let key = Key::Type(TypeId::of::<String>(), "alloc::string::String");
    assert_eq!(key.display_name(), "alloc::string::String");
    
    // Verify it's not empty or some default value
    assert!(!key.display_name().is_empty());
    assert_ne!(key.display_name(), "");
    assert_ne!(key.display_name(), "xyzzy");
}

#[test]
fn test_key_display_name_trait() {
    let key = Key::Trait("dyn core::fmt::Debug");
    assert_eq!(key.display_name(), "dyn core::fmt::Debug");
    
    assert!(!key.display_name().is_empty());
    assert_ne!(key.display_name(), "");
    assert_ne!(key.display_name(), "xyzzy");
}

#[test]
fn test_key_display_name_multi_trait() {
    let key = Key::MultiTrait("dyn myapp::Plugin", 2);
    assert_eq!(key.display_name(), "dyn myapp::Plugin");
    
    assert!(!key.display_name().is_empty());
    assert_ne!(key.display_name(), "");
    assert_ne!(key.display_name(), "xyzzy");
}

#[test]
fn test_key_display_name_type_named() {
    let key = Key::TypeNamed(TypeId::of::<u32>(), "u32", "database_port");
    assert_eq!(key.display_name(), "u32");
    
    assert!(!key.display_name().is_empty());
    assert_ne!(key.display_name(), "");
    assert_ne!(key.display_name(), "xyzzy");
}

#[test]
fn test_key_display_name_trait_named() {
    let key = Key::TraitNamed("dyn myapp::Logger", "console_logger");
    assert_eq!(key.display_name(), "dyn myapp::Logger");
    
    assert!(!key.display_name().is_empty());
    assert_ne!(key.display_name(), "");
    assert_ne!(key.display_name(), "xyzzy");
}

#[test]
fn test_key_display_name_multi_trait_named() {
    let key = Key::MultiTraitNamed("dyn myapp::Handler", "http_handler", 1);
    assert_eq!(key.display_name(), "dyn myapp::Handler");
    
    assert!(!key.display_name().is_empty());
    assert_ne!(key.display_name(), "");
    assert_ne!(key.display_name(), "xyzzy");
}

// Service name tests - these specifically target the missed mutations

#[test]
fn test_key_service_name_unnamed_types() {
    let key = Key::Type(TypeId::of::<String>(), "alloc::string::String");
    assert_eq!(key.service_name(), None);
    
    // Specifically test it's None, not Some("") or Some("xyzzy")
    assert!(key.service_name().is_none());
    assert_ne!(key.service_name(), Some(""));
    assert_ne!(key.service_name(), Some("xyzzy"));
}

#[test]
fn test_key_service_name_unnamed_trait() {
    let key = Key::Trait("dyn core::fmt::Debug");
    assert_eq!(key.service_name(), None);
    
    assert!(key.service_name().is_none());
    assert_ne!(key.service_name(), Some(""));
    assert_ne!(key.service_name(), Some("xyzzy"));
}

#[test]
fn test_key_service_name_unnamed_multi_trait() {
    let key = Key::MultiTrait("dyn myapp::Plugin", 0);
    assert_eq!(key.service_name(), None);
    
    assert!(key.service_name().is_none());
    assert_ne!(key.service_name(), Some(""));
    assert_ne!(key.service_name(), Some("xyzzy"));
}

#[test]
fn test_key_service_name_named_type() {
    let key = Key::TypeNamed(TypeId::of::<u32>(), "u32", "database_port");
    assert_eq!(key.service_name(), Some("database_port"));
    
    // Verify exact match
    assert_eq!(key.service_name().unwrap(), "database_port");
    assert_ne!(key.service_name(), None);
    assert_ne!(key.service_name(), Some(""));
    assert_ne!(key.service_name(), Some("xyzzy"));
}

#[test]
fn test_key_service_name_named_trait() {
    let key = Key::TraitNamed("dyn myapp::Logger", "console_logger");
    assert_eq!(key.service_name(), Some("console_logger"));
    
    assert_eq!(key.service_name().unwrap(), "console_logger");
    assert_ne!(key.service_name(), None);
    assert_ne!(key.service_name(), Some(""));
    assert_ne!(key.service_name(), Some("xyzzy"));
}

#[test]
fn test_key_service_name_named_multi_trait() {
    let key = Key::MultiTraitNamed("dyn myapp::Handler", "http_handler", 1);
    assert_eq!(key.service_name(), Some("http_handler"));
    
    assert_eq!(key.service_name().unwrap(), "http_handler");
    assert_ne!(key.service_name(), None);
    assert_ne!(key.service_name(), Some(""));
    assert_ne!(key.service_name(), Some("xyzzy"));
}

#[test]
fn test_key_service_name_empty_string() {
    // Test edge case with empty string name
    let key = Key::TypeNamed(TypeId::of::<u32>(), "u32", "");
    assert_eq!(key.service_name(), Some(""));
    
    // Should return Some(""), not None or Some("xyzzy")
    assert!(key.service_name().is_some());
    assert_eq!(key.service_name().unwrap(), "");
    assert_ne!(key.service_name(), None);
    assert_ne!(key.service_name(), Some("xyzzy"));
}

#[test]
fn test_key_debug_format() {
    let key = Key::Type(TypeId::of::<String>(), "alloc::string::String");
    let debug_str = format!("{:?}", key);
    
    // Debug should include the variant name and contents
    assert!(debug_str.contains("Type"));
    assert!(debug_str.contains("alloc::string::String"));
}

#[test]
fn test_key_clone() {
    let key = Key::TypeNamed(TypeId::of::<u32>(), "u32", "test_name");
    let cloned = key.clone();
    
    // Both should have same display name and service name
    assert_eq!(key.display_name(), cloned.display_name());
    assert_eq!(key.service_name(), cloned.service_name());
}

#[test]
fn test_key_equality() {
    let key1 = Key::Type(TypeId::of::<String>(), "alloc::string::String");
    let key2 = Key::Type(TypeId::of::<String>(), "alloc::string::String");
    let key3 = Key::Type(TypeId::of::<u32>(), "u32");
    
    assert_eq!(key1, key2);
    assert_ne!(key1, key3);
}

#[test]
fn test_key_hash() {
    use std::collections::HashMap;
    
    let key = Key::Type(TypeId::of::<String>(), "alloc::string::String");
    let mut map = HashMap::new();
    map.insert(key, "test_value");
    
    let lookup_key = Key::Type(TypeId::of::<String>(), "alloc::string::String");
    assert_eq!(map.get(&lookup_key), Some(&"test_value"));
}