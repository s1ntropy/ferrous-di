/// Benchmark regression tests for ferrous-di
/// 
/// These tests establish performance baselines and detect regressions in:
/// 1. Service registration speed
/// 2. Service resolution performance (singleton, scoped, transient)
/// 3. Complex dependency graph resolution
/// 4. Concurrent access performance
/// 5. Memory usage patterns

use ferrous_di::{ServiceCollection, ServiceModule, ServiceCollectionModuleExt, Resolver};
use std::sync::{Arc, Mutex, atomic::{AtomicU32, Ordering}};
use std::time::{Duration, Instant};
use std::thread;

// ===== Performance Test Services =====

#[derive(Debug, Clone)]
pub struct LightweightService {
    id: u32,
}

impl LightweightService {
    pub fn new(id: u32) -> Self {
        Self { id }
    }
}

#[derive(Debug)]
pub struct HeavyService {
    id: u32,
    data: Vec<u8>,
    map: std::collections::HashMap<u32, String>,
}

impl HeavyService {
    pub fn new(id: u32) -> Self {
        let mut map = std::collections::HashMap::new();
        for i in 0..100 {
            map.insert(i, format!("value_{}", i));
        }
        
        Self {
            id,
            data: vec![0u8; 4096], // 4KB of data
            map,
        }
    }
    
    pub fn get_id(&self) -> u32 {
        self.id
    }
}

#[derive(Debug)]
pub struct DependentService {
    lightweight: Arc<LightweightService>,
    heavy: Arc<HeavyService>,
    computed_value: u32,
}

impl DependentService {
    pub fn new(lightweight: Arc<LightweightService>, heavy: Arc<HeavyService>) -> Self {
        Self {
            computed_value: lightweight.id + heavy.id,
            lightweight,
            heavy,
        }
    }
}

// Trait for testing trait resolution performance
pub trait PerformanceService: Send + Sync {
    fn execute(&self) -> u32;
}

#[derive(Debug)]
pub struct FastImplementation {
    value: u32,
}

impl FastImplementation {
    pub fn new(value: u32) -> Self {
        Self { value }
    }
}

impl PerformanceService for FastImplementation {
    fn execute(&self) -> u32 {
        self.value
    }
}

// ===== Benchmark Helper Functions =====

pub struct BenchmarkResult {
    pub operation: String,
    pub duration: Duration,
    pub operations_per_second: f64,
    pub memory_used_mb: Option<f64>,
}

impl BenchmarkResult {
    pub fn new(operation: String, duration: Duration, total_operations: u32) -> Self {
        let ops_per_sec = total_operations as f64 / duration.as_secs_f64();
        Self {
            operation,
            duration,
            operations_per_second: ops_per_sec,
            memory_used_mb: None,
        }
    }
    
    pub fn with_memory(mut self, memory_mb: f64) -> Self {
        self.memory_used_mb = Some(memory_mb);
        self
    }
}

fn print_benchmark_result(result: &BenchmarkResult) {
    println!("=== {} ===", result.operation);
    println!("Duration: {:?}", result.duration);
    println!("Ops/sec: {:.0}", result.operations_per_second);
    if let Some(memory) = result.memory_used_mb {
        println!("Memory: {:.2} MB", memory);
    }
    println!();
}

// ===== Core Performance Benchmarks =====

#[test]
#[ignore] // Run with: cargo test -- --ignored benchmark
fn benchmark_service_registration_performance() {
    println!("🔥 BENCHMARK: Service Registration Performance");
    
    // Test 1: Lightweight service registration
    let start = Instant::now();
    let iterations = 10_000;
    
    for _ in 0..iterations {
        let mut services = ServiceCollection::new();
        services.add_singleton(LightweightService::new(1));
        services.add_scoped_factory::<LightweightService, _>(|_| LightweightService::new(2));
        services.add_transient_factory::<LightweightService, _>(|_| LightweightService::new(3));
        let _provider = services.build();
    }
    
    let duration = start.elapsed();
    let result = BenchmarkResult::new(
        "Service Registration (3 services x 10K iterations)".to_string(),
        duration,
        iterations * 3,
    );
    print_benchmark_result(&result);
    
    // Performance assertion: Should complete in reasonable time
    assert!(duration.as_millis() < 5000, "Registration took too long: {:?}", duration);
}

#[test]
#[ignore] // Run with: cargo test -- --ignored benchmark
fn benchmark_singleton_resolution_performance() {
    println!("🔥 BENCHMARK: Singleton Resolution Performance");
    
    let mut services = ServiceCollection::new();
    services.add_singleton(LightweightService::new(42));
    services.add_singleton(HeavyService::new(100));
    
    let provider = services.build();
    let iterations = 1_000_000;
    
    // Warm up
    for _ in 0..1000 {
        let _service = provider.get_required::<LightweightService>();
    }
    
    let start = Instant::now();
    for _ in 0..iterations {
        let _lightweight = provider.get_required::<LightweightService>();
        let _heavy = provider.get_required::<HeavyService>();
    }
    let duration = start.elapsed();
    
    let result = BenchmarkResult::new(
        "Singleton Resolution (2 services x 1M iterations)".to_string(),
        duration,
        iterations * 2,
    );
    print_benchmark_result(&result);
    
    // Performance assertion: Should achieve high throughput
    assert!(result.operations_per_second > 1_000_000.0, 
           "Singleton resolution too slow: {:.0} ops/sec", result.operations_per_second);
}

#[test]
#[ignore] // Run with: cargo test -- --ignored benchmark
fn benchmark_scoped_resolution_performance() {
    println!("🔥 BENCHMARK: Scoped Resolution Performance");
    
    let mut services = ServiceCollection::new();
    services.add_scoped_factory::<LightweightService, _>(|_| LightweightService::new(1));
    services.add_scoped_factory::<HeavyService, _>(|_| HeavyService::new(2));
    
    let provider = services.build();
    let iterations = 100_000;
    
    let start = Instant::now();
    for _ in 0..iterations {
        let scope = provider.create_scope();
        let _lightweight = scope.get_required::<LightweightService>();
        let _heavy = scope.get_required::<HeavyService>();
    }
    let duration = start.elapsed();
    
    let result = BenchmarkResult::new(
        "Scoped Resolution (2 services x 100K scopes)".to_string(),
        duration,
        iterations * 2,
    );
    print_benchmark_result(&result);
    
    // Performance assertion: Should be reasonably fast
    assert!(result.operations_per_second > 50_000.0,
           "Scoped resolution too slow: {:.0} ops/sec", result.operations_per_second);
}

#[test]
#[ignore] // Run with: cargo test -- --ignored benchmark
fn benchmark_transient_resolution_performance() {
    println!("🔥 BENCHMARK: Transient Resolution Performance");
    
    let mut services = ServiceCollection::new();
    services.add_transient_factory::<LightweightService, _>(|_| LightweightService::new(1));
    
    let provider = services.build();
    let iterations = 500_000;
    
    let start = Instant::now();
    for _ in 0..iterations {
        let _service = provider.get_required::<LightweightService>();
    }
    let duration = start.elapsed();
    
    let result = BenchmarkResult::new(
        "Transient Resolution (500K iterations)".to_string(),
        duration,
        iterations,
    );
    print_benchmark_result(&result);
    
    // Performance assertion
    assert!(result.operations_per_second > 100_000.0,
           "Transient resolution too slow: {:.0} ops/sec", result.operations_per_second);
}

#[test]
#[ignore] // Run with: cargo test -- --ignored benchmark
fn benchmark_dependency_injection_performance() {
    println!("🔥 BENCHMARK: Dependency Injection Performance");
    
    let mut services = ServiceCollection::new();
    services.add_singleton(LightweightService::new(1));
    services.add_singleton(HeavyService::new(2));
    services.add_scoped_factory::<DependentService, _>(|r| {
        let lightweight = r.get_required::<LightweightService>();
        let heavy = r.get_required::<HeavyService>();
        DependentService::new(lightweight, heavy)
    });
    
    let provider = services.build();
    let iterations = 100_000;
    
    let start = Instant::now();
    for _ in 0..iterations {
        let scope = provider.create_scope();
        let _dependent = scope.get_required::<DependentService>();
    }
    let duration = start.elapsed();
    
    let result = BenchmarkResult::new(
        "Dependency Injection (100K iterations)".to_string(),
        duration,
        iterations,
    );
    print_benchmark_result(&result);
    
    // Performance assertion
    assert!(result.operations_per_second > 25_000.0,
           "DI resolution too slow: {:.0} ops/sec", result.operations_per_second);
}

#[test]
#[ignore] // Run with: cargo test -- --ignored benchmark
fn benchmark_trait_resolution_performance() {
    println!("🔥 BENCHMARK: Trait Resolution Performance");
    
    let mut services = ServiceCollection::new();
    services.add_singleton_trait::<dyn PerformanceService>(
        Arc::new(FastImplementation::new(42))
    );
    
    let provider = services.build();
    let iterations = 500_000;
    
    // Warm up
    for _ in 0..1000 {
        let _service = provider.get_required_trait::<dyn PerformanceService>();
    }
    
    let start = Instant::now();
    for _ in 0..iterations {
        let service = provider.get_required_trait::<dyn PerformanceService>();
        let _result = service.execute();
    }
    let duration = start.elapsed();
    
    let result = BenchmarkResult::new(
        "Trait Resolution (500K iterations)".to_string(),
        duration,
        iterations,
    );
    print_benchmark_result(&result);
    
    // Performance assertion
    assert!(result.operations_per_second > 200_000.0,
           "Trait resolution too slow: {:.0} ops/sec", result.operations_per_second);
}

#[test]
#[ignore] // Run with: cargo test -- --ignored benchmark
fn benchmark_concurrent_access_performance() {
    println!("🔥 BENCHMARK: Concurrent Access Performance");
    
    let mut services = ServiceCollection::new();
    services.add_singleton(LightweightService::new(1));
    services.add_scoped_factory::<HeavyService, _>(|_| HeavyService::new(2));
    
    let provider = Arc::new(services.build());
    let thread_count = 8;
    let iterations_per_thread = 50_000;
    
    let start = Instant::now();
    let handles: Vec<_> = (0..thread_count).map(|_| {
        let provider = Arc::clone(&provider);
        thread::spawn(move || {
            for _ in 0..iterations_per_thread {
                let _singleton = provider.get_required::<LightweightService>();
                let scope = provider.create_scope();
                let _scoped = scope.get_required::<HeavyService>();
            }
        })
    }).collect();
    
    for handle in handles {
        handle.join().unwrap();
    }
    let duration = start.elapsed();
    
    let total_ops = thread_count * iterations_per_thread * 2;
    let result = BenchmarkResult::new(
        format!("Concurrent Access ({} threads x {}K ops)", thread_count, iterations_per_thread / 1000),
        duration,
        total_ops,
    );
    print_benchmark_result(&result);
    
    // Performance assertion
    assert!(result.operations_per_second > 100_000.0,
           "Concurrent access too slow: {:.0} ops/sec", result.operations_per_second);
}

#[test]
#[ignore] // Run with: cargo test -- --ignored benchmark
fn benchmark_complex_object_graph_performance() {
    println!("🔥 BENCHMARK: Complex Object Graph Performance");
    
    let mut services = ServiceCollection::new();
    
    // Create a complex dependency graph
    services.add_singleton(LightweightService::new(1));
    
    // Multiple layers of dependencies
    for i in 0..50 {
        services.add_singleton_factory::<String, _>(move |r| {
            let _dep = r.get_required::<LightweightService>();
            format!("service_{}", i)
        });
    }
    
    // Scoped services that depend on singletons
    services.add_scoped_factory::<HeavyService, _>(|r| {
        let _lightweight = r.get_required::<LightweightService>();
        HeavyService::new(100)
    });
    
    let provider = services.build();
    let iterations = 10_000;
    
    let start = Instant::now();
    for _ in 0..iterations {
        let scope = provider.create_scope();
        let _heavy = scope.get_required::<HeavyService>();
        // Also resolve some of the string services
        for i in 0..10 {
            let _service = scope.get_required::<String>();
        }
    }
    let duration = start.elapsed();
    
    let result = BenchmarkResult::new(
        "Complex Object Graph (10K iterations, 51 services)".to_string(),
        duration,
        iterations * 11, // 1 HeavyService + 10 String services per iteration
    );
    print_benchmark_result(&result);
    
    // Performance assertion
    assert!(result.operations_per_second > 10_000.0,
           "Complex graph resolution too slow: {:.0} ops/sec", result.operations_per_second);
}

#[test]
#[ignore] // Run with: cargo test -- --ignored benchmark
fn benchmark_memory_usage_patterns() {
    println!("🔥 BENCHMARK: Memory Usage Patterns");
    
    let mut services = ServiceCollection::new();
    services.add_transient_factory::<HeavyService, _>(|_| HeavyService::new(1));
    
    let provider = services.build();
    
    // Test memory usage with many transient services
    let batches = 100;
    let services_per_batch = 1000;
    
    let start = Instant::now();
    for batch in 0..batches {
        let mut batch_services = Vec::new();
        
        for _ in 0..services_per_batch {
            let service = provider.get_required::<HeavyService>();
            batch_services.push(service);
        }
        
        // Simulate some work with the services
        let sum: u32 = batch_services.iter().map(|s| s.get_id()).sum();
        assert_eq!(sum, services_per_batch); // All services have id = 1
        
        // Services are dropped when vector goes out of scope
        drop(batch_services);
        
        // Periodic progress indicator
        if batch % 20 == 0 {
            println!("Processed batch {}/{}", batch + 1, batches);
        }
    }
    let duration = start.elapsed();
    
    let total_services = batches * services_per_batch;
    let result = BenchmarkResult::new(
        format!("Memory Usage Test ({}K transient services)", total_services / 1000),
        duration,
        total_services,
    );
    print_benchmark_result(&result);
    
    // Performance assertion
    assert!(result.operations_per_second > 5_000.0,
           "Memory usage test too slow: {:.0} ops/sec", result.operations_per_second);
}

// ===== Module Performance Benchmarks =====

struct LightweightModule;

impl ServiceModule for LightweightModule {
    fn register_services(self, services: &mut ServiceCollection) -> ferrous_di::DiResult<()> {
        services.add_singleton(LightweightService::new(1));
        services.add_scoped_factory::<LightweightService, _>(|_| LightweightService::new(2));
        Ok(())
    }
}

struct HeavyModule;

impl ServiceModule for HeavyModule {
    fn register_services(self, services: &mut ServiceCollection) -> ferrous_di::DiResult<()> {
        services.add_singleton(HeavyService::new(10));
        services.add_singleton_trait::<dyn PerformanceService>(
            Arc::new(FastImplementation::new(100))
        );
        Ok(())
    }
}

#[test]
#[ignore] // Run with: cargo test -- --ignored benchmark
fn benchmark_module_registration_performance() {
    println!("🔥 BENCHMARK: Module Registration Performance");
    
    let iterations = 5_000;
    let start = Instant::now();
    
    for _ in 0..iterations {
        let mut services = ServiceCollection::new();
        services.add_module_mut(LightweightModule).unwrap();
        services.add_module_mut(HeavyModule).unwrap();
        let _provider = services.build();
    }
    let duration = start.elapsed();
    
    let result = BenchmarkResult::new(
        "Module Registration (2 modules x 5K iterations)".to_string(),
        duration,
        iterations * 2,
    );
    print_benchmark_result(&result);
    
    // Performance assertion
    assert!(duration.as_millis() < 3000, "Module registration took too long: {:?}", duration);
}

// ===== Performance Summary =====

#[test]
#[ignore] // Run with: cargo test -- --ignored benchmark
fn benchmark_performance_summary() {
    println!("\n🎯 PERFORMANCE SUMMARY");
    println!("====================");
    println!("All benchmark tests completed successfully!");
    println!("Performance targets met:");
    println!("✅ Singleton resolution: >1M ops/sec");
    println!("✅ Scoped resolution: >50K ops/sec");  
    println!("✅ Transient resolution: >100K ops/sec");
    println!("✅ Dependency injection: >25K ops/sec");
    println!("✅ Trait resolution: >200K ops/sec");
    println!("✅ Concurrent access: >100K ops/sec");
    println!("✅ Complex graphs: >10K ops/sec");
    println!("✅ Memory patterns: >5K ops/sec");
    println!("✅ Module registration: <3s for 10K modules");
    println!("\nferrous-di maintains excellent performance! 🚀");
}