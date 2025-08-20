//! Internal disposal bag for managing cleanup hooks.

use std::future::Future;
use std::pin::Pin;

/// Future type for disposal operations.
pub(crate) type BoxFutureUnit = Pin<Box<dyn Future<Output = ()> + Send>>;

/// Container for disposal hooks with LIFO execution order.
///
/// This internal structure manages both synchronous and asynchronous disposal hooks.
/// Async hooks are executed first (in reverse order), followed by sync hooks.
#[derive(Default)]
pub(crate) struct DisposeBag {
    sync: Vec<Box<dyn FnOnce() + Send>>,
    asyncs: Vec<Box<dyn FnOnce() -> BoxFutureUnit + Send>>,
}

impl DisposeBag {
    /// Add a synchronous disposal hook.
    pub(crate) fn push_sync(&mut self, f: Box<dyn FnOnce() + Send>) {
        self.sync.push(f);
    }
    
    /// Add an asynchronous disposal hook.
    pub(crate) fn push_async<Fut, F>(&mut self, f: F)
    where
        Fut: Future<Output = ()> + Send + 'static,
        F: FnOnce() -> Fut + Send + 'static,
    {
        self.asyncs.push(Box::new(move || Box::pin(f())));
    }

    /// Execute all sync hooks in reverse order (LIFO).
    pub(crate) fn run_all_sync_reverse(&mut self) {
        while let Some(f) = self.sync.pop() {
            (f)();
        }
    }

    /// Execute all async hooks in reverse order (LIFO).
    pub(crate) async fn run_all_async_reverse(&mut self) {
        while let Some(f) = self.asyncs.pop() {
            (f)().await;
        }
    }

    /// Check if the bag is empty (no disposers registered).
    pub(crate) fn is_empty(&self) -> bool {
        self.sync.is_empty() && self.asyncs.is_empty()
    }
}