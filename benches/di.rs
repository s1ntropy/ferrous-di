use criterion::{criterion_group, criterion_main, Criterion, black_box, BenchmarkId};
use ferrous_di::*;
use std::sync::Arc;

// ===== Micro Benchmarks =====

fn bench_singleton_hit(c: &mut Criterion) {
    let mut sc = ServiceCollection::new();
    sc.add_singleton(42u64);
    let sp = sc.build();
    
    // Prime the singleton
    let _ = sp.get::<u64>().unwrap();
    
    c.bench_function("singleton_hit_u64", |b| {
        b.iter(|| {
            let v = sp.get::<u64>().unwrap();
            black_box(v);
        })
    });
}

fn bench_singleton_cold(c: &mut Criterion) {
    struct ExpensiveToCreate {
        data: Vec<u64>,
    }
    
    c.bench_function("singleton_cold_expensive", |b| {
        b.iter_batched(
            || {
                let mut sc = ServiceCollection::new();
                sc.add_singleton_factory::<ExpensiveToCreate, _>(|_| {
                    ExpensiveToCreate {
                        data: (0..1000).collect(),
                    }
                });
                sc.build()
            },
            |sp| {
                let v = sp.get::<ExpensiveToCreate>().unwrap();
                black_box(v.data.len());
            },
            criterion::BatchSize::SmallInput,
        )
    });
}

fn bench_scoped_vs_transient(c: &mut Criterion) {
    #[derive(Clone)]
    struct Service {
        data: [u8; 64],
    }
    
    let mut group = c.benchmark_group("scoped_vs_transient");
    
    // Scoped service
    let mut sc_scoped = ServiceCollection::new();
    sc_scoped.add_scoped_factory::<Service, _>(|_| Service { data: [0; 64] });
    let sp_scoped = sc_scoped.build();
    let scope = sp_scoped.create_scope();
    
    group.bench_function("scoped_hit", |b| {
        b.iter(|| {
            let v = scope.get::<Service>().unwrap();
            black_box(&v.data);
        })
    });
    
    // Transient service
    let mut sc_transient = ServiceCollection::new();
    sc_transient.add_transient_factory::<Service, _>(|_| Service { data: [0; 64] });
    let sp_transient = sc_transient.build();
    
    group.bench_function("transient", |b| {
        b.iter(|| {
            let v = sp_transient.get::<Service>().unwrap();
            black_box(&v.data);
        })
    });
    
    group.finish();
}

fn bench_concrete_vs_trait(c: &mut Criterion) {
    trait MyTrait: Send + Sync {
        fn value(&self) -> u64;
    }
    
    struct ConcreteImpl {
        val: u64,
    }
    
    impl MyTrait for ConcreteImpl {
        fn value(&self) -> u64 {
            self.val
        }
    }
    
    let mut group = c.benchmark_group("concrete_vs_trait");
    
    // Concrete type
    let mut sc_concrete = ServiceCollection::new();
    sc_concrete.add_singleton(ConcreteImpl { val: 42 });
    let sp_concrete = sc_concrete.build();
    
    group.bench_function("concrete", |b| {
        b.iter(|| {
            let v = sp_concrete.get::<ConcreteImpl>().unwrap();
            black_box(v.val);
        })
    });
    
    // Trait object
    let mut sc_trait = ServiceCollection::new();
    sc_trait.add_singleton_trait(Arc::new(ConcreteImpl { val: 42 }) as Arc<dyn MyTrait>);
    let sp_trait = sc_trait.build();
    
    group.bench_function("trait_single", |b| {
        b.iter(|| {
            let v = sp_trait.get_trait::<dyn MyTrait>().unwrap();
            black_box(v.value());
        })
    });
    
    group.finish();
}

fn bench_multi_binding_scaling(c: &mut Criterion) {
    trait Handler: Send + Sync {
        fn id(&self) -> usize;
    }
    
    struct HandlerImpl(usize);
    impl Handler for HandlerImpl {
        fn id(&self) -> usize {
            self.0
        }
    }
    
    let mut group = c.benchmark_group("multi_binding");
    
    for &count in &[1, 4, 16, 64] {
        let mut sc = ServiceCollection::new();
        for i in 0..count {
            sc.add_trait_implementation(
                Arc::new(HandlerImpl(i)) as Arc<dyn Handler>,
                Lifetime::Singleton,
            );
        }
        let sp = sc.build();
        
        group.bench_with_input(BenchmarkId::new("get_all", count), &count, |b, _| {
            b.iter(|| {
                let handlers = sp.get_all_trait::<dyn Handler>().unwrap();
                black_box(handlers.len());
            })
        });
    }
    
    group.finish();
}

fn bench_scope_lifecycle(c: &mut Criterion) {
    struct ScopedService {
        data: Vec<u8>,
    }
    
    let mut group = c.benchmark_group("scope_lifecycle");
    
    // Empty scope
    let sc_empty = ServiceCollection::new();
    let sp_empty = sc_empty.build();
    
    group.bench_function("empty_scope_create_drop", |b| {
        b.iter(|| {
            let scope = sp_empty.create_scope();
            black_box(&scope);
        })
    });
    
    // Scope with service
    let mut sc_with_service = ServiceCollection::new();
    sc_with_service.add_scoped_factory::<ScopedService, _>(|_| ScopedService {
        data: vec![0; 1024],
    });
    let sp_with_service = sc_with_service.build();
    
    group.bench_function("scope_with_service", |b| {
        b.iter(|| {
            let scope = sp_with_service.create_scope();
            let _service = scope.get::<ScopedService>().unwrap();
            black_box(&scope);
        })
    });
    
    group.finish();
}

fn bench_using_pattern_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("using_pattern");
    
    let sc = ServiceCollection::new();
    let sp = sc.build();
    
    // Empty bag
    group.bench_function("using_empty", |b| {
        b.iter(|| {
            let _ = sp.create_scope().using_sync(|_scope| {
                black_box(42);
                Ok::<(), DiError>(())
            });
        })
    });
    
    // With disposers
    struct DisposableService {
        _data: Vec<u8>,
    }
    
    impl Dispose for DisposableService {
        fn dispose(&self) {
            // Simulate cleanup work
            black_box(&self._data);
        }
    }
    
    let mut sc_disposable = ServiceCollection::new();
    sc_disposable.add_scoped_factory::<DisposableService, _>(|_| DisposableService {
        _data: vec![0; 1024],
    });
    let sp_disposable = sc_disposable.build();
    
    group.bench_function("using_with_10_disposers", |b| {
        b.iter(|| {
            let _ = sp_disposable.create_scope().using_sync(|scope| {
                let mut services = Vec::new();
                for _ in 0..10 {
                    let service = scope.get::<DisposableService>().unwrap();
                    services.push(service);
                }
                black_box(services.len());
                Ok::<(), DiError>(())
            });
        })
    });
    
    group.finish();
}

fn bench_circular_detection_depth(c: &mut Criterion) {
    let mut group = c.benchmark_group("circular_detection");
    
    // Non-circular chain of depth 8
    struct Service1;
    struct Service2 { _s1: Arc<Service1> }
    struct Service3 { _s2: Arc<Service2> }
    struct Service4 { _s3: Arc<Service3> }
    struct Service5 { _s4: Arc<Service4> }
    struct Service6 { _s5: Arc<Service5> }
    struct Service7 { _s6: Arc<Service6> }
    struct Service8 { _s7: Arc<Service7> }
    
    let mut sc = ServiceCollection::new();
    sc.add_singleton(Service1);
    sc.add_singleton_factory::<Service2, _>(|r| Service2 { _s1: r.get_required() });
    sc.add_singleton_factory::<Service3, _>(|r| Service3 { _s2: r.get_required() });
    sc.add_singleton_factory::<Service4, _>(|r| Service4 { _s3: r.get_required() });
    sc.add_singleton_factory::<Service5, _>(|r| Service5 { _s4: r.get_required() });
    sc.add_singleton_factory::<Service6, _>(|r| Service6 { _s5: r.get_required() });
    sc.add_singleton_factory::<Service7, _>(|r| Service7 { _s6: r.get_required() });
    sc.add_singleton_factory::<Service8, _>(|r| Service8 { _s7: r.get_required() });
    let sp = sc.build();
    
    group.bench_function("chain_depth_8", |b| {
        b.iter(|| {
            let service = sp.get::<Service8>().unwrap();
            black_box(&service);
        })
    });
    
    group.finish();
}

fn bench_contention(c: &mut Criterion) {
    let mut group = c.benchmark_group("contention");
    
    let mut sc = ServiceCollection::new();
    sc.add_singleton(42u64);
    let sp = sc.build();
    
    // Prime the singleton
    let _ = sp.get::<u64>().unwrap();
    
    for &thread_count in &[1, 2, 4, 8] {
        group.bench_with_input(
            BenchmarkId::new("singleton_threads", thread_count),
            &thread_count,
            |b, &threads| {
                b.iter_custom(|iters| {
                    let start = std::time::Instant::now();
                    crossbeam_utils::thread::scope(|s| {
                        for _ in 0..threads {
                            let sp_ref = &sp;
                            s.spawn(move |_| {
                                for _ in 0..iters / threads as u64 {
                                    let v = sp_ref.get::<u64>().unwrap();
                                    black_box(v);
                                }
                            });
                        }
                    }).unwrap();
                    start.elapsed()
                })
            },
        );
    }
    
    group.finish();
}

// ===== Macro Benchmarks =====

fn bench_large_registry(c: &mut Criterion) {
    let mut group = c.benchmark_group("large_registry");
    
    for &service_count in &[10, 100, 1000] {
        // Use different types to avoid key conflicts
        let mut sc = ServiceCollection::new();
        
        // Register a baseline service we'll always resolve
        sc.add_singleton(42u64);
        
        // Register many other services of different types to simulate large registry
        for i in 0..service_count {
            let value = i as u32;
            sc.add_singleton_factory::<u32, _>(move |_| value);
            // This will overwrite previous u32 registrations, simulating registry size
        }
        
        let sp = sc.build();
        
        group.bench_with_input(
            BenchmarkId::new("resolve_from_large_registry", service_count),
            &service_count,
            |b, _| {
                b.iter(|| {
                    // Resolve the u64 from a registry with many u32 entries
                    let v = sp.get::<u64>().unwrap();
                    black_box(v);
                })
            },
        );
    }
    
    group.finish();
}

fn bench_mixed_workload(c: &mut Criterion) {
    // Simulate realistic workload: 70% singleton hits, 20% scoped hits, 10% transient
    struct SingletonService(u64);
    struct ScopedService(u64);
    struct TransientService(u64);
    
    let mut sc = ServiceCollection::new();
    sc.add_singleton(SingletonService(1));
    sc.add_scoped_factory::<ScopedService, _>(|_| ScopedService(2));
    sc.add_transient_factory::<TransientService, _>(|_| TransientService(3));
    
    let sp = sc.build();
    let scope = sp.create_scope();
    
    // Prime services
    let _ = sp.get::<SingletonService>().unwrap();
    let _ = scope.get::<ScopedService>().unwrap();
    
    c.bench_function("mixed_workload_realistic", |b| {
        b.iter(|| {
            // 70% singleton hits
            for _ in 0..7 {
                let v = sp.get::<SingletonService>().unwrap();
                black_box(v.0);
            }
            
            // 20% scoped hits
            for _ in 0..2 {
                let v = scope.get::<ScopedService>().unwrap();
                black_box(v.0);
            }
            
            // 10% transient
            let v = sp.get::<TransientService>().unwrap();
            black_box(v.0);
        })
    });
}

criterion_group!(
    micro_benches,
    bench_singleton_hit,
    bench_singleton_cold,
    bench_scoped_vs_transient,
    bench_concrete_vs_trait,
    bench_multi_binding_scaling,
    bench_scope_lifecycle,
    bench_using_pattern_overhead,
    bench_circular_detection_depth,
    bench_contention
);

criterion_group!(
    macro_benches,
    bench_large_registry,
    bench_mixed_workload
);

criterion_main!(micro_benches, macro_benches);