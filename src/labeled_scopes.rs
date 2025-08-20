//! Hierarchical labeled scopes for workflow engines.
//!
//! This module provides labeled scope capabilities essential for n8n-style workflow
//! engines where you need nested contexts (workflow → run → node) with proper
//! hierarchical organization and cleanup.

use std::sync::Arc;
use std::collections::HashMap;
use crate::ServiceProvider;

/// A hierarchical scope with a label for context and tracing.
///
/// Essential for workflow engines where you need nested execution contexts
/// like workflow → run → node, each with their own label for tracing.
///
/// # Examples
///
/// ```
/// use ferrous_di::{ServiceCollection, ServiceProvider, LabeledScope, LabeledScopeExt};
///
/// # fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let mut services = ServiceCollection::new();
///
/// let provider = services.build();
/// 
/// // Create workflow scope
/// let workflow_scope = provider.create_labeled_scope("workflow_123");
/// 
/// // Create run scope within workflow
/// let run_scope = workflow_scope.fork("run_456");
/// 
/// // Create node scope within run
/// let node_scope = run_scope.fork("node_789");
/// 
/// assert_eq!(workflow_scope.label(), "workflow_123");
/// assert_eq!(run_scope.label(), "run_456");
/// assert_eq!(node_scope.label(), "node_789");
/// 
/// // Labels can be used for tracing, logging, cleanup, etc.
/// println!("Executing in context: {}", node_scope.full_path());
/// # Ok(())
/// # }
/// ```
#[derive(Clone)]
pub struct LabeledScope {
    inner: Arc<LabeledScopeInner>,
}

struct LabeledScopeInner {
    label: &'static str,
    parent: Option<LabeledScope>,
    scope: crate::provider::Scope,
    depth: usize,
}

impl LabeledScope {
    /// Creates a new labeled scope from a regular scope.
    pub(crate) fn new(scope: crate::provider::Scope, label: &'static str) -> Self {
        Self {
            inner: Arc::new(LabeledScopeInner {
                label,
                parent: None,
                scope,
                depth: 0,
            })
        }
    }

    /// Creates a child scope with the given label.
    ///
    /// Perfect for hierarchical workflow execution (flow → run → node).
    ///
    /// # Examples
    ///
    /// ```
    /// use ferrous_di::{ServiceCollection, ServiceProvider, LabeledScopeExt};
    ///
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut services = ServiceCollection::new();
    /// let provider = services.build();
    /// 
    /// let workflow_scope = provider.create_labeled_scope("workflow");
    /// let run_scope = workflow_scope.fork("run_001");
    /// let node_scope = run_scope.fork("transform_node");
    /// 
    /// assert_eq!(node_scope.depth(), 2);
    /// assert_eq!(node_scope.full_path(), "workflow/run_001/transform_node");
    /// # Ok(())
    /// # }
    /// ```
    pub fn fork(&self, label: &'static str) -> Self {
        let child_scope = self.inner.scope.create_child();
        
        Self {
            inner: Arc::new(LabeledScopeInner {
                label,
                parent: Some(self.clone()),
                scope: child_scope,
                depth: self.inner.depth + 1,
            })
        }
    }

    /// Returns the label of this scope.
    pub fn label(&self) -> &'static str {
        self.inner.label
    }

    /// Returns the depth of this scope (0 for root, 1 for first child, etc.).
    pub fn depth(&self) -> usize {
        self.inner.depth
    }

    /// Returns the parent scope, if any.
    pub fn parent(&self) -> Option<&LabeledScope> {
        self.inner.parent.as_ref()
    }

    /// Returns the full hierarchical path (e.g., "workflow/run_001/node_123").
    ///
    /// Perfect for logging, tracing, and debugging workflow execution.
    ///
    /// # Examples
    ///
    /// ```
    /// use ferrous_di::{ServiceCollection, ServiceProvider, LabeledScopeExt};
    ///
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut services = ServiceCollection::new();
    /// let provider = services.build();
    /// 
    /// let scope = provider.create_labeled_scope("workflow")
    ///     .fork("run_001")
    ///     .fork("transform_node");
    /// 
    /// assert_eq!(scope.full_path(), "workflow/run_001/transform_node");
    /// # Ok(())
    /// # }
    /// ```
    pub fn full_path(&self) -> String {
        let mut path_parts = Vec::new();
        let mut current = Some(self);
        
        while let Some(scope) = current {
            path_parts.push(scope.label());
            current = scope.parent();
        }
        
        path_parts.reverse();
        path_parts.join("/")
    }

    /// Returns all ancestor labels from root to this scope.
    ///
    /// Useful for hierarchical context tracking in workflow engines.
    ///
    /// # Examples
    ///
    /// ```
    /// use ferrous_di::{ServiceCollection, ServiceProvider, LabeledScopeExt};
    ///
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut services = ServiceCollection::new();
    /// let provider = services.build();
    /// 
    /// let scope = provider.create_labeled_scope("workflow")
    ///     .fork("run_001")
    ///     .fork("transform");
    /// 
    /// let ancestors = scope.ancestors();
    /// assert_eq!(ancestors, vec!["workflow", "run_001", "transform"]);
    /// # Ok(())
    /// # }
    /// ```
    pub fn ancestors(&self) -> Vec<&'static str> {
        let mut ancestors = Vec::new();
        let mut current = Some(self);
        
        while let Some(scope) = current {
            ancestors.push(scope.label());
            current = scope.parent();
        }
        
        ancestors.reverse();
        ancestors
    }

    /// Finds an ancestor scope with the given label.
    ///
    /// Perfect for workflow engines where you need to access parent contexts.
    ///
    /// # Examples
    ///
    /// ```
    /// use ferrous_di::{ServiceCollection, ServiceProvider, LabeledScopeExt};
    ///
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut services = ServiceCollection::new();
    /// let provider = services.build();
    /// 
    /// let workflow_scope = provider.create_labeled_scope("workflow_123");
    /// let run_scope = workflow_scope.fork("run_456");
    /// let node_scope = run_scope.fork("node_789");
    /// 
    /// let workflow = node_scope.find_ancestor("workflow_123").unwrap();
    /// assert_eq!(workflow.label(), "workflow_123");
    /// # Ok(())
    /// # }
    /// ```
    pub fn find_ancestor(&self, label: &str) -> Option<&LabeledScope> {
        let mut current = Some(self);
        
        while let Some(scope) = current {
            if scope.label() == label {
                return Some(scope);
            }
            current = scope.parent();
        }
        
        None
    }

    /// Returns true if this scope is an ancestor of (or equal to) the other scope.
    ///
    /// Useful for validating scope hierarchies in workflow engines.
    ///
    /// # Examples
    ///
    /// ```
    /// use ferrous_di::{ServiceCollection, ServiceProvider, LabeledScopeExt};
    ///
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut services = ServiceCollection::new();
    /// let provider = services.build();
    /// 
    /// let workflow_scope = provider.create_labeled_scope("workflow");
    /// let run_scope = workflow_scope.fork("run");
    /// let node_scope = run_scope.fork("node");
    /// 
    /// assert!(workflow_scope.is_ancestor_of(&node_scope));
    /// assert!(run_scope.is_ancestor_of(&node_scope));
    /// assert!(!node_scope.is_ancestor_of(&workflow_scope));
    /// # Ok(())
    /// # }
    /// ```
    pub fn is_ancestor_of(&self, other: &LabeledScope) -> bool {
        let mut current = Some(other);
        
        while let Some(scope) = current {
            if std::ptr::eq(self.inner.as_ref(), scope.inner.as_ref()) {
                return true;
            }
            current = scope.parent();
        }
        
        false
    }

    /// Returns metadata about this scope for tracing and debugging.
    ///
    /// Essential for workflow engines that need rich context information.
    ///
    /// # Examples
    ///
    /// ```
    /// use ferrous_di::{ServiceCollection, ServiceProvider, LabeledScopeExt};
    ///
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut services = ServiceCollection::new();
    /// let provider = services.build();
    /// 
    /// let scope = provider.create_labeled_scope("workflow")
    ///     .fork("run_001");
    /// 
    /// let metadata = scope.metadata();
    /// assert_eq!(metadata.label, "run_001");
    /// assert_eq!(metadata.depth, 1);
    /// assert_eq!(metadata.full_path, "workflow/run_001");
    /// assert_eq!(metadata.ancestors, vec!["workflow", "run_001"]);
    /// # Ok(())
    /// # }
    /// ```
    pub fn metadata(&self) -> ScopeMetadata {
        ScopeMetadata {
            label: self.label(),
            depth: self.depth(),
            full_path: self.full_path(),
            ancestors: self.ancestors(),
            parent_label: self.parent().map(|p| p.label()),
        }
    }

    /// Access the underlying scope for service resolution.
    ///
    /// This provides access to all the DI functionality while maintaining
    /// the labeled scope context.
    pub fn as_scope(&self) -> &crate::provider::Scope {
        &self.inner.scope
    }
}

/// Metadata about a labeled scope for tracing and debugging.
#[derive(Debug, Clone, PartialEq)]
pub struct ScopeMetadata {
    /// The label of this scope
    pub label: &'static str,
    /// The depth in the hierarchy (0 for root)
    pub depth: usize,
    /// The full hierarchical path
    pub full_path: String,
    /// All ancestor labels from root to this scope
    pub ancestors: Vec<&'static str>,
    /// The parent scope's label, if any
    pub parent_label: Option<&'static str>,
}

/// Extension trait for ServiceProvider to create labeled scopes.
pub trait LabeledScopeExt {
    /// Creates a new labeled scope.
    ///
    /// This is the entry point for hierarchical workflow contexts.
    ///
    /// # Examples
    ///
    /// ```
    /// use ferrous_di::{ServiceCollection, LabeledScopeExt};
    ///
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut services = ServiceCollection::new();
    /// let provider = services.build();
    /// 
    /// let workflow_scope = provider.create_labeled_scope("workflow_123");
    /// assert_eq!(workflow_scope.label(), "workflow_123");
    /// assert_eq!(workflow_scope.depth(), 0);
    /// # Ok(())
    /// # }
    /// ```
    fn create_labeled_scope(&self, label: &'static str) -> LabeledScope;
}

impl LabeledScopeExt for ServiceProvider {
    fn create_labeled_scope(&self, label: &'static str) -> LabeledScope {
        let scope = self.create_scope();
        LabeledScope::new(scope, label)
    }
}

/// Helper for creating scoped context in workflow engines.
///
/// This provides a convenient way to pass labeled scope context
/// through workflow execution without manual parameter threading.
///
/// # Examples
///
/// ```
/// use ferrous_di::{ServiceCollection, LabeledScopeContext, LabeledScopeExt, Resolver};
///
/// # fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let mut services = ServiceCollection::new();
/// 
/// // Register the scope context with a factory
/// services.add_scoped_factory::<LabeledScopeContext, _>(|_| {
///     // This would be populated by the labeled scope in practice
///     panic!("Should be replaced by labeled scope")
/// });
/// 
/// let provider = services.build();
/// let scope = provider.create_labeled_scope("workflow");
/// 
/// // In practice, labeled scopes would register themselves in the context
/// // assert_eq!(context.current_label(), "workflow");
/// # Ok(())
/// # }
/// ```
#[derive(Clone)]
pub struct LabeledScopeContext {
    scope: LabeledScope,
}

impl LabeledScopeContext {
    /// Creates a new scope context.
    pub fn new(scope: LabeledScope) -> Self {
        Self { scope }
    }

    /// Returns the current scope's label.
    pub fn current_label(&self) -> &'static str {
        self.scope.label()
    }

    /// Returns the current scope's depth.
    pub fn current_depth(&self) -> usize {
        self.scope.depth()
    }

    /// Returns the full hierarchical path.
    pub fn full_path(&self) -> String {
        self.scope.full_path()
    }

    /// Returns the labeled scope for advanced operations.
    pub fn labeled_scope(&self) -> &LabeledScope {
        &self.scope
    }

    /// Returns metadata about the current scope.
    pub fn metadata(&self) -> ScopeMetadata {
        self.scope.metadata()
    }
}

/// Registry for tracking active labeled scopes in workflow engines.
///
/// Essential for debugging, monitoring, and cleanup in complex workflows.
///
/// # Examples
///
/// ```
/// use ferrous_di::{ServiceCollection, LabeledScopeRegistry, LabeledScopeExt, Resolver};
/// use std::sync::Arc;
///
/// # fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let mut services = ServiceCollection::new();
/// services.add_singleton_factory::<LabeledScopeRegistry, _>(|_| LabeledScopeRegistry::new());
///
/// let provider = services.build();
/// let registry = provider.get_required::<LabeledScopeRegistry>();
/// 
/// let scope = provider.create_labeled_scope("workflow_123");
/// registry.register(&scope);
/// 
/// let active_scopes = registry.active_scopes();
/// assert_eq!(active_scopes.len(), 1);
/// assert_eq!(active_scopes[0].label(), "workflow_123");
/// # Ok(())
/// # }
/// ```
#[derive(Default)]
pub struct LabeledScopeRegistry {
    scopes: std::sync::Mutex<HashMap<String, LabeledScope>>,
}

impl LabeledScopeRegistry {
    /// Creates a new scope registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a labeled scope for tracking.
    ///
    /// Useful for monitoring active workflows and cleanup on shutdown.
    pub fn register(&self, scope: &LabeledScope) {
        let mut scopes = self.scopes.lock().unwrap();
        scopes.insert(scope.full_path(), scope.clone());
    }

    /// Unregisters a labeled scope.
    ///
    /// Should be called when workflow execution completes.
    pub fn unregister(&self, scope: &LabeledScope) {
        let mut scopes = self.scopes.lock().unwrap();
        scopes.remove(&scope.full_path());
    }

    /// Returns all currently active scopes.
    ///
    /// Perfect for debugging and monitoring workflow state.
    pub fn active_scopes(&self) -> Vec<LabeledScope> {
        let scopes = self.scopes.lock().unwrap();
        scopes.values().cloned().collect()
    }

    /// Finds active scopes with the given label.
    ///
    /// Useful for finding all active workflows or runs with a specific identifier.
    pub fn find_by_label(&self, label: &str) -> Vec<LabeledScope> {
        let scopes = self.scopes.lock().unwrap();
        scopes.values()
            .filter(|scope| scope.label() == label)
            .cloned()
            .collect()
    }

    /// Finds active scopes at the given depth.
    ///
    /// Useful for finding all workflows (depth 0), runs (depth 1), or nodes (depth 2).
    pub fn find_by_depth(&self, depth: usize) -> Vec<LabeledScope> {
        let scopes = self.scopes.lock().unwrap();
        scopes.values()
            .filter(|scope| scope.depth() == depth)
            .cloned()
            .collect()
    }

    /// Returns the count of active scopes.
    pub fn active_count(&self) -> usize {
        let scopes = self.scopes.lock().unwrap();
        scopes.len()
    }

    /// Clears all registered scopes.
    ///
    /// Useful for cleanup during shutdown.
    pub fn clear(&self) {
        let mut scopes = self.scopes.lock().unwrap();
        scopes.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ServiceCollection;

    #[test]
    fn test_labeled_scope_creation() {
        let services = ServiceCollection::new();
        let provider = services.build();
        
        let scope = provider.create_labeled_scope("test_workflow");
        assert_eq!(scope.label(), "test_workflow");
        assert_eq!(scope.depth(), 0);
        assert!(scope.parent().is_none());
    }

    #[test]
    fn test_scope_forking() {
        let services = ServiceCollection::new();
        let provider = services.build();
        
        let workflow = provider.create_labeled_scope("workflow");
        let run = workflow.fork("run_001");
        let node = run.fork("transform");
        
        assert_eq!(workflow.label(), "workflow");
        assert_eq!(run.label(), "run_001");
        assert_eq!(node.label(), "transform");
        
        assert_eq!(workflow.depth(), 0);
        assert_eq!(run.depth(), 1);
        assert_eq!(node.depth(), 2);
    }

    #[test]
    fn test_full_path() {
        let services = ServiceCollection::new();
        let provider = services.build();
        
        let scope = provider.create_labeled_scope("workflow")
            .fork("run_001")
            .fork("transform_node");
        
        assert_eq!(scope.full_path(), "workflow/run_001/transform_node");
    }

    #[test]
    fn test_ancestors() {
        let services = ServiceCollection::new();
        let provider = services.build();
        
        let scope = provider.create_labeled_scope("workflow")
            .fork("run_001")
            .fork("transform");
        
        let ancestors = scope.ancestors();
        assert_eq!(ancestors, vec!["workflow", "run_001", "transform"]);
    }

    #[test]
    fn test_find_ancestor() {
        let services = ServiceCollection::new();
        let provider = services.build();
        
        let workflow = provider.create_labeled_scope("workflow");
        let run = workflow.fork("run_001");
        let node = run.fork("transform");
        
        let found_workflow = node.find_ancestor("workflow").unwrap();
        assert_eq!(found_workflow.label(), "workflow");
        
        let found_run = node.find_ancestor("run_001").unwrap();
        assert_eq!(found_run.label(), "run_001");
        
        assert!(node.find_ancestor("nonexistent").is_none());
    }

    #[test]
    fn test_is_ancestor_of() {
        let services = ServiceCollection::new();
        let provider = services.build();
        
        let workflow = provider.create_labeled_scope("workflow");
        let run = workflow.fork("run");
        let node = run.fork("node");
        
        assert!(workflow.is_ancestor_of(&node));
        assert!(run.is_ancestor_of(&node));
        assert!(node.is_ancestor_of(&node)); // Self is ancestor
        assert!(!node.is_ancestor_of(&workflow));
    }

    #[test]
    fn test_scope_metadata() {
        let services = ServiceCollection::new();
        let provider = services.build();
        
        let scope = provider.create_labeled_scope("workflow")
            .fork("run_001");
        
        let metadata = scope.metadata();
        assert_eq!(metadata.label, "run_001");
        assert_eq!(metadata.depth, 1);
        assert_eq!(metadata.full_path, "workflow/run_001");
        assert_eq!(metadata.ancestors, vec!["workflow", "run_001"]);
        assert_eq!(metadata.parent_label, Some("workflow"));
    }

    #[test]
    fn test_scope_registry() {
        let registry = LabeledScopeRegistry::new();
        let services = ServiceCollection::new();
        let provider = services.build();
        
        let scope1 = provider.create_labeled_scope("workflow_1");
        let scope2 = provider.create_labeled_scope("workflow_2");
        
        registry.register(&scope1);
        registry.register(&scope2);
        
        assert_eq!(registry.active_count(), 2);
        
        let active = registry.active_scopes();
        assert_eq!(active.len(), 2);
        
        registry.unregister(&scope1);
        assert_eq!(registry.active_count(), 1);
        
        registry.clear();
        assert_eq!(registry.active_count(), 0);
    }

    #[test]
    fn test_registry_find_operations() {
        let registry = LabeledScopeRegistry::new();
        let services = ServiceCollection::new();
        let provider = services.build();
        
        let workflow1 = provider.create_labeled_scope("workflow");
        let workflow2 = provider.create_labeled_scope("another_workflow");
        let run1 = workflow1.fork("run");
        
        registry.register(&workflow1);
        registry.register(&workflow2);
        registry.register(&run1);
        
        // Find by label
        let workflows = registry.find_by_label("workflow");
        assert_eq!(workflows.len(), 1);
        
        let runs = registry.find_by_label("run");
        assert_eq!(runs.len(), 1);
        
        // Find by depth
        let depth_0 = registry.find_by_depth(0);
        assert_eq!(depth_0.len(), 2); // Two workflows
        
        let depth_1 = registry.find_by_depth(1);
        assert_eq!(depth_1.len(), 1); // One run
    }
}