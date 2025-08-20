//! Circular dependency detection infrastructure.

use std::cell::RefCell;
use std::panic;

const MAX_DEPTH: usize = 1024;

// Thread-local resolution state for circular dependency detection
thread_local! {
    static RESOLUTION_TLS: RefCell<ResolutionTls> = RefCell::new(ResolutionTls::default());
}

#[derive(Default)]
struct ResolutionTls {
    stack: Vec<&'static str>,
    frozen: bool,
    depth: usize,
}

/// Panic payload for circular dependency detection.
///
/// When a circular dependency is detected during service resolution,
/// this panic payload carries the complete dependency path for debugging.
///
/// Example path: `["ServiceA", "ServiceB", "ServiceC", "ServiceA"]`
#[derive(Debug)]
pub struct CircularPanic {
    /// The complete circular dependency path showing the cycle.
    pub path: Box<[&'static str]>,
}

impl CircularPanic {
    fn new(path: Vec<&'static str>) -> Self {
        CircularPanic { path: path.into_boxed_slice() }
    }
}

/// Guard for managing thread-local resolution stack
pub(crate) struct StackGuard {
    name: &'static str,
}

impl StackGuard {
    pub(crate) fn new(name: &'static str) -> Self {
        RESOLUTION_TLS.with(|tls| {
            let mut tls = tls.borrow_mut();

            // Circular detection BEFORE pushing the new name
            if tls.stack.iter().any(|&n| n == name) {
                let mut path = tls.stack.clone();
                path.push(name);
                tls.frozen = true; // freeze pops during unwind
                panic::panic_any(CircularPanic::new(path));
            }

            // Depth guard
            if tls.depth >= MAX_DEPTH {
                panic::panic_any(crate::error::DiError::DepthExceeded(tls.depth));
            }

            tls.stack.push(name);
            tls.depth += 1;
        });

        Self { name }
    }
}

impl Drop for StackGuard {
    fn drop(&mut self) {
        RESOLUTION_TLS.with(|tls| {
            let mut tls = tls.borrow_mut();
            if !tls.frozen {
                if let Some(last) = tls.stack.pop() {
                    debug_assert_eq!(last, self.name);
                }
                tls.depth = tls.depth.saturating_sub(1);
            }
        });
    }
}

/// Execute a closure with circular dependency detection
pub(crate) fn with_circular_catch<T, F>(name: &'static str, f: F) -> crate::error::DiResult<T>
where
    F: FnOnce() -> crate::error::DiResult<T>,
{
    use std::panic::AssertUnwindSafe;
    
    let _guard = StackGuard::new(name);
    
    // Wrap in catch_unwind to handle CircularPanic
    match std::panic::catch_unwind(AssertUnwindSafe(|| f())) {
        Ok(result) => result,
        Err(payload) => {
            if let Some(circular_panic) = payload.downcast_ref::<CircularPanic>() {
                Err(crate::error::DiError::Circular(circular_panic.path.iter().copied().collect()))
            } else {
                // Re-panic for other types of panics
                std::panic::resume_unwind(payload);
            }
        }
    }
}