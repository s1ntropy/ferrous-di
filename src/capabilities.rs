//! Capability discovery and tool catalog functionality for agentic systems.
//!
//! This module provides infrastructure for discovering available tools and their
//! capabilities at runtime. Essential for agent planners that need to discover,
//! filter, and select appropriate tools based on requirements.

use std::any::{TypeId, type_name};
use std::collections::HashMap;
use std::sync::Arc;
use crate::{ServiceCollection, Key};

/// Metadata about a tool's capabilities and requirements.
///
/// This trait should be implemented by tools that want to expose their
/// capabilities to the agent planner. Tools can declare what they can do,
/// what they require, and other metadata useful for selection.
///
/// # Examples
///
/// ```
/// use ferrous_di::{ToolCapability, ServiceCollection};
/// use std::sync::Arc;
///
/// struct FileSearchTool {
///     root_dir: String,
/// }
///
/// impl ToolCapability for FileSearchTool {
///     fn name(&self) -> &str { "file_search" }
///     fn description(&self) -> &str { "Search for files matching patterns" }
///     fn version(&self) -> &str { "1.0.0" }
///     
///     fn capabilities(&self) -> Vec<&str> {
///         vec!["file_search", "pattern_matching", "filesystem_access"]
///     }
///     
///     fn requires(&self) -> Vec<&str> {
///         vec!["filesystem_read"]
///     }
///     
///     fn tags(&self) -> Vec<&str> {
///         vec!["files", "search", "core"]
///     }
/// }
/// ```
pub trait ToolCapability: Send + Sync {
    /// The unique name/identifier of this tool.
    fn name(&self) -> &str;
    
    /// Human-readable description of what this tool does.
    fn description(&self) -> &str;
    
    /// Version of this tool.
    fn version(&self) -> &str;
    
    /// List of capabilities this tool provides.
    /// 
    /// Capabilities are strings like "web_search", "file_read", "image_generation", etc.
    fn capabilities(&self) -> Vec<&str>;
    
    /// List of capabilities this tool requires to function.
    /// 
    /// For example, a file tool might require "filesystem_access".
    fn requires(&self) -> Vec<&str>;
    
    /// Optional tags for categorization and filtering.
    /// 
    /// Tags like "core", "experimental", "external", "local", etc.
    fn tags(&self) -> Vec<&str> {
        Vec::new()
    }
    
    /// Optional cost estimate for using this tool.
    /// 
    /// This could be monetary cost, computational cost, time cost, etc.
    /// Units are tool-specific but should be documented.
    fn estimated_cost(&self) -> Option<f64> {
        None
    }
    
    /// Optional reliability score (0.0 to 1.0).
    /// 
    /// How reliable/stable is this tool? 1.0 = always works, 0.0 = very unreliable.
    fn reliability(&self) -> Option<f64> {
        None
    }
}

/// Capability requirement for tool selection.
///
/// Used by planners to specify what capabilities they need when requesting
/// tool recommendations.
#[derive(Debug, Clone)]
pub struct CapabilityRequirement {
    /// The capability name that must be provided.
    pub capability: String,
    /// Whether this capability is required (vs nice-to-have).
    pub required: bool,
    /// Minimum version requirement (if applicable).
    pub min_version: Option<String>,
    /// Maximum acceptable cost for this capability.
    pub max_cost: Option<f64>,
    /// Minimum acceptable reliability for this capability.
    pub min_reliability: Option<f64>,
}

impl CapabilityRequirement {
    /// Creates a required capability.
    pub fn required(capability: impl Into<String>) -> Self {
        Self {
            capability: capability.into(),
            required: true,
            min_version: None,
            max_cost: None,
            min_reliability: None,
        }
    }
    
    /// Creates an optional capability.
    pub fn optional(capability: impl Into<String>) -> Self {
        Self {
            capability: capability.into(),
            required: false,
            min_version: None,
            max_cost: None,
            min_reliability: None,
        }
    }
    
    /// Sets minimum version requirement.
    pub fn min_version(mut self, version: impl Into<String>) -> Self {
        self.min_version = Some(version.into());
        self
    }
    
    /// Sets maximum acceptable cost.
    pub fn max_cost(mut self, cost: f64) -> Self {
        self.max_cost = Some(cost);
        self
    }
    
    /// Sets minimum acceptable reliability.
    pub fn min_reliability(mut self, reliability: f64) -> Self {
        self.min_reliability = Some(reliability);
        self
    }
}

/// Tool selection criteria for capability-based tool discovery.
#[derive(Debug, Default)]
pub struct ToolSelectionCriteria {
    /// Required and optional capabilities.
    pub capabilities: Vec<CapabilityRequirement>,
    /// Tags that tools must have.
    pub required_tags: Vec<String>,
    /// Tags that tools should not have.
    pub excluded_tags: Vec<String>,
    /// Maximum total cost across all selected tools.
    pub max_total_cost: Option<f64>,
    /// Minimum average reliability across selected tools.
    pub min_average_reliability: Option<f64>,
    /// Maximum number of tools to return.
    pub limit: Option<usize>,
}

impl ToolSelectionCriteria {
    /// Creates new selection criteria.
    pub fn new() -> Self {
        Self::default()
    }
    
    /// Adds a required capability.
    pub fn require(mut self, capability: impl Into<String>) -> Self {
        self.capabilities.push(CapabilityRequirement::required(capability));
        self
    }

    /// Adds a required capability with cost constraint.
    pub fn require_with_cost(mut self, capability: impl Into<String>, max_cost: f64) -> Self {
        self.capabilities.push(CapabilityRequirement::required(capability).max_cost(max_cost));
        self
    }
    
    /// Adds an optional capability.
    pub fn prefer(mut self, capability: impl Into<String>) -> Self {
        self.capabilities.push(CapabilityRequirement::optional(capability));
        self
    }
    
    /// Adds a required tag.
    pub fn require_tag(mut self, tag: impl Into<String>) -> Self {
        self.required_tags.push(tag.into());
        self
    }
    
    /// Adds an excluded tag.
    pub fn exclude_tag(mut self, tag: impl Into<String>) -> Self {
        self.excluded_tags.push(tag.into());
        self
    }
    
    /// Sets maximum total cost limit.
    pub fn max_cost(mut self, cost: f64) -> Self {
        self.max_total_cost = Some(cost);
        self
    }
    
    /// Sets minimum reliability requirement.
    pub fn min_reliability(mut self, reliability: f64) -> Self {
        self.min_average_reliability = Some(reliability);
        self
    }
    
    /// Sets maximum number of tools to return.
    pub fn limit(mut self, count: usize) -> Self {
        self.limit = Some(count);
        self
    }
}

/// Information about a discovered tool.
#[derive(Debug, Clone)]
pub struct ToolInfo {
    /// The service key for resolving this tool.
    pub key: Key,
    /// Tool name.
    pub name: String,
    /// Tool description.
    pub description: String,
    /// Tool version.
    pub version: String,
    /// Capabilities provided.
    pub capabilities: Vec<String>,
    /// Capabilities required.
    pub requires: Vec<String>,
    /// Tool tags.
    pub tags: Vec<String>,
    /// Estimated cost.
    pub estimated_cost: Option<f64>,
    /// Reliability score.
    pub reliability: Option<f64>,
}

impl ToolInfo {
    /// Checks if this tool satisfies a capability requirement.
    pub fn satisfies(&self, req: &CapabilityRequirement) -> bool {
        // Must provide the capability
        if !self.capabilities.contains(&req.capability) {
            return false;
        }
        
        // Check cost constraint
        if let Some(max_cost) = req.max_cost {
            if let Some(cost) = self.estimated_cost {
                if cost > max_cost {
                    return false;
                }
            }
        }
        
        // Check reliability constraint
        if let Some(min_reliability) = req.min_reliability {
            if let Some(reliability) = self.reliability {
                if reliability < min_reliability {
                    return false;
                }
            } else {
                // No reliability info - assume it doesn't meet requirement
                return false;
            }
        }
        
        // Version checking would go here if we implemented semver parsing
        // For now, skip version checks
        
        true
    }
}

/// Tool discovery result.
#[derive(Debug)]
pub struct ToolDiscoveryResult {
    /// Tools that match all required criteria.
    pub matching_tools: Vec<ToolInfo>,
    /// Tools that match some optional criteria.
    pub partial_matches: Vec<ToolInfo>,
    /// Required capabilities that couldn't be satisfied.
    pub unsatisfied_requirements: Vec<String>,
}

/// Registry of available tools and their capabilities.
pub(crate) struct CapabilityRegistry {
    /// Map from service keys to tool capability info.
    tools: HashMap<Key, ToolInfo>,
}

impl CapabilityRegistry {
    /// Creates a new empty capability registry.
    pub(crate) fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }
    
    /// Registers a tool's capabilities.
    pub(crate) fn register_tool<T: ?Sized + ToolCapability + 'static>(&mut self, key: Key, tool: &T) {
        let info = ToolInfo {
            key: key.clone(),
            name: tool.name().to_string(),
            description: tool.description().to_string(),
            version: tool.version().to_string(),
            capabilities: tool.capabilities().into_iter().map(|s| s.to_string()).collect(),
            requires: tool.requires().into_iter().map(|s| s.to_string()).collect(),
            tags: tool.tags().into_iter().map(|s| s.to_string()).collect(),
            estimated_cost: tool.estimated_cost(),
            reliability: tool.reliability(),
        };
        
        self.tools.insert(key, info);
    }
    
    /// Discovers tools based on selection criteria.
    pub(crate) fn discover(&self, criteria: &ToolSelectionCriteria) -> ToolDiscoveryResult {
        let mut matching_tools = Vec::new();
        let mut partial_matches = Vec::new();
        let mut unsatisfied_requirements = Vec::new();
        
        // Find required capabilities that no tool satisfies
        for req in &criteria.capabilities {
            if req.required {
                let satisfied = self.tools.values().any(|tool| tool.satisfies(req));
                if !satisfied {
                    unsatisfied_requirements.push(req.capability.clone());
                }
            }
        }
        
        // Score each tool
        for tool in self.tools.values() {
            let mut score = self.score_tool(tool, criteria);
            
            // Check required tags
            let has_required_tags = criteria.required_tags.iter()
                .all(|tag| tool.tags.contains(tag));
            
            // Check excluded tags
            let has_excluded_tags = criteria.excluded_tags.iter()
                .any(|tag| tool.tags.contains(tag));
            
            if !has_required_tags || has_excluded_tags {
                score = 0.0; // Disqualify
            }
            
            if score > 0.5 {
                matching_tools.push(tool.clone());
            } else if score > 0.0 {
                partial_matches.push(tool.clone());
            }
        }
        
        // Sort by score (would need to store scores, simplified for now)
        matching_tools.sort_by(|a, b| a.name.cmp(&b.name));
        partial_matches.sort_by(|a, b| a.name.cmp(&b.name));
        
        // Apply limit
        if let Some(limit) = criteria.limit {
            matching_tools.truncate(limit);
        }
        
        ToolDiscoveryResult {
            matching_tools,
            partial_matches,
            unsatisfied_requirements,
        }
    }
    
    /// Scores a tool against the selection criteria (0.0 to 1.0).
    fn score_tool(&self, tool: &ToolInfo, criteria: &ToolSelectionCriteria) -> f64 {
        let mut score = 0.0;
        let mut max_score = 0.0;
        
        // Score based on capability satisfaction
        for req in &criteria.capabilities {
            max_score += if req.required { 1.0 } else { 0.5 };
            
            if tool.satisfies(req) {
                score += if req.required { 1.0 } else { 0.5 };
            } else if req.required {
                // Failed required capability - disqualify
                return 0.0;
            }
        }
        
        if max_score == 0.0 {
            return 1.0; // No capability requirements, all tools qualify
        }
        
        score / max_score
    }
    
    /// Gets all registered tools.
    pub(crate) fn all_tools(&self) -> Vec<&ToolInfo> {
        self.tools.values().collect()
    }
    
    /// Gets tool info by key.
    pub(crate) fn get_tool(&self, key: &Key) -> Option<&ToolInfo> {
        self.tools.get(key)
    }
}

impl ServiceCollection {
    /// Registers a service as a tool with capabilities.
    ///
    /// This combines service registration with capability metadata registration,
    /// making the tool discoverable through the capability system.
    ///
    /// # Examples
    ///
    /// ```
    /// use ferrous_di::{ServiceCollection, ToolCapability};
    /// use std::sync::Arc;
    ///
    /// struct WebSearchTool {
    ///     api_key: String,
    /// }
    ///
    /// impl ToolCapability for WebSearchTool {
    ///     fn name(&self) -> &str { "web_search" }
    ///     fn description(&self) -> &str { "Search the web for information" }
    ///     fn version(&self) -> &str { "2.1.0" }
    ///     fn capabilities(&self) -> Vec<&str> { vec!["web_search", "information_retrieval"] }
    ///     fn requires(&self) -> Vec<&str> { vec!["internet_access", "api_key"] }
    ///     fn tags(&self) -> Vec<&str> { vec!["external", "search", "web"] }
    ///     fn estimated_cost(&self) -> Option<f64> { Some(0.001) } // $0.001 per query
    ///     fn reliability(&self) -> Option<f64> { Some(0.95) } // 95% reliable
    /// }
    ///
    /// let mut services = ServiceCollection::new();
    /// let tool = WebSearchTool {
    ///     api_key: "secret-key".to_string(),
    /// };
    /// services.add_tool_singleton(tool);
    /// ```
    pub fn add_tool_singleton<T>(&mut self, tool: T) -> &mut Self
    where
        T: ToolCapability + Send + Sync + 'static,
    {
        // Register capabilities first
        let key = Key::Type(TypeId::of::<T>(), type_name::<T>());
        self.capabilities.register_tool(key.clone(), &tool);
        
        // Then register as regular singleton
        self.add_singleton(tool);
        self
    }
    
    /// Registers a trait as a tool with capabilities.
    ///
    /// # Examples
    ///
    /// ```
    /// use ferrous_di::{ServiceCollection, ToolCapability};
    /// use std::sync::Arc;
    ///
    /// trait SearchTool: ToolCapability + Send + Sync {
    ///     fn search(&self, query: &str) -> Vec<String>;
    /// }
    ///
    /// struct GoogleSearchTool;
    ///
    /// impl ToolCapability for GoogleSearchTool {
    ///     fn name(&self) -> &str { "google_search" }
    ///     fn description(&self) -> &str { "Search using Google" }
    ///     fn version(&self) -> &str { "1.0.0" }
    ///     fn capabilities(&self) -> Vec<&str> { vec!["web_search"] }
    ///     fn requires(&self) -> Vec<&str> { vec!["internet"] }
    /// }
    ///
    /// impl SearchTool for GoogleSearchTool {
    ///     fn search(&self, query: &str) -> Vec<String> {
    ///         // Implementation here
    ///         vec![format!("Results for: {}", query)]
    ///     }
    /// }
    ///
    /// let mut services = ServiceCollection::new();
    /// let tool = Arc::new(GoogleSearchTool);
    /// services.add_tool_trait::<dyn SearchTool>(tool);
    /// ```
    pub fn add_tool_trait<T>(&mut self, tool: Arc<T>) -> &mut Self
    where
        T: ?Sized + ToolCapability + Send + Sync + 'static,
    {
        // Register capabilities first
        let key = Key::Trait(type_name::<T>());
        self.capabilities.register_tool(key.clone(), tool.as_ref());
        
        // Then register as trait
        self.add_singleton_trait::<T>(tool);
        self
    }
}

// Implementation is in provider/mod.rs to access inner struct