//! Async lifecycle management for ferrous-di.
//!
//! This module provides async initialization, disposal, and lifecycle hooks
//! for services in async contexts.

use async_trait::async_trait;
use std::any::Any;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};

use crate::{DiResult, DiError, Key};

/// Trait for async service initialization
#[async_trait]
pub trait AsyncInitializable: Send + Sync {
    /// Initialize the service asynchronously
    async fn initialize(&self) -> DiResult<()>;
}

/// Trait for async service disposal
#[async_trait]
pub trait AsyncDisposable: Send + Sync {
    /// Dispose the service asynchronously
    async fn dispose(self: Arc<Self>) -> DiResult<()>;
}

/// Trait for services with async startup logic
#[async_trait]
pub trait AsyncStartup: Send + Sync {
    /// Perform startup operations
    async fn startup(&self) -> DiResult<()>;
}

/// Trait for services with async shutdown logic
#[async_trait]
pub trait AsyncShutdown: Send + Sync {
    /// Perform shutdown operations
    async fn shutdown(&self) -> DiResult<()>;
}

/// Async lifecycle manager for coordinating service lifecycles
pub struct AsyncLifecycleManager {
    /// Services registered for lifecycle management
    services: RwLock<HashMap<Key, Arc<dyn Any + Send + Sync>>>,
    /// Initializable services
    initializables: RwLock<HashMap<Key, Arc<dyn AsyncInitializable>>>,
    /// Startup services
    startups: RwLock<HashMap<Key, Arc<dyn AsyncStartup>>>,
    /// Shutdown services
    shutdowns: RwLock<HashMap<Key, Arc<dyn AsyncShutdown>>>,
    /// Disposable services
    disposables: RwLock<HashMap<Key, Arc<dyn AsyncDisposable>>>,
    /// Initialization order (topologically sorted)
    init_order: RwLock<Vec<Key>>,
    /// Disposal order (reverse of init order)
    disposal_order: RwLock<Vec<Key>>,
    /// Lifecycle state
    state: Arc<Mutex<LifecycleState>>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum LifecycleState {
    Created,
    Initializing,
    Initialized,
    Starting,
    Started,
    Stopping,
    Stopped,
    Disposing,
    Disposed,
}

impl AsyncLifecycleManager {
    /// Create a new async lifecycle manager
    pub fn new() -> Self {
        Self {
            services: RwLock::new(HashMap::new()),
            initializables: RwLock::new(HashMap::new()),
            startups: RwLock::new(HashMap::new()),
            shutdowns: RwLock::new(HashMap::new()),
            disposables: RwLock::new(HashMap::new()),
            init_order: RwLock::new(Vec::new()),
            disposal_order: RwLock::new(Vec::new()),
            state: Arc::new(Mutex::new(LifecycleState::Created)),
        }
    }

    /// Register a service for lifecycle management
    pub async fn register<T>(&self, key: Key, service: Arc<T>)
    where
        T: Any + Send + Sync + 'static,
    {
        // Store the service
        let mut services = self.services.write().await;
        services.insert(key.clone(), service as Arc<dyn Any + Send + Sync>);
        
        // Add to initialization order
        let mut init_order = self.init_order.write().await;
        init_order.push(key.clone());
        
        // Add to disposal order (reverse)
        let mut disposal_order = self.disposal_order.write().await;
        disposal_order.insert(0, key);
    }

    /// Register a service that implements AsyncInitializable
    pub async fn register_initializable(&self, key: Key, initializable: Arc<dyn AsyncInitializable>) {
        let mut initializables = self.initializables.write().await;
        initializables.insert(key, initializable);
    }

    /// Register a service that implements AsyncStartup
    pub async fn register_startup(&self, key: Key, startup: Arc<dyn AsyncStartup>) {
        let mut startups = self.startups.write().await;
        startups.insert(key, startup);
    }

    /// Register a service that implements AsyncShutdown
    pub async fn register_shutdown(&self, key: Key, shutdown: Arc<dyn AsyncShutdown>) {
        let mut shutdowns = self.shutdowns.write().await;
        shutdowns.insert(key, shutdown);
    }

    /// Register a service that implements AsyncDisposable
    pub async fn register_disposable(&self, key: Key, disposable: Arc<dyn AsyncDisposable>) {
        let mut disposables = self.disposables.write().await;
        disposables.insert(key, disposable);
    }

    /// Initialize all registered services
    pub async fn initialize_all(&self) -> DiResult<()> {
        let mut state = self.state.lock().await;
        if *state != LifecycleState::Created {
            return Err(DiError::WrongLifetime("Already initialized"));
        }
        *state = LifecycleState::Initializing;
        drop(state);

        let init_order = self.init_order.read().await;
        let services = self.services.read().await;

        for key in init_order.iter() {
            // Check if we have an initializable service registered for this key
            if let Some(initializable) = self.initializables.read().await.get(key) {
                initializable.initialize().await?;
            }
        }

        let mut state = self.state.lock().await;
        *state = LifecycleState::Initialized;
        Ok(())
    }

    /// Start all registered services
    pub async fn startup_all(&self) -> DiResult<()> {
        let mut state = self.state.lock().await;
        if *state != LifecycleState::Initialized {
            return Err(DiError::WrongLifetime("Not initialized"));
        }
        *state = LifecycleState::Starting;
        drop(state);

        let init_order = self.init_order.read().await;
        let services = self.services.read().await;

        for key in init_order.iter() {
            // Check if we have a startup service registered for this key
            if let Some(startup) = self.startups.read().await.get(key) {
                startup.startup().await?;
            }
        }

        let mut state = self.state.lock().await;
        *state = LifecycleState::Started;
        Ok(())
    }

    /// Stop all registered services
    pub async fn shutdown_all(&self) -> DiResult<()> {
        let mut state = self.state.lock().await;
        if *state != LifecycleState::Started {
            return Err(DiError::WrongLifetime("Not started"));
        }
        *state = LifecycleState::Stopping;
        drop(state);

        let disposal_order = self.disposal_order.read().await;
        let services = self.services.read().await;

        for key in disposal_order.iter() {
            // Check if we have a shutdown service registered for this key
            if let Some(shutdown) = self.shutdowns.read().await.get(key) {
                shutdown.shutdown().await?;
            }
        }

        let mut state = self.state.lock().await;
        *state = LifecycleState::Stopped;
        Ok(())
    }

    /// Dispose all registered services
    pub async fn dispose_all(&self) -> DiResult<()> {
        let mut state = self.state.lock().await;
        if !matches!(*state, LifecycleState::Stopped | LifecycleState::Initialized) {
            return Err(DiError::WrongLifetime("Invalid state for disposal"));
        }
        *state = LifecycleState::Disposing;
        drop(state);

        let disposal_order = self.disposal_order.read().await;
        let mut services = self.services.write().await;

        for key in disposal_order.iter() {
            // Remove from main services
            services.remove(key);
            
            // Check if we have a disposable service registered for this key
            if let Some(disposable) = self.disposables.write().await.remove(key) {
                disposable.dispose().await?;
            }
        }

        let mut state = self.state.lock().await;
        *state = LifecycleState::Disposed;
        Ok(())
    }

    /// Get the current lifecycle state
    pub async fn state(&self) -> LifecycleState {
        let state = self.state.lock().await;
        state.clone()
    }

    /// Perform graceful shutdown (stop then dispose)
    pub async fn graceful_shutdown(&self) -> DiResult<()> {
        let state = self.state().await;
        
        if state == LifecycleState::Started {
            self.shutdown_all().await?;
        }
        
        // Check the state again after shutdown
        let current_state = self.state().await;
        if matches!(current_state, LifecycleState::Stopped | LifecycleState::Initialized) {
            self.dispose_all().await?;
        }
        
        Ok(())
    }
}

impl Default for AsyncLifecycleManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Async lifecycle hooks for service events
pub struct AsyncLifecycleHooks {
    /// Pre-initialization hooks
    pre_init: Arc<Mutex<Vec<Box<dyn Fn() -> Pin<Box<dyn Future<Output = DiResult<()>> + Send>> + Send + Sync>>>>,
    /// Post-initialization hooks
    post_init: Arc<Mutex<Vec<Box<dyn Fn() -> Pin<Box<dyn Future<Output = DiResult<()>> + Send>> + Send + Sync>>>>,
    /// Pre-disposal hooks
    pre_dispose: Arc<Mutex<Vec<Box<dyn Fn() -> Pin<Box<dyn Future<Output = DiResult<()>> + Send>> + Send + Sync>>>>,
    /// Post-disposal hooks
    post_dispose: Arc<Mutex<Vec<Box<dyn Fn() -> Pin<Box<dyn Future<Output = DiResult<()>> + Send>> + Send + Sync>>>>,
}

use std::pin::Pin;
use std::future::Future;

impl AsyncLifecycleHooks {
    /// Create new lifecycle hooks
    pub fn new() -> Self {
        Self {
            pre_init: Arc::new(Mutex::new(Vec::new())),
            post_init: Arc::new(Mutex::new(Vec::new())),
            pre_dispose: Arc::new(Mutex::new(Vec::new())),
            post_dispose: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Add a pre-initialization hook
    pub async fn add_pre_init<F, Fut>(&self, hook: F)
    where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: Future<Output = DiResult<()>> + Send + 'static,
    {
        let mut hooks = self.pre_init.lock().await;
        hooks.push(Box::new(move || Box::pin(hook())));
    }

    /// Add a post-initialization hook
    pub async fn add_post_init<F, Fut>(&self, hook: F)
    where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: Future<Output = DiResult<()>> + Send + 'static,
    {
        let mut hooks = self.post_init.lock().await;
        hooks.push(Box::new(move || Box::pin(hook())));
    }

    /// Add a pre-disposal hook
    pub async fn add_pre_dispose<F, Fut>(&self, hook: F)
    where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: Future<Output = DiResult<()>> + Send + 'static,
    {
        let mut hooks = self.pre_dispose.lock().await;
        hooks.push(Box::new(move || Box::pin(hook())));
    }

    /// Add a post-disposal hook
    pub async fn add_post_dispose<F, Fut>(&self, hook: F)
    where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: Future<Output = DiResult<()>> + Send + 'static,
    {
        let mut hooks = self.post_dispose.lock().await;
        hooks.push(Box::new(move || Box::pin(hook())));
    }

    /// Run pre-initialization hooks
    pub async fn run_pre_init(&self) -> DiResult<()> {
        let hooks = self.pre_init.lock().await;
        for hook in hooks.iter() {
            hook().await?;
        }
        Ok(())
    }

    /// Run post-initialization hooks
    pub async fn run_post_init(&self) -> DiResult<()> {
        let hooks = self.post_init.lock().await;
        for hook in hooks.iter() {
            hook().await?;
        }
        Ok(())
    }

    /// Run pre-disposal hooks
    pub async fn run_pre_dispose(&self) -> DiResult<()> {
        let hooks = self.pre_dispose.lock().await;
        for hook in hooks.iter() {
            hook().await?;
        }
        Ok(())
    }

    /// Run post-disposal hooks
    pub async fn run_post_dispose(&self) -> DiResult<()> {
        let hooks = self.post_dispose.lock().await;
        for hook in hooks.iter() {
            hook().await?;
        }
        Ok(())
    }
}

impl Default for AsyncLifecycleHooks {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::any::TypeId;

    struct TestService {
        initialized: Arc<Mutex<bool>>,
        started: Arc<Mutex<bool>>,
        stopped: Arc<Mutex<bool>>,
        disposed: Arc<Mutex<bool>>,
    }

    #[async_trait]
    impl AsyncInitializable for TestService {
        async fn initialize(&self) -> DiResult<()> {
            let mut initialized = self.initialized.lock().await;
            *initialized = true;
            Ok(())
        }
    }

    #[async_trait]
    impl AsyncStartup for TestService {
        async fn startup(&self) -> DiResult<()> {
            let mut started = self.started.lock().await;
            *started = true;
            Ok(())
        }
    }

    #[async_trait]
    impl AsyncShutdown for TestService {
        async fn shutdown(&self) -> DiResult<()> {
            let mut stopped = self.stopped.lock().await;
            *stopped = true;
            Ok(())
        }
    }

    #[async_trait]
    impl AsyncDisposable for TestService {
        async fn dispose(self: Arc<Self>) -> DiResult<()> {
            let mut disposed = self.disposed.lock().await;
            *disposed = true;
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_lifecycle_manager() {
        let manager = AsyncLifecycleManager::new();
        
        let service = Arc::new(TestService {
            initialized: Arc::new(Mutex::new(false)),
            started: Arc::new(Mutex::new(false)),
            stopped: Arc::new(Mutex::new(false)),
            disposed: Arc::new(Mutex::new(false)),
        });

        let key = Key::Type(TypeId::of::<TestService>(), "TestService");
        manager.register(key.clone(), service.clone()).await;
        
        // Register trait implementations
        manager.register_initializable(key.clone(), service.clone()).await;
        manager.register_startup(key.clone(), service.clone()).await;
        manager.register_shutdown(key.clone(), service.clone()).await;
        manager.register_disposable(key.clone(), service.clone()).await;

        // Test initialization
        assert_eq!(manager.state().await, LifecycleState::Created);
        manager.initialize_all().await.unwrap();
        assert!(*service.initialized.lock().await);
        assert_eq!(manager.state().await, LifecycleState::Initialized);

        // Test startup
        manager.startup_all().await.unwrap();
        assert!(*service.started.lock().await);
        assert_eq!(manager.state().await, LifecycleState::Started);

        // Test shutdown
        manager.shutdown_all().await.unwrap();
        assert!(*service.stopped.lock().await);
        assert_eq!(manager.state().await, LifecycleState::Stopped);

        // Test disposal
        manager.dispose_all().await.unwrap();
        assert!(*service.disposed.lock().await);
        assert_eq!(manager.state().await, LifecycleState::Disposed);
    }

    #[tokio::test]
    async fn test_lifecycle_hooks() {
        let hooks = AsyncLifecycleHooks::new();
        let counter = Arc::new(Mutex::new(0));
        
        let counter_clone = counter.clone();
        hooks.add_pre_init(move || {
            let counter = counter_clone.clone();
            async move {
                let mut count = counter.lock().await;
                *count += 1;
                Ok(())
            }
        }).await;

        let counter_clone = counter.clone();
        hooks.add_post_init(move || {
            let counter = counter_clone.clone();
            async move {
                let mut count = counter.lock().await;
                *count += 10;
                Ok(())
            }
        }).await;

        hooks.run_pre_init().await.unwrap();
        assert_eq!(*counter.lock().await, 1);

        hooks.run_post_init().await.unwrap();
        assert_eq!(*counter.lock().await, 11);
    }

    #[tokio::test]
    async fn test_graceful_shutdown() {
        let manager = AsyncLifecycleManager::new();
        
        let service = Arc::new(TestService {
            initialized: Arc::new(Mutex::new(false)),
            started: Arc::new(Mutex::new(false)),
            stopped: Arc::new(Mutex::new(false)),
            disposed: Arc::new(Mutex::new(false)),
        });

        let key = Key::Type(TypeId::of::<TestService>(), "TestService");
        manager.register(key.clone(), service.clone()).await;
        
        // Register trait implementations
        manager.register_initializable(key.clone(), service.clone()).await;
        manager.register_startup(key.clone(), service.clone()).await;
        manager.register_shutdown(key.clone(), service.clone()).await;
        manager.register_disposable(key.clone(), service.clone()).await;

        // Initialize and start
        manager.initialize_all().await.unwrap();
        manager.startup_all().await.unwrap();

        // Graceful shutdown should stop then dispose
        manager.graceful_shutdown().await.unwrap();
        
        assert!(*service.stopped.lock().await);
        assert!(*service.disposed.lock().await);
        assert_eq!(manager.state().await, LifecycleState::Disposed);
    }
}