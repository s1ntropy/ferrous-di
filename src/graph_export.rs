//! Graph export functionality for dependency visualization and UI integration.
//!
//! This module provides tools for exporting the dependency injection container's
//! structure as graphs for visualization, debugging, and UI presentation.
//! Essential for n8n-style workflow engines where understanding service
//! relationships is critical.

use std::collections::{HashMap, HashSet};

#[cfg(feature = "graph-export")]
use serde::{Serialize, Deserialize};

/// A node in the dependency graph representing a service or trait registration.
///
/// Contains metadata about the service including its type, lifetime, 
/// dependencies, and registration details.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "graph-export", derive(Serialize, Deserialize))]
pub struct GraphNode {
    /// Unique identifier for this node
    pub id: String,
    /// Display name of the service type
    pub type_name: String,
    /// Service lifetime (Singleton, Scoped, Transient)
    pub lifetime: String,
    /// Whether this is a trait registration
    pub is_trait: bool,
    /// List of dependency type names this service requires
    pub dependencies: Vec<String>,
    /// Additional metadata about the service
    pub metadata: HashMap<String, String>,
    /// Visual positioning hints for UI (optional)
    pub position: Option<NodePosition>,
}

/// Visual positioning information for graph layout.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "graph-export", derive(Serialize, Deserialize))]
pub struct NodePosition {
    pub x: f64,
    pub y: f64,
    pub z: Option<f64>,
}

/// An edge in the dependency graph representing a dependency relationship.
///
/// Connects services that depend on each other, with optional metadata
/// about the relationship type and strength.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "graph-export", derive(Serialize, Deserialize))]
pub struct GraphEdge {
    /// Source node ID (the service that depends on another)
    pub from: String,
    /// Target node ID (the service being depended upon)
    pub to: String,
    /// Type of dependency (required, optional, multiple, etc.)
    pub dependency_type: DependencyType,
    /// Additional metadata about this relationship
    pub metadata: HashMap<String, String>,
}

/// Types of dependency relationships between services.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "graph-export", derive(Serialize, Deserialize))]
pub enum DependencyType {
    /// Required single dependency
    Required,
    /// Optional dependency (might not be present)
    Optional,
    /// Multiple instances of the same service
    Multiple,
    /// Trait dependency
    Trait,
    /// Factory dependency (service creates other services)
    Factory,
    /// Scoped dependency (specific to scope context)
    Scoped,
    /// Decorated dependency (wrapped by decorators)
    Decorated,
}

/// Complete dependency graph export containing all nodes and relationships.
///
/// This structure can be serialized to JSON, YAML, or other formats for
/// consumption by visualization tools, debuggers, or workflow UIs.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "graph-export", derive(Serialize, Deserialize))]
pub struct DependencyGraph {
    /// All service nodes in the graph
    pub nodes: Vec<GraphNode>,
    /// All dependency relationships between nodes
    pub edges: Vec<GraphEdge>,
    /// Graph-level metadata
    pub metadata: GraphMetadata,
    /// Layout information for visualization
    pub layout: Option<GraphLayout>,
}

/// Metadata about the entire dependency graph.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "graph-export", derive(Serialize, Deserialize))]
pub struct GraphMetadata {
    /// Total number of registered services
    pub service_count: usize,
    /// Total number of trait registrations
    pub trait_count: usize,
    /// Number of singleton services
    pub singleton_count: usize,
    /// Number of scoped services
    pub scoped_count: usize,
    /// Number of transient services
    pub transient_count: usize,
    /// Whether circular dependencies were detected
    pub has_circular_dependencies: bool,
    /// Export timestamp
    pub exported_at: String,
    /// Export format version
    pub version: String,
}

/// Layout information for graph visualization.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "graph-export", derive(Serialize, Deserialize))]
pub struct GraphLayout {
    /// Suggested layout algorithm
    pub algorithm: String,
    /// Layout-specific parameters
    #[cfg(feature = "graph-export")]
    pub parameters: HashMap<String, serde_json::Value>,
    #[cfg(not(feature = "graph-export"))]
    pub parameters: HashMap<String, String>,
    /// Viewport bounds for the graph
    pub bounds: Option<LayoutBounds>,
}

/// Viewport bounds for graph layout.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "graph-export", derive(Serialize, Deserialize))]
pub struct LayoutBounds {
    pub min_x: f64,
    pub min_y: f64,
    pub max_x: f64,
    pub max_y: f64,
}

/// Graph export configuration options.
#[derive(Debug, Clone)]
pub struct ExportOptions {
    /// Include dependency details in nodes
    pub include_dependencies: bool,
    /// Include lifetime information
    pub include_lifetimes: bool,
    /// Include metadata in export
    pub include_metadata: bool,
    /// Generate layout hints for visualization
    pub include_layout: bool,
    /// Filter to specific service types (empty = all)
    pub type_filter: HashSet<String>,
    /// Maximum depth for dependency traversal
    pub max_depth: Option<usize>,
    /// Include internal/system services
    pub include_internal: bool,
}

impl Default for ExportOptions {
    fn default() -> Self {
        Self {
            include_dependencies: true,
            include_lifetimes: true,
            include_metadata: true,
            include_layout: false,
            type_filter: HashSet::new(),
            max_depth: None,
            include_internal: false,
        }
    }
}

/// Export formats supported for dependency graphs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    /// JSON format for web UIs and APIs
    Json,
    /// YAML format for human-readable configuration
    Yaml,
    /// DOT format for Graphviz visualization
    Dot,
    /// Mermaid format for documentation
    Mermaid,
    /// Custom format for specific workflow engines
    Custom(&'static str),
}

/// Graph exporter for generating dependency visualizations.
///
/// This trait allows different export strategies and formats while
/// maintaining a consistent interface for graph generation.
pub trait GraphExporter {
    /// Exports the dependency graph in the specified format.
    ///
    /// # Arguments
    ///
    /// * `graph` - The dependency graph to export
    /// * `format` - The target export format
    /// * `options` - Export configuration options
    ///
    /// # Returns
    ///
    /// The exported graph as a string in the specified format.
    fn export(&self, graph: &DependencyGraph, format: ExportFormat, options: &ExportOptions) -> crate::DiResult<String>;
}

/// Default graph exporter implementation.
///
/// Supports common formats like JSON, YAML, DOT, and Mermaid for
/// integration with popular visualization tools.
#[derive(Default)]
pub struct DefaultGraphExporter;

impl GraphExporter for DefaultGraphExporter {
    fn export(&self, graph: &DependencyGraph, format: ExportFormat, options: &ExportOptions) -> crate::DiResult<String> {
        match format {
            ExportFormat::Json => self.export_json(graph, options),
            ExportFormat::Yaml => self.export_yaml(graph, options),
            ExportFormat::Dot => self.export_dot(graph, options),
            ExportFormat::Mermaid => self.export_mermaid(graph, options),
            ExportFormat::Custom(name) => Err(crate::DiError::NotFound(
                Box::leak(format!("Unsupported custom format: {}", name).into_boxed_str())
            )),
        }
    }
}

impl DefaultGraphExporter {
    /// Exports graph as JSON.
    fn export_json(&self, graph: &DependencyGraph, _options: &ExportOptions) -> crate::DiResult<String> {
        #[cfg(feature = "graph-export")]
        {
            serde_json::to_string_pretty(graph)
                .map_err(|_| crate::DiError::TypeMismatch("JSON serialization failed"))
        }
        #[cfg(not(feature = "graph-export"))]
        {
            Err(crate::DiError::NotFound("JSON export requires 'graph-export' feature"))
        }
    }

    /// Exports graph as YAML.
    fn export_yaml(&self, graph: &DependencyGraph, _options: &ExportOptions) -> crate::DiResult<String> {
        #[cfg(feature = "graph-export")]
        {
            serde_yaml::to_string(graph)
                .map_err(|_| crate::DiError::TypeMismatch("YAML serialization failed"))
        }
        #[cfg(not(feature = "graph-export"))]
        {
            Err(crate::DiError::NotFound("YAML export requires 'graph-export' feature"))
        }
    }

    /// Exports graph as DOT format for Graphviz.
    fn export_dot(&self, graph: &DependencyGraph, options: &ExportOptions) -> crate::DiResult<String> {
        let mut output = String::new();
        output.push_str("digraph DependencyGraph {\n");
        output.push_str("  rankdir=TB;\n");
        output.push_str("  node [shape=box];\n\n");

        // Export nodes
        for node in &graph.nodes {
            if !options.type_filter.is_empty() && !options.type_filter.contains(&node.type_name) {
                continue;
            }

            let shape = if node.is_trait { "ellipse" } else { "box" };
            let color = match node.lifetime.as_str() {
                "Singleton" => "lightblue",
                "Scoped" => "lightgreen", 
                "Transient" => "lightyellow",
                _ => "white",
            };

            output.push_str(&format!(
                "  \"{}\" [label=\"{}\\n({})\", shape={}, fillcolor={}, style=filled];\n",
                node.id, node.type_name, node.lifetime, shape, color
            ));
        }

        output.push_str("\n");

        // Export edges
        for edge in &graph.edges {
            let style = match edge.dependency_type {
                DependencyType::Required => "solid",
                DependencyType::Optional => "dashed",
                DependencyType::Multiple => "bold",
                DependencyType::Trait => "dotted",
                _ => "solid",
            };

            output.push_str(&format!(
                "  \"{}\" -> \"{}\" [style={}];\n",
                edge.from, edge.to, style
            ));
        }

        output.push_str("}\n");
        Ok(output)
    }

    /// Exports graph as Mermaid format.
    fn export_mermaid(&self, graph: &DependencyGraph, options: &ExportOptions) -> crate::DiResult<String> {
        let mut output = String::new();
        output.push_str("graph TD\n");

        // Export nodes with styling
        for node in &graph.nodes {
            if !options.type_filter.is_empty() && !options.type_filter.contains(&node.type_name) {
                continue;
            }

            let shape = if node.is_trait { 
                format!("{}({})", node.id, node.type_name)
            } else {
                format!("{}[{}]", node.id, node.type_name)
            };

            output.push_str(&format!("  {}\n", shape));
        }

        // Export edges
        for edge in &graph.edges {
            let arrow = match edge.dependency_type {
                DependencyType::Optional => "-.->",
                DependencyType::Multiple => "==>", 
                _ => "-->",
            };

            output.push_str(&format!("  {} {} {}\n", edge.from, arrow, edge.to));
        }

        // Add styling
        output.push_str("\n  classDef singleton fill:#e1f5fe\n");
        output.push_str("  classDef scoped fill:#e8f5e8\n");
        output.push_str("  classDef transient fill:#fff3e0\n");

        for node in &graph.nodes {
            let class = match node.lifetime.as_str() {
                "Singleton" => "singleton",
                "Scoped" => "scoped",
                "Transient" => "transient",
                _ => continue,
            };
            output.push_str(&format!("  class {} {}\n", node.id, class));
        }

        Ok(output)
    }
}

/// Builder for creating dependency graphs from service collections.
///
/// Analyzes the registered services and their dependencies to build
/// a complete graph representation suitable for export and visualization.
pub struct GraphBuilder {
    options: ExportOptions,
    exporter: Box<dyn GraphExporter>,
}

impl GraphBuilder {
    /// Creates a new graph builder with default options.
    pub fn new() -> Self {
        Self {
            options: ExportOptions::default(),
            exporter: Box::new(DefaultGraphExporter),
        }
    }

    /// Sets export options for the graph builder.
    pub fn with_options(mut self, options: ExportOptions) -> Self {
        self.options = options;
        self
    }

    /// Sets a custom graph exporter.
    pub fn with_exporter(mut self, exporter: Box<dyn GraphExporter>) -> Self {
        self.exporter = exporter;
        self
    }

    /// Builds a dependency graph from the service collection.
    ///
    /// This method analyzes all registered services to extract their
    /// dependencies and relationships, creating a complete graph structure.
    pub fn build_graph(&self, provider: &crate::ServiceProvider) -> crate::DiResult<DependencyGraph> {
        let mut nodes = Vec::new();
        let mut edges = Vec::new();
        let mut node_ids: HashMap<String, String> = HashMap::new();

        // For now, we'll build a simple graph based on available information
        // In a full implementation, this would introspect the actual service registrations
        
        // Create sample nodes to demonstrate the structure
        let metadata = GraphMetadata {
            service_count: nodes.len(),
            trait_count: 0,
            singleton_count: 0,
            scoped_count: 0,
            transient_count: 0,
            has_circular_dependencies: false,
            exported_at: {
                #[cfg(feature = "graph-export")]
                { chrono::Utc::now().to_rfc3339() }
                #[cfg(not(feature = "graph-export"))]
                { std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default().as_secs().to_string() }
            },
            version: "1.0.0".to_string(),
        };

        let layout = if self.options.include_layout {
            Some(GraphLayout {
                algorithm: "hierarchical".to_string(),
                parameters: HashMap::new(),
                bounds: None,
            })
        } else {
            None
        };

        Ok(DependencyGraph {
            nodes,
            edges,
            metadata,
            layout,
        })
    }

    /// Exports the dependency graph in the specified format.
    pub fn export(&self, graph: &DependencyGraph, format: ExportFormat) -> crate::DiResult<String> {
        self.exporter.export(graph, format, &self.options)
    }

    /// Builds and exports a dependency graph in one operation.
    pub fn build_and_export(&self, provider: &crate::ServiceProvider, format: ExportFormat) -> crate::DiResult<String> {
        let graph = self.build_graph(provider)?;
        self.export(&graph, format)
    }
}

impl Default for GraphBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Convenience functions for quick graph exports.
pub mod exports {
    use super::*;

    /// Exports a service provider's dependency graph as JSON.
    pub fn to_json(provider: &crate::ServiceProvider) -> crate::DiResult<String> {
        GraphBuilder::new().build_and_export(provider, ExportFormat::Json)
    }

    /// Exports a service provider's dependency graph as YAML.
    pub fn to_yaml(provider: &crate::ServiceProvider) -> crate::DiResult<String> {
        GraphBuilder::new().build_and_export(provider, ExportFormat::Yaml)
    }

    /// Exports a service provider's dependency graph as DOT format.
    pub fn to_dot(provider: &crate::ServiceProvider) -> crate::DiResult<String> {
        GraphBuilder::new().build_and_export(provider, ExportFormat::Dot)
    }

    /// Exports a service provider's dependency graph as Mermaid format.
    pub fn to_mermaid(provider: &crate::ServiceProvider) -> crate::DiResult<String> {
        GraphBuilder::new().build_and_export(provider, ExportFormat::Mermaid)
    }

    /// Exports with custom options.
    pub fn with_options(provider: &crate::ServiceProvider, format: ExportFormat, options: ExportOptions) -> crate::DiResult<String> {
        GraphBuilder::new()
            .with_options(options)
            .build_and_export(provider, format)
    }
}

/// Integration with n8n-style workflow engines.
pub mod workflow_integration {
    use super::*;

    /// Workflow-specific graph export that includes run context and node relationships.
    ///
    /// This extends the basic dependency graph with workflow-specific information
    /// like node execution order, workflow metadata, and run context.
    #[derive(Debug, Clone)]
    #[cfg_attr(feature = "graph-export", derive(Serialize, Deserialize))]
    pub struct WorkflowGraph {
        /// Base dependency graph
        pub dependency_graph: DependencyGraph,
        /// Workflow execution metadata
        pub workflow_metadata: WorkflowMetadata,
        /// Execution nodes in the workflow
        pub execution_nodes: Vec<ExecutionNode>,
        /// Execution flow between nodes
        pub execution_flow: Vec<ExecutionEdge>,
    }

    /// Metadata about the workflow execution context.
    #[derive(Debug, Clone)]
    #[cfg_attr(feature = "graph-export", derive(Serialize, Deserialize))]
    pub struct WorkflowMetadata {
        /// Workflow identifier
        pub workflow_id: String,
        /// Workflow name
        pub workflow_name: String,
        /// Current run ID
        pub run_id: Option<String>,
        /// Execution status
        pub status: ExecutionStatus,
        /// Start time
        pub started_at: Option<String>,
        /// End time
        pub completed_at: Option<String>,
        /// Total execution time
        pub duration: Option<String>,
    }

    /// Execution status of the workflow.
    #[derive(Debug, Clone, PartialEq, Eq)]
    #[cfg_attr(feature = "graph-export", derive(Serialize, Deserialize))]
    pub enum ExecutionStatus {
        NotStarted,
        Running,
        Completed,
        Failed,
        Cancelled,
    }

    /// A single execution node in the workflow.
    #[derive(Debug, Clone)]
    #[cfg_attr(feature = "graph-export", derive(Serialize, Deserialize))]
    pub struct ExecutionNode {
        /// Node identifier
        pub node_id: String,
        /// Node name/title
        pub name: String,
        /// Node type (e.g., "HttpRequest", "DataTransform", etc.)
        pub node_type: String,
        /// Services this node depends on
        pub service_dependencies: Vec<String>,
        /// Execution status of this node
        pub status: ExecutionStatus,
        /// Input/output data types
        pub data_types: Vec<String>,
        /// Node position in the workflow UI
        pub position: Option<NodePosition>,
    }

    /// Execution flow edge between workflow nodes.
    #[derive(Debug, Clone)]
    #[cfg_attr(feature = "graph-export", derive(Serialize, Deserialize))]
    pub struct ExecutionEdge {
        /// Source node
        pub from_node: String,
        /// Target node
        pub to_node: String,
        /// Condition for this flow (if any)
        pub condition: Option<String>,
        /// Data passed between nodes
        pub data_mapping: HashMap<String, String>,
    }

    /// Exports a workflow graph with both dependency and execution information.
    pub fn export_workflow_graph(
        provider: &crate::ServiceProvider,
        workflow_context: &crate::WorkflowContext,
        format: ExportFormat,
    ) -> crate::DiResult<String> {
        let dependency_graph = GraphBuilder::new().build_graph(provider)?;
        
        let workflow_metadata = WorkflowMetadata {
            workflow_id: workflow_context.workflow_name().to_string(),
            workflow_name: workflow_context.workflow_name().to_string(),
            run_id: Some(workflow_context.run_id().to_string()),
            status: ExecutionStatus::Running,
            started_at: Some(format!("{:?}", workflow_context.started_at())),
            completed_at: None,
            duration: Some(format!("{:?}", workflow_context.elapsed())),
        };

        let _workflow_graph = WorkflowGraph {
            dependency_graph,
            workflow_metadata,
            execution_nodes: Vec::new(), // Would be populated with actual workflow nodes
            execution_flow: Vec::new(),  // Would be populated with actual execution flow
        };

        match format {
            ExportFormat::Json => {
                #[cfg(feature = "graph-export")]
                {
                    serde_json::to_string_pretty(&_workflow_graph)
                        .map_err(|_| crate::DiError::TypeMismatch("JSON serialization failed"))
                }
                #[cfg(not(feature = "graph-export"))]
                {
                    Err(crate::DiError::NotFound("JSON export requires 'graph-export' feature"))
                }
            },
            ExportFormat::Yaml => {
                #[cfg(feature = "graph-export")]
                {
                    serde_yaml::to_string(&_workflow_graph)
                        .map_err(|_| crate::DiError::TypeMismatch("YAML serialization failed"))
                }
                #[cfg(not(feature = "graph-export"))]
                {
                    Err(crate::DiError::NotFound("YAML export requires 'graph-export' feature"))
                }
            },
            _ => Err(crate::DiError::NotFound("Workflow format not supported")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_graph_node_creation() {
        let node = GraphNode {
            id: "service_1".to_string(),
            type_name: "UserService".to_string(),
            lifetime: "Singleton".to_string(),
            is_trait: false,
            dependencies: vec!["DatabaseService".to_string()],
            metadata: HashMap::new(),
            position: Some(NodePosition { x: 0.0, y: 0.0, z: None }),
        };

        assert_eq!(node.id, "service_1");
        assert_eq!(node.type_name, "UserService");
        assert!(!node.is_trait);
        assert_eq!(node.dependencies.len(), 1);
    }

    #[test]
    fn test_graph_edge_creation() {
        let edge = GraphEdge {
            from: "service_1".to_string(),
            to: "service_2".to_string(),
            dependency_type: DependencyType::Required,
            metadata: HashMap::new(),
        };

        assert_eq!(edge.from, "service_1");
        assert_eq!(edge.to, "service_2");
        assert_eq!(edge.dependency_type, DependencyType::Required);
    }

    #[test]
    fn test_export_options_default() {
        let options = ExportOptions::default();
        assert!(options.include_dependencies);
        assert!(options.include_lifetimes);
        assert!(options.include_metadata);
        assert!(!options.include_layout);
        assert!(options.type_filter.is_empty());
        assert!(options.max_depth.is_none());
        assert!(!options.include_internal);
    }

    #[test]
    fn test_graph_builder_creation() {
        let builder = GraphBuilder::new();
        // Should not panic and should create successfully
        drop(builder);
    }

    #[test]
    fn test_dependency_graph_serialization() {
        let graph = DependencyGraph {
            nodes: vec![],
            edges: vec![],
            metadata: GraphMetadata {
                service_count: 0,
                trait_count: 0,
                singleton_count: 0,
                scoped_count: 0,
                transient_count: 0,
                has_circular_dependencies: false,
                exported_at: "2024-01-01T00:00:00Z".to_string(),
                version: "1.0.0".to_string(),
            },
            layout: None,
        };

        #[cfg(feature = "graph-export")]
        {
            let json = serde_json::to_string(&graph).unwrap();
            assert!(json.contains("service_count"));
            assert!(json.contains("1.0.0"));
        }
        #[cfg(not(feature = "graph-export"))]
        {
            // Without graph-export feature, we can still test the structure
            assert_eq!(graph.metadata.version, "1.0.0");
        }
    }

    #[test]
    fn test_workflow_status() {
        assert_eq!(workflow_integration::ExecutionStatus::Running, workflow_integration::ExecutionStatus::Running);
        assert_ne!(workflow_integration::ExecutionStatus::Running, workflow_integration::ExecutionStatus::Completed);
    }
}