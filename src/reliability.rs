//! Reliability patterns for ferrous-di.
//!
//! This module provides circuit breaker patterns, retry mechanisms,
//! and graceful degradation for robust service resolution.

use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};
use std::collections::HashMap;
use crate::{Key, DiResult, DiError};

/// Circuit breaker for protecting against cascading failures
#[derive(Debug)]
pub struct CircuitBreaker {
    /// Circuit breaker state
    state: RwLock<CircuitState>,
    /// Configuration settings
    config: CircuitBreakerConfig,
    /// Failure tracking metrics
    metrics: Mutex<CircuitMetrics>,
}

#[derive(Debug, Clone)]
pub struct CircuitBreakerConfig {
    /// Number of failures before opening the circuit
    pub failure_threshold: u32,
    /// Success threshold to close the circuit from half-open
    pub success_threshold: u32,
    /// Duration to wait before transitioning from open to half-open
    pub timeout: Duration,
    /// Window for tracking failures
    pub failure_window: Duration,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            success_threshold: 3,
            timeout: Duration::from_secs(60),
            failure_window: Duration::from_secs(120),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
enum CircuitState {
    Closed,
    Open(Instant),
    HalfOpen,
}

#[derive(Debug, Default)]
struct CircuitMetrics {
    /// Recent failure timestamps
    failures: Vec<Instant>,
    /// Consecutive successes in half-open state
    consecutive_successes: u32,
    /// Total operations attempted
    total_attempts: u64,
    /// Total failures recorded
    total_failures: u64,
}

impl CircuitBreaker {
    /// Create a new circuit breaker with default configuration
    pub fn new() -> Self {
        Self::with_config(CircuitBreakerConfig::default())
    }

    /// Create a new circuit breaker with custom configuration
    pub fn with_config(config: CircuitBreakerConfig) -> Self {
        Self {
            state: RwLock::new(CircuitState::Closed),
            config,
            metrics: Mutex::new(CircuitMetrics::default()),
        }
    }

    /// Execute a function with circuit breaker protection
    pub fn call<T, F>(&self, operation: F) -> DiResult<T>
    where
        F: FnOnce() -> DiResult<T>,
    {
        // Check if circuit is open
        if self.is_open() {
            return Err(DiError::TypeMismatch("Circuit breaker is open"));
        }

        // Update metrics
        if let Ok(mut metrics) = self.metrics.lock() {
            metrics.total_attempts += 1;
        }

        // Execute the operation
        let result = operation();

        // Update circuit state based on result
        match result {
            Ok(value) => {
                self.on_success();
                Ok(value)
            }
            Err(error) => {
                self.on_failure();
                Err(error)
            }
        }
    }

    /// Check if the circuit is currently open
    pub fn is_open(&self) -> bool {
        if let Ok(state) = self.state.read() {
            match *state {
                CircuitState::Open(opened_at) => {
                    if opened_at.elapsed() >= self.config.timeout {
                        // Transition to half-open
                        drop(state);
                        if let Ok(mut write_state) = self.state.write() {
                            *write_state = CircuitState::HalfOpen;
                        }
                        false // Allow one attempt in half-open state
                    } else {
                        true // Still open
                    }
                }
                CircuitState::HalfOpen | CircuitState::Closed => false,
            }
        } else {
            false // Default to closed if we can't read state
        }
    }

    /// Handle successful operation
    fn on_success(&self) {
        if let Ok(state) = self.state.read() {
            match *state {
                CircuitState::HalfOpen => {
                    // Track consecutive successes in half-open state
                    if let Ok(mut metrics) = self.metrics.lock() {
                        metrics.consecutive_successes += 1;
                        
                        if metrics.consecutive_successes >= self.config.success_threshold {
                            // Transition to closed
                            drop(state);
                            if let Ok(mut write_state) = self.state.write() {
                                *write_state = CircuitState::Closed;
                            }
                            metrics.consecutive_successes = 0;
                            metrics.failures.clear();
                        }
                    }
                }
                CircuitState::Closed => {
                    // Clear old failures on success
                    if let Ok(mut metrics) = self.metrics.lock() {
                        let cutoff = Instant::now() - self.config.failure_window;
                        metrics.failures.retain(|&failure_time| failure_time > cutoff);
                    }
                }
                CircuitState::Open(_) => {
                    // This shouldn't happen as we check is_open() before calling
                }
            }
        }
    }

    /// Handle failed operation
    fn on_failure(&self) {
        if let Ok(mut metrics) = self.metrics.lock() {
            metrics.total_failures += 1;
            metrics.failures.push(Instant::now());
            metrics.consecutive_successes = 0;

            // Clean up old failures outside the window
            let cutoff = Instant::now() - self.config.failure_window;
            metrics.failures.retain(|&failure_time| failure_time > cutoff);

            // Check if we should open the circuit
            if metrics.failures.len() as u32 >= self.config.failure_threshold {
                drop(metrics);
                if let Ok(mut state) = self.state.write() {
                    *state = CircuitState::Open(Instant::now());
                }
            }
        }
    }

    /// Get current circuit breaker statistics
    pub fn stats(&self) -> CircuitBreakerStats {
        let state = if let Ok(state) = self.state.read() {
            match *state {
                CircuitState::Closed => "Closed".to_string(),
                CircuitState::Open(opened_at) => format!("Open ({}s ago)", opened_at.elapsed().as_secs()),
                CircuitState::HalfOpen => "Half-Open".to_string(),
            }
        } else {
            "Unknown".to_string()
        };

        let metrics = self.metrics.lock().unwrap();
        CircuitBreakerStats {
            state,
            total_attempts: metrics.total_attempts,
            total_failures: metrics.total_failures,
            recent_failures: metrics.failures.len(),
            consecutive_successes: metrics.consecutive_successes,
            failure_rate: if metrics.total_attempts > 0 {
                (metrics.total_failures as f64 / metrics.total_attempts as f64) * 100.0
            } else {
                0.0
            },
        }
    }
}

#[derive(Debug, Clone)]
pub struct CircuitBreakerStats {
    pub state: String,
    pub total_attempts: u64,
    pub total_failures: u64,
    pub recent_failures: usize,
    pub consecutive_successes: u32,
    pub failure_rate: f64,
}

impl Default for CircuitBreaker {
    fn default() -> Self {
        Self::new()
    }
}

/// Retry mechanism with exponential backoff
#[derive(Debug)]
pub struct RetryPolicy {
    /// Maximum number of retry attempts
    pub max_attempts: u32,
    /// Initial delay between retries
    pub initial_delay: Duration,
    /// Maximum delay between retries
    pub max_delay: Duration,
    /// Backoff multiplier
    pub backoff_multiplier: f64,
    /// Jitter to add to delays (prevents thundering herd)
    pub jitter: bool,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            initial_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(30),
            backoff_multiplier: 2.0,
            jitter: true,
        }
    }
}

impl RetryPolicy {
    /// Execute a function with retry logic
    pub fn execute<T, F>(&self, mut operation: F) -> DiResult<T>
    where
        F: FnMut() -> DiResult<T>,
    {
        let mut attempt = 0;
        let mut last_error: Option<DiError> = None;

        loop {
            attempt += 1;

            match operation() {
                Ok(result) => return Ok(result),
                Err(error) => {
                    last_error = Some(error);
                    
                    if attempt >= self.max_attempts {
                        break;
                    }

                    // Calculate delay for next attempt
                    let delay = self.calculate_delay(attempt - 1);
                    std::thread::sleep(delay);
                }
            }
        }

        // Return the last error if all retries failed
        Err(last_error.unwrap_or_else(|| DiError::TypeMismatch("All retries exhausted")))
    }

    /// Calculate delay for a given attempt number
    fn calculate_delay(&self, attempt: u32) -> Duration {
        let base_delay = self.initial_delay.as_millis() as f64;
        let exponential_delay = base_delay * self.backoff_multiplier.powi(attempt as i32);
        let capped_delay = exponential_delay.min(self.max_delay.as_millis() as f64);

        let final_delay = if self.jitter {
            let jitter_factor = 0.1; // 10% jitter
            let jitter = (rand::random::<f64>() - 0.5) * 2.0 * jitter_factor * capped_delay;
            (capped_delay + jitter).max(0.0)
        } else {
            capped_delay
        };

        Duration::from_millis(final_delay as u64)
    }
}

// Simple random number generator to avoid dependency
mod rand {
    use std::sync::atomic::{AtomicU32, Ordering};

    static SEED: AtomicU32 = AtomicU32::new(1);

    pub fn random<T: From<u32>>() -> T {
        // Simple LCG for basic randomness
        let current = SEED.load(Ordering::Relaxed);
        let next = current.wrapping_mul(1103515245).wrapping_add(12345);
        SEED.store(next, Ordering::Relaxed);
        T::from(next)
    }
}

/// Fallback service provider for graceful degradation
pub struct FallbackProvider {
    /// Primary service keys to fallback services
    fallbacks: RwLock<HashMap<Key, Key>>,
    /// Default fallback factories
    default_factories: RwLock<HashMap<Key, Box<dyn Fn() -> crate::AnyArc + Send + Sync>>>,
}

impl std::fmt::Debug for FallbackProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FallbackProvider")
            .field("fallbacks", &self.fallbacks.read().unwrap().len())
            .field("default_factories", &self.default_factories.read().unwrap().len())
            .finish()
    }
}

impl FallbackProvider {
    /// Create a new fallback provider
    pub fn new() -> Self {
        Self {
            fallbacks: RwLock::new(HashMap::new()),
            default_factories: RwLock::new(HashMap::new()),
        }
    }

    /// Register a fallback service for a primary service
    pub fn register_fallback(&self, primary: Key, fallback: Key) {
        if let Ok(mut fallbacks) = self.fallbacks.write() {
            fallbacks.insert(primary, fallback);
        }
    }

    /// Register a default factory for a service
    pub fn register_default_factory<T, F>(&self, key: Key, factory: F)
    where
        T: Send + Sync + 'static,
        F: Fn() -> T + Send + Sync + 'static,
    {
        let boxed_factory = Box::new(move || {
            Arc::new(factory()) as crate::AnyArc
        });

        if let Ok(mut factories) = self.default_factories.write() {
            factories.insert(key, boxed_factory);
        }
    }

    /// Get a fallback service key
    pub fn get_fallback(&self, primary: &Key) -> Option<Key> {
        self.fallbacks.read().ok()?.get(primary).cloned()
    }

    /// Create a default service instance
    pub fn create_default(&self, key: &Key) -> Option<crate::AnyArc> {
        let factories = self.default_factories.read().ok()?;
        let factory = factories.get(key)?;
        Some(factory())
    }
}

impl Default for FallbackProvider {
    fn default() -> Self {
        Self::new()
    }
}

/// Reliability coordinator that combines circuit breakers, retries, and fallbacks
#[derive(Debug)]
pub struct ReliabilityCoordinator {
    /// Circuit breakers per service key
    circuit_breakers: RwLock<HashMap<Key, Arc<CircuitBreaker>>>,
    /// Default circuit breaker configuration
    default_cb_config: CircuitBreakerConfig,
    /// Default retry policy
    default_retry_policy: RetryPolicy,
    /// Fallback provider
    fallback_provider: Arc<FallbackProvider>,
}

impl ReliabilityCoordinator {
    /// Create a new reliability coordinator
    pub fn new() -> Self {
        Self {
            circuit_breakers: RwLock::new(HashMap::new()),
            default_cb_config: CircuitBreakerConfig::default(),
            default_retry_policy: RetryPolicy::default(),
            fallback_provider: Arc::new(FallbackProvider::new()),
        }
    }

    /// Execute service resolution with full reliability protection
    pub fn execute_with_protection<T, F>(&self, key: &Key, operation: F) -> DiResult<T>
    where
        F: Fn() -> DiResult<T> + Clone,
        T: 'static,
    {
        // Get or create circuit breaker for this service
        let circuit_breaker = self.get_or_create_circuit_breaker(key);

        // Execute with circuit breaker and retry protection
        let result = self.default_retry_policy.execute(|| {
            circuit_breaker.call(|| operation())
        });

        // If all else fails, try fallback
        if result.is_err() {
            if let Some(fallback_key) = self.fallback_provider.get_fallback(key) {
                // Try to resolve fallback service
                // Note: In a real implementation, this would involve the actual service provider
                return self.execute_with_protection(&fallback_key, operation);
            }

            // Try default factory as last resort
            if let Some(_default_service) = self.fallback_provider.create_default(key) {
                // Return the default service
                // Note: This is simplified - real implementation would need proper type handling
            }
        }

        result
    }

    /// Get or create a circuit breaker for a service key
    fn get_or_create_circuit_breaker(&self, key: &Key) -> Arc<CircuitBreaker> {
        // Fast path: check if circuit breaker exists
        if let Ok(breakers) = self.circuit_breakers.read() {
            if let Some(breaker) = breakers.get(key) {
                return breaker.clone();
            }
        }

        // Slow path: create new circuit breaker
        let new_breaker = Arc::new(CircuitBreaker::with_config(self.default_cb_config.clone()));
        
        if let Ok(mut breakers) = self.circuit_breakers.write() {
            // Double-checked locking
            if let Some(existing) = breakers.get(key) {
                existing.clone()
            } else {
                breakers.insert(key.clone(), new_breaker.clone());
                new_breaker
            }
        } else {
            new_breaker
        }
    }

    /// Get statistics for all circuit breakers
    pub fn get_all_circuit_stats(&self) -> HashMap<Key, CircuitBreakerStats> {
        if let Ok(breakers) = self.circuit_breakers.read() {
            breakers
                .iter()
                .map(|(key, breaker)| (key.clone(), breaker.stats()))
                .collect()
        } else {
            HashMap::new()
        }
    }

    /// Get the fallback provider for manual fallback configuration
    pub fn fallback_provider(&self) -> Arc<FallbackProvider> {
        self.fallback_provider.clone()
    }
}

impl Default for ReliabilityCoordinator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::any::TypeId;
    use std::sync::atomic::{AtomicU32, Ordering};

    #[test]
    fn test_circuit_breaker_open_close() {
        let config = CircuitBreakerConfig {
            failure_threshold: 2,
            success_threshold: 1,
            timeout: Duration::from_millis(100),
            failure_window: Duration::from_secs(60),
        };

        let circuit = CircuitBreaker::with_config(config);
        
        // Initially closed
        assert!(!circuit.is_open());

        // Trigger failures to open circuit
        let _ = circuit.call(|| -> DiResult<()> {
            Err(DiError::NotFound("test"))
        });
        let _ = circuit.call(|| -> DiResult<()> {
            Err(DiError::NotFound("test"))
        });

        // Should be open now
        assert!(circuit.is_open());

        // Wait for timeout and try success
        std::thread::sleep(Duration::from_millis(150));
        
        let result = circuit.call(|| -> DiResult<()> { Ok(()) });
        assert!(result.is_ok());
        
        // Should be closed again
        assert!(!circuit.is_open());
    }

    #[test]
    fn test_retry_policy() {
        let policy = RetryPolicy {
            max_attempts: 3,
            initial_delay: Duration::from_millis(1),
            max_delay: Duration::from_millis(10),
            backoff_multiplier: 2.0,
            jitter: false,
        };

        let counter = Arc::new(AtomicU32::new(0));
        let counter_clone = counter.clone();

        // Should succeed on third attempt
        let result = policy.execute(|| {
            let count = counter_clone.fetch_add(1, Ordering::SeqCst);
            if count < 2 {
                Err(DiError::NotFound("test"))
            } else {
                Ok("success")
            }
        });

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "success");
        assert_eq!(counter.load(Ordering::SeqCst), 3);
    }

    #[test]
    fn test_fallback_provider() {
        let provider = FallbackProvider::new();
        let primary_key = Key::Type(TypeId::of::<String>(), "Primary");
        let fallback_key = Key::Type(TypeId::of::<String>(), "Fallback");

        provider.register_fallback(primary_key.clone(), fallback_key.clone());
        
        let retrieved_fallback = provider.get_fallback(&primary_key);
        assert_eq!(retrieved_fallback, Some(fallback_key));
    }

    #[test]
    fn test_circuit_breaker_stats() {
        let circuit = CircuitBreaker::new();
        
        // Successful call
        let _ = circuit.call(|| -> DiResult<()> { Ok(()) });
        
        // Failed call
        let _ = circuit.call(|| -> DiResult<()> {
            Err(DiError::NotFound("test"))
        });

        let stats = circuit.stats();
        assert_eq!(stats.total_attempts, 2);
        assert_eq!(stats.total_failures, 1);
        assert_eq!(stats.failure_rate, 50.0);
    }

    #[test]
    fn test_reliability_coordinator() {
        let coordinator = ReliabilityCoordinator::new();
        let key = Key::Type(TypeId::of::<String>(), "TestService");

        // Register a fallback
        let fallback_key = Key::Type(TypeId::of::<String>(), "FallbackService");
        coordinator.fallback_provider().register_fallback(key.clone(), fallback_key);

        // Test that circuit breakers are created and cached
        let breaker1 = coordinator.get_or_create_circuit_breaker(&key);
        let breaker2 = coordinator.get_or_create_circuit_breaker(&key);
        
        // Should be the same instance (Arc equality)
        assert!(Arc::ptr_eq(&breaker1, &breaker2));
    }
}