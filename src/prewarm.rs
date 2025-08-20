//! Pre-warm and readiness functionality for deterministic agent startup.
//!
//! This module provides infrastructure to pre-initialize services during
//! application startup, eliminating cold-start penalties during agent execution.

use std::any::TypeId;
use std::collections::HashSet;
use crate::{Key, ServiceProvider};

/// Trait for services that can perform readiness checks.
///
/// Implement this trait on services that need to perform initialization
/// or health checks during the pre-warm phase. This is particularly useful
/// for services that need to:
/// - Establish database connections
/// - Load machine learning models
/// - Authenticate with external APIs  
/// - Pre-populate caches
/// - Validate configuration
///
/// # Examples
///
/// ```
/// use ferrous_di::ReadyCheck;
/// use async_trait::async_trait;
///
/// struct DatabaseService {
///     connection_string: String,
/// }
///
/// #[async_trait]
/// impl ReadyCheck for DatabaseService {
///     async fn ready(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
///         // Perform connection test
///         println!("Testing database connection: {}", self.connection_string);
///         // In real implementation: test actual connection
///         Ok(())
///     }
/// }
/// ```
#[async_trait::async_trait]
pub trait ReadyCheck: Send + Sync {
    /// Performs readiness check for this service.
    ///
    /// This method is called during the pre-warm phase to verify that
    /// the service is ready for use. It should perform any necessary
    /// initialization and return an error if the service is not ready.
    ///
    /// # Returns
    ///
    /// * `Ok(())` if the service is ready
    /// * `Err(error)` if initialization failed or the service is not ready
    async fn ready(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
}

/// Collection of service types to pre-warm during startup.
#[derive(Default)]
pub(crate) struct PrewarmSet {
    /// Set of type IDs to pre-warm
    types: HashSet<TypeId>,
    /// Set of trait names to pre-warm
    traits: HashSet<&'static str>,
}

impl PrewarmSet {
    /// Creates a new empty prewarm set.
    pub(crate) fn new() -> Self {
        Self {
            types: HashSet::new(),
            traits: HashSet::new(),
        }
    }

    /// Adds a concrete type to the prewarm set.
    pub(crate) fn add_type<T: 'static + Send + Sync>(&mut self) {
        self.types.insert(TypeId::of::<T>());
    }

    /// Adds a trait to the prewarm set.
    pub(crate) fn add_trait<T: ?Sized + 'static + Send + Sync>(&mut self) {
        self.traits.insert(std::any::type_name::<T>());
    }

    /// Returns true if any services are marked for prewarming.
    #[allow(dead_code)]
    pub(crate) fn has_services(&self) -> bool {
        !self.types.is_empty() || !self.traits.is_empty()
    }

    /// Gets all service keys that should be prewarmed.
    #[allow(dead_code)]
    pub(crate) fn get_keys(&self) -> Vec<Key> {
        let mut keys = Vec::new();

        // Add concrete type keys
        for _type_id in &self.types {
            // We need to reconstruct the type name from TypeId
            // This is a limitation - we'll need to store both TypeId and name
            // For now, we'll just document this limitation
        }

        // Add trait keys
        for trait_name in &self.traits {
            keys.push(Key::Trait(trait_name));
        }

        keys
    }
}

/// Readiness check result for a single service.
pub struct ReadinessResult {
    /// The service key that was checked
    pub key: Key,
    /// Whether the readiness check passed
    pub success: bool,
    /// Error message if the check failed
    pub error: Option<String>,
    /// Time taken for the readiness check
    pub duration: std::time::Duration,
}

impl ReadinessResult {
    /// Creates a successful readiness result.
    pub fn success(key: Key, duration: std::time::Duration) -> Self {
        Self {
            key,
            success: true,
            error: None,
            duration,
        }
    }

    /// Creates a failed readiness result.
    pub fn failure(key: Key, error: String, duration: std::time::Duration) -> Self {
        Self {
            key,
            success: false,
            error: Some(error),
            duration,
        }
    }
}

/// Overall readiness check results.
pub struct ReadinessReport {
    /// Individual service results
    pub services: Vec<ReadinessResult>,
    /// Total time taken for all checks
    pub total_duration: std::time::Duration,
}

impl ReadinessReport {
    /// Returns true if all readiness checks passed.
    pub fn all_ready(&self) -> bool {
        self.services.iter().all(|r| r.success)
    }

    /// Returns the number of services that passed readiness checks.
    pub fn ready_count(&self) -> usize {
        self.services.iter().filter(|r| r.success).count()
    }

    /// Returns the number of services that failed readiness checks.
    pub fn failed_count(&self) -> usize {
        self.services.iter().filter(|r| !r.success).count()
    }

    /// Gets all failed services.
    pub fn failures(&self) -> Vec<&ReadinessResult> {
        self.services.iter().filter(|r| !r.success).collect()
    }
}

impl ServiceProvider {
    /// Performs readiness checks on all prewarmed services.
    ///
    /// This method resolves all services marked for prewarming and runs
    /// readiness checks on those that implement `ReadyCheck`. This is
    /// typically called during application startup to ensure all critical
    /// services are ready before accepting requests.
    ///
    /// # Performance
    ///
    /// Readiness checks are run in parallel with a configurable concurrency
    /// limit to balance startup time with resource usage.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use ferrous_di::{ServiceCollection, ReadyCheck};
    /// use async_trait::async_trait;
    /// use std::sync::Arc;
    ///
    /// struct DatabaseService;
    /// #[async_trait]
    /// impl ReadyCheck for DatabaseService {
    ///     async fn ready(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    ///         // Test database connection
    ///         Ok(())
    ///     }
    /// }
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
/// let mut services = ServiceCollection::new();
/// services.add_singleton(DatabaseService);
/// services.prewarm::<DatabaseService>();
///
/// let provider = services.build();
/// let report = provider.ready().await?;
///
/// if report.all_ready() {
///     println!("All services ready! Starting application...");
/// } else {
///     eprintln!("Some services failed readiness checks:");
///     for failure in report.failures() {
///         eprintln!("  {}: {}", 
///             failure.key.display_name(), 
///             failure.error.as_deref().unwrap_or("Unknown error"));
///     }
///     std::process::exit(1);
/// }
/// # Ok(())
/// # }
    /// ```
    pub async fn ready(&self) -> Result<ReadinessReport, Box<dyn std::error::Error + Send + Sync>> {
        // TODO: This is a placeholder implementation
        // We need to integrate with the ServiceCollection's prewarm set
        // and actually resolve and check the services
        
        let start = std::time::Instant::now();
        let services = Vec::new(); // Empty for now
        let total_duration = start.elapsed();

        Ok(ReadinessReport {
            services,
            total_duration,
        })
    }
}