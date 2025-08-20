//! Performance optimization components for ferrous-di.
//!
//! This module provides advanced performance features including:
//! - Service resolution caching
//! - Memory pool management
//! - Lazy initialization
//! - Dependency graph optimization

use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};
// use std::any::TypeId;
use std::time::{Duration, Instant};
use crate::{Key, DiResult, DiError};
use crate::registration::AnyArc;

/// Service resolution cache for frequently accessed services
///
/// Caches resolved service instances to avoid repeated resolution overhead.
/// Particularly beneficial for complex dependency graphs with deep nesting.
#[derive(Debug)]
pub struct ResolutionCache {
    /// Cached service instances with expiration times
    cache: RwLock<HashMap<Key, CacheEntry>>,
    /// Cache configuration settings
    config: CacheConfig,
    /// Cache hit/miss statistics
    stats: Mutex<CacheStats>,
}

#[derive(Debug, Clone)]
struct CacheEntry {
    /// The cached service instance
    service: AnyArc,
    /// When this entry was created
    created_at: Instant,
    /// How many times this entry has been accessed
    access_count: u64,
    /// Last access time for LRU eviction
    last_accessed: Instant,
}

#[derive(Debug, Clone)]
pub struct CacheConfig {
    /// Maximum number of cached entries
    pub max_entries: usize,
    /// Time-to-live for cache entries (None = no expiration)
    pub ttl: Option<Duration>,
    /// Enable LRU eviction when cache is full
    pub enable_lru: bool,
    /// Enable access count tracking
    pub track_access_count: bool,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            max_entries: 1000,
            ttl: Some(Duration::from_secs(300)), // 5 minutes
            enable_lru: true,
            track_access_count: true,
        }
    }
}

#[derive(Debug, Default)]
pub struct CacheStats {
    /// Total cache hit count
    pub hits: u64,
    /// Total cache miss count
    pub misses: u64,
    /// Total evictions due to TTL expiration
    pub ttl_evictions: u64,
    /// Total evictions due to LRU
    pub lru_evictions: u64,
}

impl CacheStats {
    /// Calculate hit ratio as a percentage
    pub fn hit_ratio(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 {
            0.0
        } else {
            (self.hits as f64 / total as f64) * 100.0
        }
    }
}

impl ResolutionCache {
    /// Create a new resolution cache with default configuration
    pub fn new() -> Self {
        Self::with_config(CacheConfig::default())
    }

    /// Create a new resolution cache with custom configuration
    pub fn with_config(config: CacheConfig) -> Self {
        Self {
            cache: RwLock::new(HashMap::new()),
            config,
            stats: Mutex::new(CacheStats::default()),
        }
    }

    /// Get a service from the cache
    pub fn get(&self, key: &Key) -> Option<AnyArc> {
        let mut stats = self.stats.lock().unwrap();
        
        // Check if we have the entry and it's not expired
        if let Ok(mut cache) = self.cache.write() {
            if let Some(entry) = cache.get_mut(key) {
                // Check TTL expiration
                if let Some(ttl) = self.config.ttl {
                    if entry.created_at.elapsed() > ttl {
                        cache.remove(key);
                        stats.ttl_evictions += 1;
                        stats.misses += 1;
                        return None;
                    }
                }

                // Update access tracking
                if self.config.track_access_count {
                    entry.access_count += 1;
                    entry.last_accessed = Instant::now();
                }

                stats.hits += 1;
                return Some(entry.service.clone());
            }
        }

        stats.misses += 1;
        None
    }

    /// Put a service into the cache
    pub fn put(&self, key: Key, service: AnyArc) -> DiResult<()> {
        if let Ok(mut cache) = self.cache.write() {
            // Check if we need to evict entries
            if cache.len() >= self.config.max_entries {
                if self.config.enable_lru {
                    self.evict_lru(&mut cache)?;
                } else {
                    return Err(DiError::TypeMismatch("Cache capacity exceeded"));
                }
            }

            let entry = CacheEntry {
                service,
                created_at: Instant::now(),
                access_count: 0,
                last_accessed: Instant::now(),
            };

            cache.insert(key, entry);
        }

        Ok(())
    }

    /// Evict the least recently used entry
    fn evict_lru(&self, cache: &mut HashMap<Key, CacheEntry>) -> DiResult<()> {
        if cache.is_empty() {
            return Ok(());
        }

        // Find the LRU entry
        let lru_key = cache
            .iter()
            .min_by_key(|(_, entry)| entry.last_accessed)
            .map(|(key, _)| key.clone())
            .ok_or(DiError::TypeMismatch("Failed to find LRU entry"))?;

        cache.remove(&lru_key);
        
        // Update stats
        if let Ok(mut stats) = self.stats.lock() {
            stats.lru_evictions += 1;
        }

        Ok(())
    }

    /// Clear all cached entries
    pub fn clear(&self) {
        if let Ok(mut cache) = self.cache.write() {
            cache.clear();
        }
    }

    /// Get cache statistics
    pub fn stats(&self) -> CacheStats {
        let stats = self.stats.lock().unwrap();
        CacheStats {
            hits: stats.hits,
            misses: stats.misses,
            ttl_evictions: stats.ttl_evictions,
            lru_evictions: stats.lru_evictions,
        }
    }

    /// Get current cache size
    pub fn size(&self) -> usize {
        self.cache.read().unwrap().len()
    }
}

impl Default for ResolutionCache {
    fn default() -> Self {
        Self::new()
    }
}

/// Memory pool for reusing transient service allocations
///
/// Reduces allocation overhead for frequently created transient services
/// by maintaining pools of pre-allocated instances.
pub struct ServicePool<T> {
    /// Pool of available service instances
    pool: Mutex<Vec<T>>,
    /// Factory function to create new instances
    factory: Box<dyn Fn() -> T + Send + Sync>,
    /// Maximum pool size
    max_size: usize,
}

impl<T> std::fmt::Debug for ServicePool<T> 
where 
    T: Send + 'static,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ServicePool")
            .field("pool_size", &self.size())
            .field("max_size", &self.max_size)
            .finish()
    }
}

impl<T> ServicePool<T> 
where 
    T: Send + 'static,
{
    /// Create a new service pool with a factory function
    pub fn new<F>(factory: F, max_size: usize) -> Self 
    where 
        F: Fn() -> T + Send + Sync + 'static,
    {
        Self {
            pool: Mutex::new(Vec::new()),
            factory: Box::new(factory),
            max_size,
        }
    }

    /// Get an instance from the pool or create a new one
    pub fn get(&self) -> T {
        if let Ok(mut pool) = self.pool.lock() {
            if let Some(instance) = pool.pop() {
                return instance;
            }
        }
        
        // Pool is empty, create new instance
        (self.factory)()
    }

    /// Return an instance to the pool for reuse
    pub fn put(&self, instance: T) {
        if let Ok(mut pool) = self.pool.lock() {
            if pool.len() < self.max_size {
                pool.push(instance);
            }
            // If pool is full, just drop the instance
        }
    }

    /// Get current pool size
    pub fn size(&self) -> usize {
        self.pool.lock().unwrap().len()
    }

    /// Clear the pool
    pub fn clear(&self) {
        if let Ok(mut pool) = self.pool.lock() {
            pool.clear();
        }
    }
}

/// Lazy initialization wrapper for expensive singleton services
///
/// Defers service creation until first access, improving startup performance.
pub struct LazyService<T> {
    /// The lazily initialized value
    value: RwLock<Option<Arc<T>>>,
    /// Initialization function
    init: Box<dyn Fn() -> DiResult<T> + Send + Sync>,
}

impl<T> std::fmt::Debug for LazyService<T> 
where 
    T: Send + Sync + 'static,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LazyService")
            .field("is_initialized", &self.is_initialized())
            .finish()
    }
}

impl<T> LazyService<T> 
where 
    T: Send + Sync + 'static,
{
    /// Create a new lazy service with an initialization function
    pub fn new<F>(init: F) -> Self 
    where 
        F: Fn() -> DiResult<T> + Send + Sync + 'static,
    {
        Self {
            value: RwLock::new(None),
            init: Box::new(init),
        }
    }

    /// Get the service instance, initializing if necessary
    pub fn get(&self) -> DiResult<Arc<T>> {
        // Fast path: check if already initialized
        if let Ok(value) = self.value.read() {
            if let Some(service) = value.as_ref() {
                return Ok(service.clone());
            }
        }

        // Slow path: need to initialize
        if let Ok(mut value) = self.value.write() {
            // Double-checked locking pattern
            if let Some(service) = value.as_ref() {
                return Ok(service.clone());
            }

            // Initialize the service
            let service = (self.init)()?;
            let service_arc = Arc::new(service);
            *value = Some(service_arc.clone());
            Ok(service_arc)
        } else {
            Err(DiError::TypeMismatch("Failed to acquire write lock for lazy initialization"))
        }
    }

    /// Check if the service has been initialized
    pub fn is_initialized(&self) -> bool {
        if let Ok(value) = self.value.read() {
            value.is_some()
        } else {
            false
        }
    }
}

/// Dependency resolution path optimization
///
/// Pre-computes and caches optimal resolution paths for complex dependency graphs.
#[derive(Debug)]
pub struct DependencyGraphOptimizer {
    /// Cached resolution paths
    paths: RwLock<HashMap<Key, ResolutionPath>>,
    /// Graph analysis statistics
    stats: Mutex<GraphStats>,
}

#[derive(Debug, Clone)]
pub struct ResolutionPath {
    /// Ordered list of services to resolve
    pub steps: Vec<Key>,
    /// Estimated resolution cost
    pub cost: u32,
    /// Path depth (number of dependencies)
    pub depth: usize,
}

#[derive(Debug, Default)]
pub struct GraphStats {
    /// Total paths analyzed
    pub paths_analyzed: u64,
    /// Total paths optimized
    pub paths_optimized: u64,
    /// Average path depth
    pub avg_depth: f64,
    /// Maximum path depth found
    pub max_depth: usize,
}

impl DependencyGraphOptimizer {
    /// Create a new dependency graph optimizer
    pub fn new() -> Self {
        Self {
            paths: RwLock::new(HashMap::new()),
            stats: Mutex::new(GraphStats::default()),
        }
    }

    /// Analyze and optimize a resolution path
    pub fn optimize_path(&self, root_key: &Key, dependencies: &[Key]) -> DiResult<ResolutionPath> {
        if let Ok(paths) = self.paths.read() {
            if let Some(cached_path) = paths.get(root_key) {
                return Ok(cached_path.clone());
            }
        }

        // Compute optimal resolution order
        let optimized_steps = self.compute_optimal_order(dependencies)?;
        let path = ResolutionPath {
            cost: optimized_steps.len() as u32 * 10, // Simple cost model
            depth: optimized_steps.len(),
            steps: optimized_steps,
        };

        // Cache the optimized path
        if let Ok(mut paths) = self.paths.write() {
            paths.insert(root_key.clone(), path.clone());
        }

        // Update statistics
        if let Ok(mut stats) = self.stats.lock() {
            stats.paths_analyzed += 1;
            stats.paths_optimized += 1;
            stats.max_depth = stats.max_depth.max(path.depth);
            
            // Update average depth
            let total_depth = stats.avg_depth * (stats.paths_analyzed - 1) as f64 + path.depth as f64;
            stats.avg_depth = total_depth / stats.paths_analyzed as f64;
        }

        Ok(path)
    }

    /// Compute optimal resolution order using topological sort
    fn compute_optimal_order(&self, dependencies: &[Key]) -> DiResult<Vec<Key>> {
        // For now, use simple ordering - in a full implementation,
        // this would do topological sorting of the dependency graph
        Ok(dependencies.to_vec())
    }

    /// Get optimization statistics
    pub fn stats(&self) -> GraphStats {
        let stats = self.stats.lock().unwrap();
        GraphStats {
            paths_analyzed: stats.paths_analyzed,
            paths_optimized: stats.paths_optimized,
            avg_depth: stats.avg_depth,
            max_depth: stats.max_depth,
        }
    }

    /// Clear cached paths
    pub fn clear(&self) {
        if let Ok(mut paths) = self.paths.write() {
            paths.clear();
        }
    }
}

impl Default for DependencyGraphOptimizer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::any::TypeId;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn test_resolution_cache_basic_operations() {
        let cache = ResolutionCache::new();
        let key = Key::Type(TypeId::of::<String>(), "String");
        let service = Arc::new("test_service".to_string()) as AnyArc;

        // Cache miss initially
        assert!(cache.get(&key).is_none());

        // Put and get
        cache.put(key.clone(), service.clone()).unwrap();
        let cached = cache.get(&key).unwrap();
        
        // Should be the same Arc
        assert!(Arc::ptr_eq(&service, &cached));

        // Check stats
        let stats = cache.stats();
        assert_eq!(stats.hits, 1);
        assert_eq!(stats.misses, 1);
        assert!(stats.hit_ratio() > 0.0);
    }

    #[test]
    fn test_resolution_cache_ttl_expiration() {
        let config = CacheConfig {
            max_entries: 10,
            ttl: Some(Duration::from_millis(50)),
            enable_lru: true,
            track_access_count: true,
        };
        
        let cache = ResolutionCache::with_config(config);
        let key = Key::Type(TypeId::of::<String>(), "String");
        let service = Arc::new("test_service".to_string()) as AnyArc;

        // Put service in cache
        cache.put(key.clone(), service).unwrap();
        
        // Should be available immediately
        assert!(cache.get(&key).is_some());

        // Wait for TTL expiration
        thread::sleep(Duration::from_millis(60));

        // Should be expired now
        assert!(cache.get(&key).is_none());

        let stats = cache.stats();
        assert_eq!(stats.ttl_evictions, 1);
    }

    #[test]
    fn test_service_pool_reuse() {
        let pool = ServicePool::new(|| "new_instance".to_string(), 5);

        // Get instance from empty pool (creates new)
        let instance1 = pool.get();
        assert_eq!(instance1, "new_instance");

        // Return to pool
        pool.put(instance1);
        assert_eq!(pool.size(), 1);

        // Get from pool (should reuse)
        let instance2 = pool.get();
        assert_eq!(instance2, "new_instance");
        assert_eq!(pool.size(), 0);
    }

    #[test]
    fn test_lazy_service_initialization() {
        let counter = Arc::new(Mutex::new(0));
        let counter_clone = counter.clone();

        let lazy = LazyService::new(move || {
            let mut c = counter_clone.lock().unwrap();
            *c += 1;
            Ok(format!("initialized_{}", *c))
        });

        // Not initialized initially
        assert!(!lazy.is_initialized());

        // First access initializes
        let service1 = lazy.get().unwrap();
        assert!(lazy.is_initialized());
        assert_eq!(*service1, "initialized_1");

        // Second access returns same instance
        let service2 = lazy.get().unwrap();
        assert!(Arc::ptr_eq(&service1, &service2));

        // Counter should only be incremented once
        assert_eq!(*counter.lock().unwrap(), 1);
    }

    #[test]
    fn test_dependency_graph_optimizer() {
        let optimizer = DependencyGraphOptimizer::new();
        let root_key = Key::Type(TypeId::of::<String>(), "Root");
        let deps = vec![
            Key::Type(TypeId::of::<i32>(), "Dep1"),
            Key::Type(TypeId::of::<f64>(), "Dep2"),
        ];

        let path = optimizer.optimize_path(&root_key, &deps).unwrap();
        
        assert_eq!(path.steps.len(), 2);
        assert_eq!(path.depth, 2);
        assert!(path.cost > 0);

        // Second call should use cached path
        let path2 = optimizer.optimize_path(&root_key, &deps).unwrap();
        assert_eq!(path.steps, path2.steps);

        let stats = optimizer.stats();
        assert_eq!(stats.paths_analyzed, 1);
        assert_eq!(stats.paths_optimized, 1);
    }
}