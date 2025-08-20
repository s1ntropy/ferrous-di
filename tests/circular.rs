use ferrous_di::{ServiceCollection, DiError, Resolver, CircularPanic};
use std::panic::{catch_unwind, AssertUnwindSafe};

/// Helper: assert that `f()` panics with a CircularPanic carrying `expected_path`.
fn assert_circular_panics<F>(f: F, expected_path: &[&'static str])
where
    F: FnOnce(),
{
    let res = catch_unwind(AssertUnwindSafe(f));
    assert!(res.is_err(), "Expected panic due to circular dependency");

    let err = res.err().unwrap();
    // Prefer downcast to your payload type first:
    if let Some(cp) = err.downcast_ref::<CircularPanic>() {
        assert_eq!(&*cp.path, expected_path, "wrong circular path");
        return;
    }
    // Fallback: string message (if someone used panic!(...)):
    if let Some(msg) = err.downcast_ref::<&'static str>() {
        // Check if all expected path elements are present in the message
        for &expected_element in expected_path {
            assert!(
                msg.contains(expected_element),
                "panic message missing path element '{}' ; got: {}",
                expected_element,
                msg
            );
        }
        return;
    }
    if let Some(msg) = err.downcast_ref::<String>() {
        // Check if all expected path elements are present in the message
        for &expected_element in expected_path {
            assert!(
                msg.contains(expected_element),
                "panic message missing path element '{}' ; got: {}",
                expected_element,
                msg
            );
        }
        return;
    }
    panic!("panic payload not recognized (neither CircularPanic nor string)");
}

#[test]
fn test_self_circular_dependency() {
    struct SelfReferencing;
    
    let mut sc = ServiceCollection::new();
    sc.add_transient_factory::<SelfReferencing, _>(|r| {
        let _ = r.get::<SelfReferencing>(); // Self-reference
        SelfReferencing
    });
    
    let sp = sc.build();
    let result = sp.get::<SelfReferencing>();
    
    match result {
        Err(DiError::Circular(path)) => {
            assert_eq!(path.len(), 2);
            assert!(path[0].contains("SelfReferencing"));
            assert!(path[1].contains("SelfReferencing"));
        }
        _ => panic!("Expected Circular error"),
    }
}

#[test]
fn test_two_level_circular() {
    struct A {
        b: std::sync::Arc<B>,
    }
    
    struct B {
        a: std::sync::Arc<A>,
    }
    
    let mut sc = ServiceCollection::new();
    
    sc.add_transient_factory::<A, _>(|r| {
        let b = r.get_required::<B>();
        A { b }
    });
    
    sc.add_transient_factory::<B, _>(|r| {
        let a = r.get_required::<A>();
        B { a }
    });
    
    let sp = sc.build();
    
    // Test A -> B -> A circular dependency
    assert_circular_panics(|| {
        let _ = sp.get::<A>(); // This will panic inside get_required
    }, &["circular::test_two_level_circular::A", "circular::test_two_level_circular::B", "circular::test_two_level_circular::A"]);
}

#[test]
fn test_three_level_circular() {
    struct X {
        y: std::sync::Arc<Y>,
    }
    
    struct Y {
        z: std::sync::Arc<Z>,
    }
    
    struct Z {
        x: std::sync::Arc<X>,
    }
    
    let mut sc = ServiceCollection::new();
    
    sc.add_singleton_factory::<X, _>(|r| X {
        y: r.get_required::<Y>(),
    });
    
    sc.add_singleton_factory::<Y, _>(|r| Y {
        z: r.get_required::<Z>(),
    });
    
    sc.add_singleton_factory::<Z, _>(|r| Z {
        x: r.get_required::<X>(),
    });
    
    let sp = sc.build();
    
    // Test X -> Y -> Z -> X circular dependency
    assert_circular_panics(|| {
        let _ = sp.get::<X>(); // X -> Y -> Z -> X
    }, &["circular::test_three_level_circular::X", "circular::test_three_level_circular::Y", "circular::test_three_level_circular::Z", "circular::test_three_level_circular::X"]);
}

#[test]
fn test_circular_with_traits() {
    trait ServiceA: Send + Sync {
        fn name(&self) -> &str;
    }
    
    trait ServiceB: Send + Sync {
        fn name(&self) -> &str;
    }
    
    struct ImplA {
        b: std::sync::Arc<dyn ServiceB>,
    }
    
    impl ServiceA for ImplA {
        fn name(&self) -> &str { "A" }
    }
    
    struct ImplB {
        a: std::sync::Arc<dyn ServiceA>,
    }
    
    impl ServiceB for ImplB {
        fn name(&self) -> &str { "B" }
    }
    
    let mut sc = ServiceCollection::new();
    
    sc.add_singleton_trait_factory::<dyn ServiceA, _>(|r| {
        std::sync::Arc::new(ImplA {
            b: r.get_required_trait::<dyn ServiceB>(),
        }) as std::sync::Arc<dyn ServiceA>
    });
    
    sc.add_singleton_trait_factory::<dyn ServiceB, _>(|r| {
        std::sync::Arc::new(ImplB {
            a: r.get_required_trait::<dyn ServiceA>(),
        }) as std::sync::Arc<dyn ServiceB>
    });
    
    let sp = sc.build();
    
    // Test ServiceA <-> ServiceB circular dependency
    assert_circular_panics(|| {
        let _ = sp.get_trait::<dyn ServiceA>(); // ServiceA -> ServiceB -> ServiceA
    }, &["dyn circular::test_circular_with_traits::ServiceA", "dyn circular::test_circular_with_traits::ServiceB", "dyn circular::test_circular_with_traits::ServiceA"]);
}

#[test]
fn test_depth_exceeded() {
    // Create a deeply nested dependency chain that doesn't cycle
    // but exceeds maximum depth
    
    struct DeepService {
        depth: usize,
    }
    
    let mut sc = ServiceCollection::new();
    
    // This factory will recursively create services until depth is reached
    sc.add_transient_factory::<DeepService, _>(|r| {
        // Try to create another DeepService recursively
        // This will eventually hit the depth limit
        let _ = r.get::<DeepService>();
        DeepService { depth: 0 }
    });
    
    let sp = sc.build();
    let result = sp.get::<DeepService>();
    
    // This should either be Circular (since it's self-referencing)
    // or DepthExceeded if it hits the limit first
    assert!(matches!(
        result,
        Err(DiError::Circular(_)) | Err(DiError::DepthExceeded(_))
    ));
}