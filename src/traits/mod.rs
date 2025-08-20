//! Core traits for the dependency injection container.

mod dispose;
mod resolver;

pub use dispose::{Dispose, AsyncDispose};
pub use resolver::{Resolver, ResolverCore};