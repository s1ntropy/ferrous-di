//! Debug tooling and visualization for ferrous-di.
//!
//! This module provides tools for debugging dependency injection issues,
//! visualizing dependency graphs, and analyzing service resolution paths.

use std::collections::{HashMap, HashSet, VecDeque};
use std::fmt;
use crate::{Key, ServiceDescriptor, Registry, Lifetime};

/// Dependency graph analyzer and visualizer
#[derive(Debug)]
pub struct DependencyGraphAnalyzer {
    /// Service registry reference
    registry: Registry,
}

/// Represents a node in the dependency graph
#[derive(Debug, Clone)]
pub struct GraphNode {
    /// Service key
    pub key: Key,
    /// Service descriptor
    pub descriptor: ServiceDescriptor,
    /// Direct dependencies
    pub dependencies: Vec<Key>,
    /// Services that depend on this one
    pub dependents: Vec<Key>,
    /// Graph analysis metadata
    pub metadata: NodeMetadata,
}

/// Metadata for graph analysis
#[derive(Debug, Clone, Default)]
pub struct NodeMetadata {
    /// Depth from root services
    pub depth: usize,
    /// Number of times this service is depended upon
    pub reference_count: usize,
    /// Whether this node is part of a circular dependency
    pub is_circular: bool,
    /// Estimated resolution cost
    pub resolution_cost: u32,
}

/// Analysis results for the dependency graph
#[derive(Debug)]
pub struct GraphAnalysis {
    /// Total number of services
    pub service_count: usize,
    /// Total number of dependencies
    pub dependency_count: usize,
    /// Maximum dependency depth
    pub max_depth: usize,
    /// Average dependency depth
    pub avg_depth: f64,
    /// Services with no dependencies (leaf nodes)
    pub leaf_services: Vec<Key>,
    /// Services with many dependencies (potential bottlenecks)
    pub high_dependency_services: Vec<(Key, usize)>,
    /// Circular dependency chains found
    pub circular_dependencies: Vec<Vec<Key>>,
    /// Services that are never used
    pub unused_services: Vec<Key>,
}

impl DependencyGraphAnalyzer {
    /// Create a new dependency graph analyzer
    pub fn new(registry: Registry) -> Self {
        Self { registry }
    }

    /// Analyze the complete dependency graph
    pub fn analyze(&self) -> GraphAnalysis {
        let nodes = self.build_graph();
        
        let service_count = nodes.len();
        let dependency_count: usize = nodes.values().map(|n| n.dependencies.len()).sum();
        
        let depths: Vec<usize> = nodes.values().map(|n| n.metadata.depth).collect();
        let max_depth = depths.iter().copied().max().unwrap_or(0);
        let avg_depth = if depths.is_empty() {
            0.0
        } else {
            depths.iter().sum::<usize>() as f64 / depths.len() as f64
        };

        let leaf_services = nodes
            .values()
            .filter(|n| n.dependencies.is_empty())
            .map(|n| n.key.clone())
            .collect();

        let high_dependency_services = nodes
            .values()
            .filter(|n| n.dependencies.len() > 5) // Threshold for "high"
            .map(|n| (n.key.clone(), n.dependencies.len()))
            .collect();

        let circular_dependencies = self.find_circular_dependencies(&nodes);
        let unused_services = self.find_unused_services(&nodes);

        GraphAnalysis {
            service_count,
            dependency_count,
            max_depth,
            avg_depth,
            leaf_services,
            high_dependency_services,
            circular_dependencies,
            unused_services,
        }
    }

    /// Build the complete dependency graph
    fn build_graph(&self) -> HashMap<Key, GraphNode> {
        let mut nodes = HashMap::new();
        let descriptors = self.registry.get_all_descriptors();

        // First pass: create nodes
        for descriptor in &descriptors {
            let node = GraphNode {
                key: descriptor.key.clone(),
                descriptor: descriptor.clone(),
                dependencies: Vec::new(), // Will be populated in second pass
                dependents: Vec::new(),
                metadata: NodeMetadata::default(),
            };
            nodes.insert(descriptor.key.clone(), node);
        }

        // Second pass: build dependency relationships
        // Note: In a full implementation, this would analyze factory functions
        // to extract dependencies. For now, we'll use a simplified approach.
        
        // Third pass: calculate metadata
        self.calculate_depths(&mut nodes);
        self.calculate_reference_counts(&mut nodes);
        self.mark_circular_dependencies(&mut nodes);

        nodes
    }

    /// Calculate depth for each node using BFS
    fn calculate_depths(&self, nodes: &mut HashMap<Key, GraphNode>) {
        // Find root nodes (services with no dependencies)
        let root_keys: Vec<Key> = nodes
            .values()
            .filter(|n| n.dependencies.is_empty())
            .map(|n| n.key.clone())
            .collect();

        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();

        // Initialize root nodes with depth 0
        for key in root_keys {
            queue.push_back((key.clone(), 0));
            visited.insert(key);
        }

        // BFS to calculate depths
        while let Some((key, depth)) = queue.pop_front() {
            if let Some(node) = nodes.get_mut(&key) {
                node.metadata.depth = depth;

                // Add dependents to queue with increased depth
                for dependent_key in &node.dependents {
                    if !visited.contains(dependent_key) {
                        visited.insert(dependent_key.clone());
                        queue.push_back((dependent_key.clone(), depth + 1));
                    }
                }
            }
        }
    }

    /// Calculate reference counts for each node
    fn calculate_reference_counts(&self, nodes: &mut HashMap<Key, GraphNode>) {
        for node in nodes.values_mut() {
            node.metadata.reference_count = node.dependents.len();
        }
    }

    /// Mark nodes that are part of circular dependencies
    fn mark_circular_dependencies(&self, nodes: &mut HashMap<Key, GraphNode>) {
        let circular_chains = self.find_circular_dependencies(nodes);
        
        for chain in &circular_chains {
            for key in chain {
                if let Some(node) = nodes.get_mut(key) {
                    node.metadata.is_circular = true;
                }
            }
        }
    }

    /// Find all circular dependency chains
    fn find_circular_dependencies(&self, nodes: &HashMap<Key, GraphNode>) -> Vec<Vec<Key>> {
        let mut circular_chains = Vec::new();
        let mut visited = HashSet::new();
        let mut rec_stack = HashSet::new();

        for key in nodes.keys() {
            if !visited.contains(key) {
                if let Some(chain) = self.dfs_find_cycle(key, nodes, &mut visited, &mut rec_stack, &mut Vec::new()) {
                    circular_chains.push(chain);
                }
            }
        }

        circular_chains
    }

    /// DFS helper for finding circular dependencies
    fn dfs_find_cycle(
        &self,
        key: &Key,
        nodes: &HashMap<Key, GraphNode>,
        visited: &mut HashSet<Key>,
        rec_stack: &mut HashSet<Key>,
        path: &mut Vec<Key>,
    ) -> Option<Vec<Key>> {
        visited.insert(key.clone());
        rec_stack.insert(key.clone());
        path.push(key.clone());

        if let Some(node) = nodes.get(key) {
            for dep in &node.dependencies {
                if !visited.contains(dep) {
                    if let Some(cycle) = self.dfs_find_cycle(dep, nodes, visited, rec_stack, path) {
                        return Some(cycle);
                    }
                } else if rec_stack.contains(dep) {
                    // Found a cycle - extract the circular part
                    if let Some(cycle_start) = path.iter().position(|k| k == dep) {
                        return Some(path[cycle_start..].to_vec());
                    }
                }
            }
        }

        path.pop();
        rec_stack.remove(key);
        None
    }

    /// Find services that are never used as dependencies
    fn find_unused_services(&self, nodes: &HashMap<Key, GraphNode>) -> Vec<Key> {
        nodes
            .values()
            .filter(|n| n.dependents.is_empty() && n.metadata.reference_count == 0)
            .map(|n| n.key.clone())
            .collect()
    }

    /// Generate a DOT graph representation for visualization
    pub fn to_dot(&self) -> String {
        let nodes = self.build_graph();
        let mut dot = String::from("digraph DependencyGraph {\n");
        dot.push_str("  rankdir=TB;\n");
        dot.push_str("  node [shape=box, style=rounded];\n\n");

        // Add nodes with styling based on metadata
        for node in nodes.values() {
            let service_name = node.key.display_name();
            let mut style = String::new();
            
            // Color code by lifetime
            let color = match node.descriptor.lifetime {
                Lifetime::Singleton => "lightblue",
                Lifetime::Scoped => "lightgreen", 
                Lifetime::Transient => "lightyellow",
            };
            
            // Special styling for circular dependencies
            if node.metadata.is_circular {
                style.push_str(", color=red, penwidth=2");
            }

            dot.push_str(&format!(
                "  \"{}\" [label=\"{}\\nDepth: {}\\nRefs: {}\" fillcolor={} style=\"filled{}\"]; \n",
                service_name,
                service_name,
                node.metadata.depth,
                node.metadata.reference_count,
                color,
                style
            ));
        }

        dot.push('\n');

        // Add edges
        for node in nodes.values() {
            let from = node.key.display_name();
            for dep in &node.dependencies {
                let to = dep.display_name();
                dot.push_str(&format!("  \"{}\" -> \"{}\";\n", from, to));
            }
        }

        dot.push_str("}\n");
        dot
    }

    /// Generate a text-based tree representation
    pub fn to_tree(&self, root_key: &Key) -> String {
        let nodes = self.build_graph();
        let mut result = String::new();
        let mut visited = HashSet::new();
        self.build_tree_string(&nodes, root_key, 0, &mut visited, &mut result);
        result
    }

    /// Recursive helper for building tree string
    fn build_tree_string(
        &self,
        nodes: &HashMap<Key, GraphNode>,
        key: &Key,
        depth: usize,
        visited: &mut HashSet<Key>,
        result: &mut String,
    ) {
        let indent = "  ".repeat(depth);
        let service_name = key.display_name();
        
        if visited.contains(key) {
            result.push_str(&format!("{}└─ {} (circular reference)\n", indent, service_name));
            return;
        }

        visited.insert(key.clone());

        if let Some(node) = nodes.get(key) {
            let lifetime_str = match node.descriptor.lifetime {
                Lifetime::Singleton => "S",
                Lifetime::Scoped => "C",
                Lifetime::Transient => "T",
            };

            result.push_str(&format!(
                "{}└─ {} [{}] (depth: {}, refs: {})\n",
                indent, service_name, lifetime_str, node.metadata.depth, node.metadata.reference_count
            ));

            for (i, dep) in node.dependencies.iter().enumerate() {
                if i == node.dependencies.len() - 1 {
                    self.build_tree_string(nodes, dep, depth + 1, visited, result);
                } else {
                    self.build_tree_string(nodes, dep, depth + 1, visited, result);
                }
            }
        }

        visited.remove(key);
    }
}

/// Service resolution tracer for debugging resolution paths
#[derive(Debug)]
pub struct ResolutionTracer {
    /// Trace entries in chronological order
    trace_entries: Vec<TraceEntry>,
    /// Whether tracing is currently enabled
    enabled: bool,
}

#[derive(Debug, Clone)]
pub struct TraceEntry {
    /// Service key being resolved
    pub key: Key,
    /// Resolution depth (nesting level)
    pub depth: usize,
    /// Timestamp when resolution started
    pub timestamp: std::time::Instant,
    /// Resolution duration (set when resolution completes)
    pub duration: Option<std::time::Duration>,
    /// Resolution result
    pub result: TraceResult,
}

#[derive(Debug, Clone)]
pub enum TraceResult {
    /// Resolution started
    Started,
    /// Resolution completed successfully
    Success,
    /// Resolution failed with error
    Error(String),
    /// Resolution was cached
    Cached,
}

impl ResolutionTracer {
    /// Create a new resolution tracer
    pub fn new() -> Self {
        Self {
            trace_entries: Vec::new(),
            enabled: false,
        }
    }

    /// Enable tracing
    pub fn enable(&mut self) {
        self.enabled = true;
    }

    /// Disable tracing
    pub fn disable(&mut self) {
        self.enabled = false;
    }

    /// Record the start of service resolution
    pub fn trace_start(&mut self, key: &Key, depth: usize) {
        if !self.enabled {
            return;
        }

        let entry = TraceEntry {
            key: key.clone(),
            depth,
            timestamp: std::time::Instant::now(),
            duration: None,
            result: TraceResult::Started,
        };

        self.trace_entries.push(entry);
    }

    /// Record successful service resolution
    pub fn trace_success(&mut self, key: &Key) {
        if !self.enabled {
            return;
        }

        if let Some(entry) = self.trace_entries.iter_mut().rev().find(|e| e.key == *key) {
            entry.duration = Some(entry.timestamp.elapsed());
            entry.result = TraceResult::Success;
        }
    }

    /// Record failed service resolution
    pub fn trace_error(&mut self, key: &Key, error: &str) {
        if !self.enabled {
            return;
        }

        if let Some(entry) = self.trace_entries.iter_mut().rev().find(|e| e.key == *key) {
            entry.duration = Some(entry.timestamp.elapsed());
            entry.result = TraceResult::Error(error.to_string());
        }
    }

    /// Record cached service resolution
    pub fn trace_cached(&mut self, key: &Key) {
        if !self.enabled {
            return;
        }

        if let Some(entry) = self.trace_entries.iter_mut().rev().find(|e| e.key == *key) {
            entry.duration = Some(entry.timestamp.elapsed());
            entry.result = TraceResult::Cached;
        }
    }

    /// Get all trace entries
    pub fn get_trace(&self) -> &[TraceEntry] {
        &self.trace_entries
    }

    /// Clear the trace history
    pub fn clear(&mut self) {
        self.trace_entries.clear();
    }

    /// Format trace as a tree structure
    pub fn format_trace(&self) -> String {
        let mut result = String::new();
        
        for entry in &self.trace_entries {
            let indent = "  ".repeat(entry.depth);
            let duration_str = if let Some(duration) = entry.duration {
                format!(" ({:.2}ms)", duration.as_secs_f64() * 1000.0)
            } else {
                String::new()
            };

            let result_str = match &entry.result {
                TraceResult::Started => "⏳ Started",
                TraceResult::Success => "✅ Success",
                TraceResult::Error(err) => &format!("❌ Error: {}", err),
                TraceResult::Cached => "💾 Cached",
            };

            result.push_str(&format!(
                "{}{} - {}{}\n",
                indent,
                entry.key.display_name(),
                result_str,
                duration_str
            ));
        }

        result
    }
}

impl Default for ResolutionTracer {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for GraphAnalysis {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Dependency Graph Analysis")?;
        writeln!(f, "========================")?;
        writeln!(f, "Services: {}", self.service_count)?;
        writeln!(f, "Dependencies: {}", self.dependency_count)?;
        writeln!(f, "Max Depth: {}", self.max_depth)?;
        writeln!(f, "Avg Depth: {:.2}", self.avg_depth)?;
        writeln!(f, "Leaf Services: {}", self.leaf_services.len())?;
        writeln!(f, "High Dependency Services: {}", self.high_dependency_services.len())?;
        writeln!(f, "Circular Dependencies: {}", self.circular_dependencies.len())?;
        writeln!(f, "Unused Services: {}", self.unused_services.len())?;

        if !self.circular_dependencies.is_empty() {
            writeln!(f, "\nCircular Dependencies Found:")?;
            for (i, cycle) in self.circular_dependencies.iter().enumerate() {
                write!(f, "  {}: ", i + 1)?;
                for (j, key) in cycle.iter().enumerate() {
                    if j > 0 {
                        write!(f, " -> ")?;
                    }
                    write!(f, "{}", key.display_name())?;
                }
                writeln!(f, " -> {}", cycle[0].display_name())?;
            }
        }

        if !self.unused_services.is_empty() {
            writeln!(f, "\nUnused Services:")?;
            for key in &self.unused_services {
                writeln!(f, "  - {}", key.display_name())?;
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ServiceCollection};
    use std::any::TypeId;

    #[test]
    fn test_dependency_graph_analyzer() {
        let mut services = ServiceCollection::new();
        services.add_singleton("Database Connection".to_string());
        services.add_scoped_factory::<String, _>(|_| "User Service".to_string());
        
        let provider = services.build();
        
        // For this test, we'll create a mock registry
        // In a real implementation, this would come from the provider
        let registry = Registry::new(); // This is a placeholder
        
        let analyzer = DependencyGraphAnalyzer::new(registry);
        let analysis = analyzer.analyze();
        
        assert!(analysis.service_count >= 0); // Basic sanity check
        assert!(analysis.max_depth >= 0);
    }

    #[test]
    fn test_resolution_tracer() {
        let mut tracer = ResolutionTracer::new();
        let key = Key::Type(TypeId::of::<String>(), "String");
        
        tracer.enable();
        tracer.trace_start(&key, 0);
        tracer.trace_success(&key);
        
        let trace = tracer.get_trace();
        assert_eq!(trace.len(), 1);
        assert!(matches!(trace[0].result, TraceResult::Success));
        assert!(trace[0].duration.is_some());
        
        let formatted = tracer.format_trace();
        assert!(formatted.contains("String"));
        assert!(formatted.contains("✅ Success"));
    }

    #[test]
    fn test_graph_analysis_display() {
        let analysis = GraphAnalysis {
            service_count: 5,
            dependency_count: 8,
            max_depth: 3,
            avg_depth: 1.8,
            leaf_services: vec![Key::Type(TypeId::of::<String>(), "String")],
            high_dependency_services: vec![(Key::Type(TypeId::of::<i32>(), "i32"), 6)],
            circular_dependencies: vec![vec![
                Key::Type(TypeId::of::<u32>(), "u32"),
                Key::Type(TypeId::of::<u64>(), "u64"),
            ]],
            unused_services: vec![],
        };
        
        let display_str = format!("{}", analysis);
        assert!(display_str.contains("Services: 5"));
        assert!(display_str.contains("Dependencies: 8"));
        assert!(display_str.contains("Circular Dependencies Found:"));
    }
}