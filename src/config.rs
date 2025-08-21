//! Configuration management system for ferrous-di.
//!
//! This module provides environment-based configuration, service discovery
//! integration, and runtime configuration management for production deployments.

use std::collections::HashMap;
use std::env;
use std::sync::RwLock;
use std::time::Duration;
#[cfg(feature = "config")]
use serde::{Deserialize, Serialize};
use crate::{DiResult, DiError};

/// Configuration provider for dependency injection container
pub struct ConfigProvider {
    /// Configuration sources in priority order
    sources: Vec<Box<dyn ConfigSource>>,
    /// Cached configuration values
    cache: RwLock<HashMap<String, ConfigValue>>,
    /// Configuration change listeners
    listeners: RwLock<Vec<Box<dyn Fn(&str, &ConfigValue) + Send + Sync>>>,
}

impl std::fmt::Debug for ConfigProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConfigProvider")
            .field("sources", &format!("{} sources", self.sources.len()))
            .field("cache", &self.cache)
            .field("listeners", &format!("{} listeners", self.listeners.read().unwrap().len()))
            .finish()
    }
}

/// A configuration value that can be various types
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "config", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "config", serde(untagged))]
pub enum ConfigValue {
    String(String),
    Integer(i64),
    Float(f64),
    Boolean(bool),
    Array(Vec<ConfigValue>),
    Object(HashMap<String, ConfigValue>),
}

impl ConfigValue {
    /// Try to convert to string
    pub fn as_string(&self) -> DiResult<&str> {
        match self {
            ConfigValue::String(s) => Ok(s),
            _ => Err(DiError::TypeMismatch("Config value is not a string")),
        }
    }

    /// Try to convert to integer
    pub fn as_i64(&self) -> DiResult<i64> {
        match self {
            ConfigValue::Integer(i) => Ok(*i),
            _ => Err(DiError::TypeMismatch("Config value is not an integer")),
        }
    }

    /// Try to convert to boolean
    pub fn as_bool(&self) -> DiResult<bool> {
        match self {
            ConfigValue::Boolean(b) => Ok(*b),
            _ => Err(DiError::TypeMismatch("Config value is not a boolean")),
        }
    }

    /// Try to convert to duration from milliseconds
    pub fn as_duration_ms(&self) -> DiResult<Duration> {
        let ms = self.as_i64()?;
        if ms < 0 {
            return Err(DiError::TypeMismatch("Duration cannot be negative"));
        }
        Ok(Duration::from_millis(ms as u64))
    }
}

/// Trait for configuration sources
pub trait ConfigSource: Send + Sync + std::fmt::Debug {
    /// Get a configuration value by key
    fn get(&self, key: &str) -> Option<ConfigValue>;
    
    /// List all available keys
    fn keys(&self) -> Vec<String>;
    
    /// Check if the source supports live updates
    fn supports_updates(&self) -> bool {
        false
    }
}

/// Environment variable configuration source
#[derive(Debug, Default)]
pub struct EnvironmentConfigSource {
    /// Prefix to filter environment variables
    prefix: Option<String>,
}

impl EnvironmentConfigSource {
    pub fn new() -> Self {
        Self { prefix: None }
    }

    pub fn with_prefix(prefix: String) -> Self {
        Self { prefix: Some(prefix) }
    }
}

impl ConfigSource for EnvironmentConfigSource {
    fn get(&self, key: &str) -> Option<ConfigValue> {
        let env_key = if let Some(prefix) = &self.prefix {
            format!("{}_{}", prefix.to_uppercase(), key.to_uppercase())
        } else {
            key.to_uppercase()
        };

        env::var(&env_key).ok().map(|value| {
            // Try to parse as different types
            if let Ok(int_val) = value.parse::<i64>() {
                ConfigValue::Integer(int_val)
            } else if let Ok(float_val) = value.parse::<f64>() {
                ConfigValue::Float(float_val)
            } else if let Ok(bool_val) = value.parse::<bool>() {
                ConfigValue::Boolean(bool_val)
            } else {
                ConfigValue::String(value)
            }
        })
    }

    fn keys(&self) -> Vec<String> {
        env::vars()
            .filter_map(|(key, _)| {
                if let Some(prefix) = &self.prefix {
                    let prefix_upper = prefix.to_uppercase();
                    if key.starts_with(&format!("{}_", prefix_upper)) {
                        Some(key[prefix_upper.len() + 1..].to_lowercase())
                    } else {
                        None
                    }
                } else {
                    Some(key.to_lowercase())
                }
            })
            .collect()
    }
}

/// JSON file configuration source
#[cfg(feature = "config")]
#[derive(Debug)]
pub struct JsonConfigSource {
    /// File path to JSON configuration
    file_path: String,
    /// Cached parsed configuration
    config: RwLock<Option<HashMap<String, ConfigValue>>>,
}

#[cfg(feature = "config")]
impl JsonConfigSource {
    pub fn new(file_path: String) -> Self {
        Self {
            file_path,
            config: RwLock::new(None),
        }
    }

    /// Reload configuration from file
    pub fn reload(&self) -> DiResult<()> {
        let content = std::fs::read_to_string(&self.file_path)
            .map_err(|_| DiError::NotFound("Configuration file not found"))?;
        
        let parsed: HashMap<String, ConfigValue> = serde_json::from_str(&content)
            .map_err(|_| DiError::TypeMismatch("Invalid JSON configuration"))?;
        
        if let Ok(mut config) = self.config.write() {
            *config = Some(parsed);
        }

        Ok(())
    }
}

#[cfg(feature = "config")]
impl ConfigSource for JsonConfigSource {
    fn get(&self, key: &str) -> Option<ConfigValue> {
        // Load config if not cached
        if self.config.read().ok()?.is_none() {
            let _ = self.reload();
        }

        self.config.read().ok()?.as_ref()?.get(key).cloned()
    }

    fn keys(&self) -> Vec<String> {
        if let Ok(config) = self.config.read() {
            if let Some(cfg) = config.as_ref() {
                return cfg.keys().cloned().collect();
            }
        }
        Vec::new()
    }

    fn supports_updates(&self) -> bool {
        true
    }
}

impl ConfigProvider {
    /// Create a new configuration provider
    pub fn new() -> Self {
        Self {
            sources: Vec::new(),
            cache: RwLock::new(HashMap::new()),
            listeners: RwLock::new(Vec::new()),
        }
    }

    /// Add a configuration source (higher priority sources should be added first)
    pub fn add_source(&mut self, source: Box<dyn ConfigSource>) {
        self.sources.push(source);
    }

    /// Get a configuration value, checking sources in priority order
    pub fn get(&self, key: &str) -> Option<ConfigValue> {
        // Check cache first
        if let Ok(cache) = self.cache.read() {
            if let Some(value) = cache.get(key) {
                return Some(value.clone());
            }
        }

        // Check sources in order
        for source in &self.sources {
            if let Some(value) = source.get(key) {
                // Cache the value
                if let Ok(mut cache) = self.cache.write() {
                    cache.insert(key.to_string(), value.clone());
                }
                return Some(value);
            }
        }

        None
    }

    /// Get a configuration value with a default
    pub fn get_or_default(&self, key: &str, default: ConfigValue) -> ConfigValue {
        self.get(key).unwrap_or(default)
    }

    /// Get a string configuration value
    pub fn get_string(&self, key: &str) -> DiResult<String> {
        self.get(key)
            .ok_or_else(|| DiError::NotFound("Configuration key not found"))?
            .as_string()
            .map(|s| s.to_string())
    }

    /// Get a string configuration value with default
    pub fn get_string_or(&self, key: &str, default: &str) -> String {
        self.get_string(key).unwrap_or_else(|_| default.to_string())
    }

    /// Get an integer configuration value
    pub fn get_i64(&self, key: &str) -> DiResult<i64> {
        self.get(key)
            .ok_or_else(|| DiError::NotFound("Configuration key not found"))?
            .as_i64()
    }

    /// Get an integer configuration value with default
    pub fn get_i64_or(&self, key: &str, default: i64) -> i64 {
        self.get_i64(key).unwrap_or(default)
    }

    /// Get a boolean configuration value
    pub fn get_bool(&self, key: &str) -> DiResult<bool> {
        self.get(key)
            .ok_or_else(|| DiError::NotFound("Configuration key not found"))?
            .as_bool()
    }

    /// Get a boolean configuration value with default
    pub fn get_bool_or(&self, key: &str, default: bool) -> bool {
        self.get_bool(key).unwrap_or(default)
    }

    /// Get a duration configuration value (from milliseconds)
    pub fn get_duration_ms(&self, key: &str) -> DiResult<Duration> {
        self.get(key)
            .ok_or_else(|| DiError::NotFound("Configuration key not found"))?
            .as_duration_ms()
    }

    /// Get a duration configuration value with default
    pub fn get_duration_ms_or(&self, key: &str, default: Duration) -> Duration {
        self.get_duration_ms(key).unwrap_or(default)
    }

    /// Add a configuration change listener
    pub fn add_listener<F>(&self, listener: F)
    where
        F: Fn(&str, &ConfigValue) + Send + Sync + 'static,
    {
        if let Ok(mut listeners) = self.listeners.write() {
            listeners.push(Box::new(listener));
        }
    }

    /// Clear the configuration cache (forces reload from sources)
    pub fn invalidate_cache(&self) {
        if let Ok(mut cache) = self.cache.write() {
            cache.clear();
        }
    }

    /// Get all configuration keys from all sources
    pub fn all_keys(&self) -> Vec<String> {
        let mut keys = Vec::new();
        for source in &self.sources {
            keys.extend(source.keys());
        }
        keys.sort();
        keys.dedup();
        keys
    }
}

impl Default for ConfigProvider {
    fn default() -> Self {
        let mut provider = Self::new();
        // Add environment variables as default source
        provider.add_source(Box::new(EnvironmentConfigSource::new()));
        provider
    }
}

/// Service discovery configuration
#[derive(Debug, Clone)]
#[cfg_attr(feature = "config", derive(Serialize, Deserialize))]
pub struct ServiceDiscoveryConfig {
    /// Service discovery backend type
    pub backend: ServiceDiscoveryBackend,
    /// Connection endpoint
    pub endpoint: String,
    /// Refresh interval for service discovery
    pub refresh_interval: Duration,
    /// Service health check interval
    pub health_check_interval: Option<Duration>,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "config", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "config", serde(rename_all = "lowercase"))]
pub enum ServiceDiscoveryBackend {
    Consul,
    Etcd,
    Kubernetes,
    Static,
}

/// Container configuration for production deployments
#[derive(Debug, Clone)]
pub struct ContainerConfig {
    /// Performance settings
    pub performance: PerformanceConfig,
    /// Metrics and monitoring settings
    pub monitoring: MonitoringConfig,
    /// Service discovery configuration
    pub service_discovery: Option<ServiceDiscoveryConfig>,
}

#[derive(Debug, Clone)]
pub struct PerformanceConfig {
    /// Enable resolution caching
    pub enable_cache: bool,
    /// Cache configuration
    pub cache_max_entries: usize,
    pub cache_ttl: Option<Duration>,
    /// Enable memory pooling for transients
    pub enable_memory_pools: bool,
    /// Enable lazy initialization for singletons
    pub enable_lazy_init: bool,
}

#[derive(Debug, Clone)]
pub struct MonitoringConfig {
    /// Enable metrics collection
    pub enable_metrics: bool,
    /// Metrics export format
    pub metrics_format: MetricsFormat,
    /// Health check endpoint configuration
    pub health_check_enabled: bool,
    pub health_check_port: Option<u16>,
}

#[derive(Debug, Clone)]
pub enum MetricsFormat {
    Prometheus,
    Json,
    StatsD,
}

impl ContainerConfig {
    /// Load configuration from a config provider
    pub fn load(config: &ConfigProvider) -> Self {
        Self {
            performance: PerformanceConfig {
                enable_cache: config.get_bool_or("performance.cache.enabled", true),
                cache_max_entries: config.get_i64_or("performance.cache.max_entries", 1000) as usize,
                cache_ttl: Some(config.get_duration_ms_or("performance.cache.ttl_ms", Duration::from_secs(300))),
                enable_memory_pools: config.get_bool_or("performance.memory_pools.enabled", true),
                enable_lazy_init: config.get_bool_or("performance.lazy_init.enabled", true),
            },
            monitoring: MonitoringConfig {
                enable_metrics: config.get_bool_or("monitoring.metrics.enabled", true),
                metrics_format: match config.get_string_or("monitoring.metrics.format", "prometheus").as_str() {
                    "json" => MetricsFormat::Json,
                    "statsd" => MetricsFormat::StatsD,
                    _ => MetricsFormat::Prometheus,
                },
                health_check_enabled: config.get_bool_or("monitoring.health.enabled", true),
                health_check_port: config.get_i64("monitoring.health.port").ok().map(|p| p as u16),
            },
            service_discovery: None, // TODO: Implement service discovery config loading
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_environment_config_source() {
        env::set_var("TEST_KEY", "test_value");
        env::set_var("TEST_INT", "42");
        env::set_var("TEST_BOOL", "true");

        let source = EnvironmentConfigSource::new();
        
        assert_eq!(source.get("test_key"), Some(ConfigValue::String("test_value".to_string())));
        assert_eq!(source.get("test_int"), Some(ConfigValue::Integer(42)));
        assert_eq!(source.get("test_bool"), Some(ConfigValue::Boolean(true)));

        env::remove_var("TEST_KEY");
        env::remove_var("TEST_INT");
        env::remove_var("TEST_BOOL");
    }

    #[test]
    fn test_environment_config_with_prefix() {
        env::set_var("MYAPP_DATABASE_URL", "postgres://localhost");
        
        let source = EnvironmentConfigSource::with_prefix("MYAPP".to_string());
        
        assert_eq!(
            source.get("database_url"), 
            Some(ConfigValue::String("postgres://localhost".to_string()))
        );

        env::remove_var("MYAPP_DATABASE_URL");
    }

    #[test]
    fn test_config_provider() {
        let mut provider = ConfigProvider::new();
        
        // Create a mock source
        #[derive(Debug)]
        struct MockSource;
        impl ConfigSource for MockSource {
            fn get(&self, key: &str) -> Option<ConfigValue> {
                match key {
                    "test_key" => Some(ConfigValue::String("mock_value".to_string())),
                    "test_number" => Some(ConfigValue::Integer(123)),
                    _ => None,
                }
            }
            
            fn keys(&self) -> Vec<String> {
                vec!["test_key".to_string(), "test_number".to_string()]
            }
        }

        provider.add_source(Box::new(MockSource));

        assert_eq!(provider.get_string("test_key").unwrap(), "mock_value");
        assert_eq!(provider.get_i64("test_number").unwrap(), 123);
        assert_eq!(provider.get_string_or("missing_key", "default"), "default");

        let keys = provider.all_keys();
        assert!(keys.contains(&"test_key".to_string()));
        assert!(keys.contains(&"test_number".to_string()));
    }

    #[test]
    fn test_config_value_conversions() {
        let string_val = ConfigValue::String("hello".to_string());
        let int_val = ConfigValue::Integer(42);
        let bool_val = ConfigValue::Boolean(true);
        let duration_val = ConfigValue::Integer(5000); // 5 seconds in ms

        assert_eq!(string_val.as_string().unwrap(), "hello");
        assert_eq!(int_val.as_i64().unwrap(), 42);
        assert_eq!(bool_val.as_bool().unwrap(), true);
        assert_eq!(duration_val.as_duration_ms().unwrap(), Duration::from_secs(5));

        // Test type mismatches
        assert!(string_val.as_i64().is_err());
        assert!(int_val.as_string().is_err());
    }

    #[test]
    fn test_container_config_loading() {
        let mut provider = ConfigProvider::new();
        
        #[derive(Debug)]
        struct TestConfigSource;
        impl ConfigSource for TestConfigSource {
            fn get(&self, key: &str) -> Option<ConfigValue> {
                match key {
                    "performance.cache.enabled" => Some(ConfigValue::Boolean(false)),
                    "performance.cache.max_entries" => Some(ConfigValue::Integer(500)),
                    "monitoring.metrics.enabled" => Some(ConfigValue::Boolean(true)),
                    "monitoring.metrics.format" => Some(ConfigValue::String("json".to_string())),
                    _ => None,
                }
            }
            
            fn keys(&self) -> Vec<String> { Vec::new() }
        }

        provider.add_source(Box::new(TestConfigSource));
        
        let config = ContainerConfig::load(&provider);
        
        assert!(!config.performance.enable_cache);
        assert_eq!(config.performance.cache_max_entries, 500);
        assert!(config.monitoring.enable_metrics);
        assert!(matches!(config.monitoring.metrics_format, MetricsFormat::Json));
    }
}