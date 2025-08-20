//! Metrics collection and monitoring for ferrous-di performance.
//!
//! This module provides instrumentation for tracking service resolution
//! performance, cache effectiveness, and system health metrics.

use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, SystemTime};
use std::collections::HashMap;
use crate::Key;

/// Central metrics collector for ferrous-di operations
#[derive(Debug)]
pub struct MetricsCollector {
    /// Service resolution timing metrics
    resolution_times: RwLock<HashMap<Key, TimingStats>>,
    /// Overall system metrics
    system_metrics: Mutex<SystemMetrics>,
    /// Performance counters
    counters: RwLock<HashMap<String, u64>>,
}

#[derive(Debug, Clone)]
pub struct TimingStats {
    /// Total number of resolutions
    pub count: u64,
    /// Minimum resolution time
    pub min_duration: Duration,
    /// Maximum resolution time
    pub max_duration: Duration,
    /// Total accumulated time
    pub total_duration: Duration,
    /// Recent resolution times (for percentiles)
    pub recent_times: Vec<Duration>,
}

#[derive(Debug, Clone)]
pub struct SystemMetrics {
    /// Start time of the metrics collection
    pub start_time: SystemTime,
    /// Total services resolved
    pub total_resolutions: u64,
    /// Total scopes created
    pub scopes_created: u64,
    /// Total services disposed
    pub services_disposed: u64,
    /// Current active scopes
    pub active_scopes: u64,
    /// Memory usage estimates
    pub estimated_memory_kb: u64,
}

impl TimingStats {
    fn new() -> Self {
        Self {
            count: 0,
            min_duration: Duration::MAX,
            max_duration: Duration::ZERO,
            total_duration: Duration::ZERO,
            recent_times: Vec::with_capacity(100), // Keep last 100 measurements
        }
    }

    /// Add a new timing measurement
    pub fn record(&mut self, duration: Duration) {
        self.count += 1;
        self.min_duration = self.min_duration.min(duration);
        self.max_duration = self.max_duration.max(duration);
        self.total_duration += duration;
        
        // Keep recent times for percentile calculations
        if self.recent_times.len() >= 100 {
            self.recent_times.remove(0);
        }
        self.recent_times.push(duration);
    }

    /// Calculate average resolution time
    pub fn average_duration(&self) -> Duration {
        if self.count == 0 {
            Duration::ZERO
        } else {
            self.total_duration / self.count as u32
        }
    }

    /// Calculate 95th percentile resolution time
    pub fn p95_duration(&self) -> Duration {
        if self.recent_times.is_empty() {
            return Duration::ZERO;
        }

        let mut sorted = self.recent_times.clone();
        sorted.sort();
        let index = (sorted.len() as f64 * 0.95) as usize;
        sorted.get(index.min(sorted.len() - 1)).copied().unwrap_or(Duration::ZERO)
    }
}

impl MetricsCollector {
    /// Create a new metrics collector
    pub fn new() -> Self {
        Self {
            resolution_times: RwLock::new(HashMap::new()),
            system_metrics: Mutex::new(SystemMetrics {
                start_time: SystemTime::now(),
                total_resolutions: 0,
                scopes_created: 0,
                services_disposed: 0,
                active_scopes: 0,
                estimated_memory_kb: 0,
            }),
            counters: RwLock::new(HashMap::new()),
        }
    }

    /// Record a service resolution timing
    pub fn record_resolution(&self, key: &Key, duration: Duration) {
        // Update per-service timing stats
        if let Ok(mut times) = self.resolution_times.write() {
            let stats = times.entry(key.clone()).or_insert_with(TimingStats::new);
            stats.record(duration);
        }

        // Update system metrics
        if let Ok(mut system) = self.system_metrics.lock() {
            system.total_resolutions += 1;
        }
    }

    /// Record scope creation
    pub fn record_scope_created(&self) {
        if let Ok(mut system) = self.system_metrics.lock() {
            system.scopes_created += 1;
            system.active_scopes += 1;
        }
    }

    /// Record scope disposal
    pub fn record_scope_disposed(&self) {
        if let Ok(mut system) = self.system_metrics.lock() {
            if system.active_scopes > 0 {
                system.active_scopes -= 1;
            }
        }
    }

    /// Record service disposal
    pub fn record_service_disposed(&self) {
        if let Ok(mut system) = self.system_metrics.lock() {
            system.services_disposed += 1;
        }
    }

    /// Increment a named counter
    pub fn increment_counter(&self, name: &str) {
        if let Ok(mut counters) = self.counters.write() {
            *counters.entry(name.to_string()).or_insert(0) += 1;
        }
    }

    /// Get timing stats for a specific service
    pub fn get_timing_stats(&self, key: &Key) -> Option<TimingStats> {
        self.resolution_times.read().ok()?.get(key).cloned()
    }

    /// Get system-wide metrics
    pub fn get_system_metrics(&self) -> SystemMetrics {
        self.system_metrics.lock().unwrap().clone()
    }

    /// Get all counters
    pub fn get_counters(&self) -> HashMap<String, u64> {
        self.counters.read().unwrap().clone()
    }

    /// Get a summary of the top slowest services
    pub fn get_slowest_services(&self, limit: usize) -> Vec<(Key, Duration)> {
        if let Ok(times) = self.resolution_times.read() {
            let mut services: Vec<_> = times
                .iter()
                .map(|(key, stats)| (key.clone(), stats.average_duration()))
                .collect();
            
            services.sort_by(|a, b| b.1.cmp(&a.1));
            services.into_iter().take(limit).collect()
        } else {
            Vec::new()
        }
    }

    /// Export metrics in Prometheus format
    pub fn export_prometheus(&self) -> String {
        let mut output = String::new();
        
        // System metrics
        if let Ok(system) = self.system_metrics.lock() {
            output.push_str(&format!(
                "# HELP ferrous_di_total_resolutions Total number of service resolutions\n\
                # TYPE ferrous_di_total_resolutions counter\n\
                ferrous_di_total_resolutions {}\n\n",
                system.total_resolutions
            ));
            
            output.push_str(&format!(
                "# HELP ferrous_di_active_scopes Current number of active scopes\n\
                # TYPE ferrous_di_active_scopes gauge\n\
                ferrous_di_active_scopes {}\n\n",
                system.active_scopes
            ));
        }

        // Resolution time histograms
        if let Ok(times) = self.resolution_times.read() {
            output.push_str(
                "# HELP ferrous_di_resolution_duration_seconds Time spent resolving services\n\
                # TYPE ferrous_di_resolution_duration_seconds histogram\n"
            );
            
            for (key, stats) in times.iter() {
                let service_name = key.display_name();
                let _avg_secs = stats.average_duration().as_secs_f64();
                output.push_str(&format!(
                    "ferrous_di_resolution_duration_seconds_sum{{service=\"{}\"}} {}\n\
                    ferrous_di_resolution_duration_seconds_count{{service=\"{}\"}} {}\n",
                    service_name, stats.total_duration.as_secs_f64(),
                    service_name, stats.count
                ));
            }
            output.push('\n');
        }

        // Custom counters
        if let Ok(counters) = self.counters.read() {
            for (name, value) in counters.iter() {
                output.push_str(&format!(
                    "# HELP ferrous_di_{} Custom counter\n\
                    # TYPE ferrous_di_{} counter\n\
                    ferrous_di_{} {}\n\n",
                    name, name, name, value
                ));
            }
        }

        output
    }
}

impl Default for MetricsCollector {
    fn default() -> Self {
        Self::new()
    }
}

/// Health check system for monitoring service container health
pub struct HealthChecker {
    /// Registered health check functions
    checks: RwLock<HashMap<String, Box<dyn Fn() -> HealthStatus + Send + Sync>>>,
    /// Metrics collector reference
    metrics: Arc<MetricsCollector>,
}

impl std::fmt::Debug for HealthChecker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HealthChecker")
            .field("checks", &format!("HashMap with {} checks", self.checks.read().unwrap().len()))
            .field("metrics", &self.metrics)
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum HealthStatus {
    Healthy,
    Degraded(String),
    Unhealthy(String),
}

impl HealthStatus {
    /// Check if the status indicates health
    pub fn is_healthy(&self) -> bool {
        matches!(self, HealthStatus::Healthy)
    }

    /// Get a numeric score (0-100) for the health status
    pub fn score(&self) -> u8 {
        match self {
            HealthStatus::Healthy => 100,
            HealthStatus::Degraded(_) => 50,
            HealthStatus::Unhealthy(_) => 0,
        }
    }
}

impl HealthChecker {
    /// Create a new health checker
    pub fn new(metrics: Arc<MetricsCollector>) -> Self {
        let mut checker = Self {
            checks: RwLock::new(HashMap::new()),
            metrics,
        };

        // Register default health checks
        checker.register_default_checks();
        checker
    }

    /// Register a health check function
    pub fn register_check<F>(&self, name: String, check: F)
    where
        F: Fn() -> HealthStatus + Send + Sync + 'static,
    {
        if let Ok(mut checks) = self.checks.write() {
            checks.insert(name, Box::new(check));
        }
    }

    /// Run all health checks and return overall status
    pub fn check_health(&self) -> HashMap<String, HealthStatus> {
        if let Ok(checks) = self.checks.read() {
            checks
                .iter()
                .map(|(name, check)| (name.clone(), check()))
                .collect()
        } else {
            HashMap::new()
        }
    }

    /// Get overall system health score (0-100)
    pub fn overall_health_score(&self) -> u8 {
        let results = self.check_health();
        if results.is_empty() {
            return 100; // No checks means healthy by default
        }

        let total_score: u32 = results.values().map(|status| status.score() as u32).sum();
        (total_score / results.len() as u32) as u8
    }

    /// Register default health checks
    fn register_default_checks(&mut self) {
        let metrics = self.metrics.clone();
        
        // Memory usage check
        self.register_check("memory_usage".to_string(), move || {
            let system_metrics = metrics.get_system_metrics();
            if system_metrics.estimated_memory_kb > 1_000_000 { // 1GB
                HealthStatus::Degraded("High memory usage".to_string())
            } else if system_metrics.estimated_memory_kb > 2_000_000 { // 2GB
                HealthStatus::Unhealthy("Critical memory usage".to_string())
            } else {
                HealthStatus::Healthy
            }
        });

        // Active scopes check
        let metrics2 = self.metrics.clone();
        self.register_check("scope_count".to_string(), move || {
            let system_metrics = metrics2.get_system_metrics();
            if system_metrics.active_scopes > 1000 {
                HealthStatus::Degraded("High number of active scopes".to_string())
            } else if system_metrics.active_scopes > 5000 {
                HealthStatus::Unhealthy("Critical number of active scopes".to_string())
            } else {
                HealthStatus::Healthy
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_timing_stats() {
        let mut stats = TimingStats::new();
        
        stats.record(Duration::from_millis(10));
        stats.record(Duration::from_millis(20));
        stats.record(Duration::from_millis(30));
        
        assert_eq!(stats.count, 3);
        assert_eq!(stats.min_duration, Duration::from_millis(10));
        assert_eq!(stats.max_duration, Duration::from_millis(30));
        assert_eq!(stats.average_duration(), Duration::from_millis(20));
    }

    #[test]
    fn test_metrics_collector() {
        let collector = MetricsCollector::new();
        let key = crate::Key::Type(std::any::TypeId::of::<String>(), "String");
        
        collector.record_resolution(&key, Duration::from_millis(15));
        collector.record_scope_created();
        collector.increment_counter("test_counter");
        
        let stats = collector.get_timing_stats(&key).unwrap();
        assert_eq!(stats.count, 1);
        assert_eq!(stats.average_duration(), Duration::from_millis(15));
        
        let system = collector.get_system_metrics();
        assert_eq!(system.total_resolutions, 1);
        assert_eq!(system.scopes_created, 1);
        assert_eq!(system.active_scopes, 1);
        
        let counters = collector.get_counters();
        assert_eq!(counters.get("test_counter"), Some(&1));
    }

    #[test]
    fn test_health_checker() {
        let metrics = Arc::new(MetricsCollector::new());
        let checker = HealthChecker::new(metrics);
        
        checker.register_check("always_healthy".to_string(), || HealthStatus::Healthy);
        checker.register_check("always_degraded".to_string(), || {
            HealthStatus::Degraded("Test degradation".to_string())
        });
        
        let results = checker.check_health();
        assert_eq!(results.len(), 4); // 2 custom + 2 default checks
        assert!(results.get("always_healthy").unwrap().is_healthy());
        
        let score = checker.overall_health_score();
        assert!(score > 0 && score <= 100);
    }

    #[test]
    fn test_prometheus_export() {
        let collector = MetricsCollector::new();
        let key = crate::Key::Type(std::any::TypeId::of::<String>(), "String");
        
        collector.record_resolution(&key, Duration::from_millis(10));
        collector.increment_counter("test_events");
        
        let prometheus = collector.export_prometheus();
        assert!(prometheus.contains("ferrous_di_total_resolutions 1"));
        assert!(prometheus.contains("ferrous_di_test_events 1"));
        assert!(prometheus.contains("ferrous_di_resolution_duration_seconds"));
    }
}