/// Unit tests for DiError and DiResult types
/// These tests specifically target mutations found by cargo-mutants

use ferrous_di::{DiError, DiResult};
use std::error::Error;

#[test]
fn test_error_display_not_found() {
    let error = DiError::NotFound("TestService");
    let display_str = format!("{}", error);
    assert_eq!(display_str, "Service not found: TestService");
    
    // Verify it's not an empty string or default
    assert!(!display_str.is_empty());
    assert!(display_str.contains("TestService"));
    assert!(display_str.contains("not found"));
}

#[test]
fn test_error_display_type_mismatch() {
    let error = DiError::TypeMismatch("std::string::String");
    let display_str = format!("{}", error);
    assert_eq!(display_str, "Type mismatch for: std::string::String");
    
    // Verify specific content
    assert!(display_str.contains("std::string::String"));
    assert!(display_str.contains("mismatch"));
}

#[test]
fn test_error_display_circular() {
    let path = vec!["ServiceA", "ServiceB", "ServiceA"];
    let error = DiError::Circular(path);
    let display_str = format!("{}", error);
    assert_eq!(display_str, "Circular dependency: ServiceA -> ServiceB -> ServiceA");
    
    // Verify the path is joined correctly
    assert!(display_str.contains("ServiceA -> ServiceB -> ServiceA"));
    assert!(display_str.contains("Circular dependency"));
}

#[test]
fn test_error_display_wrong_lifetime() {
    let error = DiError::WrongLifetime("Cannot resolve scoped from singleton context");
    let display_str = format!("{}", error);
    assert_eq!(display_str, "Lifetime error: Cannot resolve scoped from singleton context");
    
    assert!(display_str.contains("Lifetime error"));
    assert!(display_str.contains("scoped from singleton"));
}

#[test]
fn test_error_display_depth_exceeded() {
    let error = DiError::DepthExceeded(100);
    let display_str = format!("{}", error);
    assert_eq!(display_str, "Max depth 100 exceeded");
    
    assert!(display_str.contains("100"));
    assert!(display_str.contains("exceeded"));
}

#[test]
fn test_error_display_empty_circular_path() {
    let error = DiError::Circular(vec![]);
    let display_str = format!("{}", error);
    assert_eq!(display_str, "Circular dependency: ");
    
    // Should still show the prefix even with empty path
    assert!(display_str.contains("Circular dependency"));
}

#[test]
fn test_diresult_ok() {
    let result: DiResult<String> = Ok("success".to_string());
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "success");
}

#[test]
fn test_diresult_err() {
    let result: DiResult<String> = Err(DiError::NotFound("TestService"));
    assert!(result.is_err());
    
    match result {
        Err(DiError::NotFound(name)) => assert_eq!(name, "TestService"),
        _ => panic!("Expected NotFound error"),
    }
}

#[test]
fn test_error_debug_format() {
    let error = DiError::NotFound("TestService");
    let debug_str = format!("{:?}", error);
    
    // Debug format should contain the type name and field
    assert!(debug_str.contains("NotFound"));
    assert!(debug_str.contains("TestService"));
}

#[test]
fn test_error_clone() {
    let error = DiError::TypeMismatch("SomeType");
    let cloned = error.clone();
    
    // Both should format the same way
    assert_eq!(format!("{}", error), format!("{}", cloned));
}

#[test]
fn test_error_as_std_error() {
    let error = DiError::NotFound("TestService");
    
    // Should implement std::error::Error
    let _: &dyn std::error::Error = &error;
    
    // Should have a source (None in our case)
    assert!(error.source().is_none());
}