//! Error types for the dependency injection container.

use std::fmt;

/// Dependency injection errors
///
/// Represents the various error conditions that can occur during service
/// registration, resolution, or container operations in ferrous-di.
///
/// # Examples
///
/// ```rust
/// use ferrous_di::{DiError, ServiceCollection, Resolver};
///
/// // Example of NotFound error
/// let provider = ServiceCollection::new().build();
/// match provider.get::<String>() {
///     Err(DiError::NotFound(type_name)) => {
///         assert_eq!(type_name, "alloc::string::String");
///         println!("Service not found: {}", type_name);
///     }
///     _ => unreachable!(),
/// }
/// ```
///
/// ```rust
/// use ferrous_di::DiError;
///
/// // Examples of error types
/// let not_found = DiError::NotFound("MyService");
/// let type_mismatch = DiError::TypeMismatch("std::string::String");
/// let circular = DiError::Circular(vec!["ServiceA", "ServiceB", "ServiceA"]);
/// let wrong_lifetime = DiError::WrongLifetime("Cannot resolve scoped from singleton");
/// let depth_exceeded = DiError::DepthExceeded(100);
///
/// // All errors implement Display
/// println!("Error: {}", not_found);
/// println!("Error: {}", circular);
/// ```
#[derive(Debug, Clone)]
pub enum DiError {
    /// Service not registered
    NotFound(&'static str),
    /// Type downcast failed
    TypeMismatch(&'static str),
    /// Circular dependency detected (includes path)
    Circular(Vec<&'static str>),
    /// Invalid lifetime resolution (e.g., scoped from root)
    WrongLifetime(&'static str),
    /// Maximum recursion depth exceeded
    DepthExceeded(usize),
}

impl fmt::Display for DiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DiError::NotFound(name) => write!(f, "Service not found: {}", name),
            DiError::TypeMismatch(name) => write!(f, "Type mismatch for: {}", name),
            DiError::Circular(path) => {
                write!(f, "Circular dependency: {}", path.join(" -> "))
            }
            DiError::WrongLifetime(msg) => write!(f, "Lifetime error: {}", msg),
            DiError::DepthExceeded(depth) => write!(f, "Max depth {} exceeded", depth),
        }
    }
}

impl std::error::Error for DiError {}

/// Result type for DI operations
///
/// A convenience type alias for `Result<T, DiError>` used throughout ferrous-di.
/// This follows the common Rust pattern of having a crate-specific Result type
/// to reduce boilerplate in function signatures.
///
/// # Examples
///
/// ```rust
/// use ferrous_di::{DiResult, DiError};
///
/// fn create_service() -> DiResult<String> {
///     Ok("service created".to_string())
/// }
///
/// fn failing_operation() -> DiResult<()> {
///     Err(DiError::NotFound("some_service"))
/// }
///
/// // Usage
/// match create_service() {
///     Ok(service) => println!("Success: {}", service),
///     Err(e) => eprintln!("Error: {}", e),
/// }
/// ```
pub type DiResult<T> = Result<T, DiError>;