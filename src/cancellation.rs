//! Cancellation token support for workflow engines.
//!
//! This module provides cancellation token primitives that are essential
//! for n8n-style workflow engines where nodes need abort capabilities.

use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
use std::time::{Duration, Instant};

/// A token that can be used to signal cancellation across async operations.
///
/// Essential for workflow engines where nodes need abort capabilities.
/// The token is designed to be DI-visible and propagate through scopes.
///
/// # Examples
///
/// ```
/// use ferrous_di::{ServiceCollection, CancellationToken, Resolver};
/// use std::sync::Arc;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let mut services = ServiceCollection::new();
/// 
/// // Register cancellation token as scoped resource
/// services.add_scoped_factory::<CancellationToken, _>(|_| {
///     CancellationToken::new()
/// });
///
/// let provider = services.build();
/// let scope = provider.create_scope();
///
/// let cancel_token = scope.get_required::<CancellationToken>();
/// 
/// // Check if cancelled
/// if cancel_token.is_cancelled() {
///     return Err("Operation cancelled".into());
/// }
/// 
/// # #[cfg(feature = "async")]
/// // Use in async operation with tokio
/// tokio::select! {
///     result = some_long_operation() => {
///         // Operation completed
///     }
///     _ = cancel_token.cancelled() => {
///         // Operation was cancelled
///         return Err("Operation cancelled".into());
///     }
/// }
/// # Ok(())
/// # }
/// 
/// # async fn some_long_operation() {}
/// ```
#[derive(Clone)]
pub struct CancellationToken {
    inner: Arc<CancellationTokenInner>,
}

struct CancellationTokenInner {
    cancelled: AtomicBool,
    parent: Option<CancellationToken>,
    created_at: Instant,
}

impl CancellationToken {
    /// Creates a new cancellation token.
    pub fn new() -> Self {
        Self {
            inner: Arc::new(CancellationTokenInner {
                cancelled: AtomicBool::new(false),
                parent: None,
                created_at: Instant::now(),
            })
        }
    }

    /// Creates a child token that will be cancelled when either this token
    /// or the parent token is cancelled.
    ///
    /// Perfect for hierarchical workflow cancellation (flow → run → node).
    ///
    /// # Examples
    ///
    /// ```
    /// use ferrous_di::CancellationToken;
    ///
    /// let parent_token = CancellationToken::new();
    /// let child_token = parent_token.child_token();
    ///
    /// parent_token.cancel();
    /// assert!(child_token.is_cancelled());
    /// ```
    pub fn child_token(&self) -> Self {
        Self {
            inner: Arc::new(CancellationTokenInner {
                cancelled: AtomicBool::new(false),
                parent: Some(self.clone()),
                created_at: Instant::now(),
            })
        }
    }

    /// Cancels the token, signaling that associated operations should stop.
    pub fn cancel(&self) {
        self.inner.cancelled.store(true, Ordering::Release);
    }

    /// Returns true if cancellation has been requested.
    pub fn is_cancelled(&self) -> bool {
        // Check self first
        if self.inner.cancelled.load(Ordering::Acquire) {
            return true;
        }
        
        // Check parent chain
        if let Some(ref parent) = self.inner.parent {
            return parent.is_cancelled();
        }
        
        false
    }

    /// Throws a cancellation error if the token is cancelled.
    ///
    /// # Errors
    ///
    /// Returns `Err` with a cancellation message if the token is cancelled.
    pub fn throw_if_cancelled(&self) -> Result<(), CancellationError> {
        if self.is_cancelled() {
            Err(CancellationError::new("Operation was cancelled"))
        } else {
            Ok(())
        }
    }

    /// Returns a future that completes when cancellation is requested.
    ///
    /// Perfect for use with `tokio::select!` to race against operations.
    ///
    /// # Examples
    ///
    /// ```
    /// use ferrous_di::CancellationToken;
    ///
    /// # async fn example() {
    /// let token = CancellationToken::new();
    /// 
    /// # #[cfg(feature = "async")]
    /// tokio::select! {
    ///     result = some_operation() => {
    ///         // Operation completed normally
    ///     }
    ///     _ = token.cancelled() => {
    ///         // Operation was cancelled
    ///     }
    /// }
    /// # }
    /// 
    /// # async fn some_operation() {}
    /// ```
    #[cfg(feature = "async")]
    pub async fn cancelled(&self) {
        loop {
            if self.is_cancelled() {
                return;
            }
            
            // Small delay to avoid busy waiting
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
    }

    /// Returns the elapsed time since this token was created.
    ///
    /// Useful for timeout-based cancellation in workflow engines.
    pub fn elapsed(&self) -> Duration {
        self.inner.created_at.elapsed()
    }

    /// Creates a token that will automatically cancel after the specified duration.
    ///
    /// Perfect for implementing timeouts in workflow nodes.
    ///
    /// # Examples
    ///
    /// ```
    /// use ferrous_di::CancellationToken;
    /// use std::time::Duration;
    ///
    /// # async fn example() {
    /// let token = CancellationToken::with_timeout(Duration::from_secs(30));
    /// 
    /// // Token will automatically cancel after 30 seconds
    /// # #[cfg(feature = "async")]
    /// tokio::select! {
    ///     result = long_running_operation() => {
    ///         // Completed within timeout
    ///     }
    ///     _ = token.cancelled() => {
    ///         // Timed out after 30 seconds
    ///     }
    /// }
    /// # }
    /// 
    /// # async fn long_running_operation() {}
    /// ```
    #[cfg(feature = "async")]
    pub fn with_timeout(timeout: Duration) -> Self {
        let token = Self::new();
        let token_clone = token.clone();
        
        tokio::spawn(async move {
            tokio::time::sleep(timeout).await;
            token_clone.cancel();
        });
        
        token
    }
}

impl Default for CancellationToken {
    fn default() -> Self {
        Self::new()
    }
}

/// Error type for cancellation operations.
#[derive(Debug, Clone)]
pub struct CancellationError {
    message: String,
}

impl CancellationError {
    /// Creates a new cancellation error with the given message.
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl std::fmt::Display for CancellationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Cancellation error: {}", self.message)
    }
}

impl std::error::Error for CancellationError {}

/// Extension trait for Scope to easily create child scopes with cancellation.
pub trait ScopeCancellationExt {
    /// Creates a child scope with a cancellation token derived from the parent scope.
    ///
    /// Perfect for n8n-style hierarchical cancellation (workflow → run → node).
    ///
    /// # Examples
    ///
    /// ```
    /// use ferrous_di::{ServiceCollection, CancellationToken, ScopeCancellationExt, Resolver};
    ///
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut services = ServiceCollection::new();
    /// services.add_scoped_factory::<CancellationToken, _>(|_| CancellationToken::new());
    ///
    /// let provider = services.build();
    /// let parent_scope = provider.create_scope();
    /// 
    /// // Child scope inherits cancellation from parent
    /// let child_scope = parent_scope.with_cancellation_from_parent();
    /// 
    /// let parent_token = parent_scope.get_required::<CancellationToken>();
    /// let child_token = child_scope.get_required::<CancellationToken>();
    /// 
    /// parent_token.cancel();
    /// assert!(child_token.is_cancelled()); // Child is cancelled when parent is
    /// # Ok(())
    /// # }
    /// ```
    fn with_cancellation_from_parent(&self) -> Self;
}

impl ScopeCancellationExt for crate::provider::Scope {
    fn with_cancellation_from_parent(&self) -> Self {
        use std::sync::Arc;
        use crate::traits::Resolver;
        
        // Create a child scope
        let child_scope = self.create_child();
        
        // Get parent cancellation token if it exists
        let parent_token = self.get::<CancellationToken>().unwrap_or_else(|_| {
            // No parent token, create a new root token
            Arc::new(CancellationToken::new())
        });
        
        // Create child token that will be cancelled when parent is cancelled
        let child_token = parent_token.child_token();
        
        // We need to inject the child token into the child scope
        // Since we can't modify the service registration after the provider is built,
        // we'll use a different approach: manually cache the token in the scope
        
        // Store the child token in the child scope's scoped storage
        // This leverages the existing scoped caching mechanism
        let token_key = crate::key::key_of_type::<CancellationToken>();
        
        #[cfg(feature = "once-cell")]
        {
            // Find the slot for CancellationToken in the registry
            if let Some(reg) = child_scope.root.inner().registry.registrations.get(&token_key) {
                if let Some(slot) = reg.scoped_slot {
                    // Initialize the slot with our child token
                    let _ = child_scope.scoped_cells[slot].set(Arc::new(child_token) as crate::registration::AnyArc);
                }
            }
        }
        
        #[cfg(not(feature = "once-cell"))]
        {
            // Use HashMap-based scoped storage
            let mut scoped = child_scope.scoped.lock().unwrap();
            scoped.insert(token_key, Arc::new(child_token) as crate::registration::AnyArc);
        }
        
        child_scope
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_cancellation_token_basic() {
        let token = CancellationToken::new();
        assert!(!token.is_cancelled());
        
        token.cancel();
        assert!(token.is_cancelled());
    }

    #[test]
    fn test_child_token_cancellation() {
        let parent = CancellationToken::new();
        let child = parent.child_token();
        
        assert!(!parent.is_cancelled());
        assert!(!child.is_cancelled());
        
        parent.cancel();
        assert!(parent.is_cancelled());
        assert!(child.is_cancelled());
    }

    #[test]
    fn test_child_token_independent_cancellation() {
        let parent = CancellationToken::new();
        let child = parent.child_token();
        
        child.cancel();
        assert!(!parent.is_cancelled());
        assert!(child.is_cancelled());
    }

    #[test]
    fn test_throw_if_cancelled() {
        let token = CancellationToken::new();
        
        // Should not throw when not cancelled
        assert!(token.throw_if_cancelled().is_ok());
        
        token.cancel();
        
        // Should throw when cancelled
        assert!(token.throw_if_cancelled().is_err());
    }

    #[cfg(feature = "async")]
    #[tokio::test]
    async fn test_timeout_cancellation() {
        let token = CancellationToken::with_timeout(Duration::from_millis(10));
        
        assert!(!token.is_cancelled());
        
        // Wait for timeout
        tokio::time::sleep(Duration::from_millis(20)).await;
        
        assert!(token.is_cancelled());
    }

    #[cfg(feature = "async")]
    #[tokio::test]
    async fn test_cancelled_future() {
        let token = CancellationToken::new();
        let token_clone = token.clone();
        
        // Spawn task to cancel token after delay
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(10)).await;
            token_clone.cancel();
        });
        
        // Wait for cancellation
        token.cancelled().await;
        assert!(token.is_cancelled());
    }

    #[test]
    fn test_scope_cancellation_ext() {
        use crate::{ServiceCollection, traits::Resolver};
        
        let mut services = ServiceCollection::new();
        services.add_scoped_factory::<CancellationToken, _>(|_| CancellationToken::new());
        
        let provider = services.build();
        let parent_scope = provider.create_scope();
        
        // Create child scope with inherited cancellation
        let child_scope = parent_scope.with_cancellation_from_parent();
        
        let parent_token = parent_scope.get_required::<CancellationToken>();
        let child_token = child_scope.get_required::<CancellationToken>();
        
        // Initially neither should be cancelled
        assert!(!parent_token.is_cancelled());
        assert!(!child_token.is_cancelled());
        
        // Cancel parent - child should also be cancelled
        parent_token.cancel();
        assert!(parent_token.is_cancelled());
        assert!(child_token.is_cancelled());
    }
}