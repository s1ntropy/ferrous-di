//! Tests for agent-focused features like decoration, observers, prewarm, and capabilities.

use ferrous_di::{ServiceCollection, DiObserver, LoggingObserver, Resolver, ScopeLocal, ToolCapability, ToolSelectionCriteria, CapabilityRequirement, ValidationBuilder, Lifetime, ValidationError, FastSingletonCache};
use std::sync::{Arc, atomic::{AtomicU32, Ordering}};

#[test]
fn test_decorate_trait_basic() {
    trait Tool: Send + Sync {
        fn execute(&self, input: &str) -> String;
    }

    struct FileTool;
    impl Tool for FileTool {
        fn execute(&self, input: &str) -> String {
            format!("File: {}", input)
        }
    }

    struct LoggingWrapper<T: ?Sized> {
        inner: Arc<T>,
        counter: Arc<AtomicU32>,
    }

    impl<T: ?Sized + Tool> Tool for LoggingWrapper<T> {
        fn execute(&self, input: &str) -> String {
            self.counter.fetch_add(1, Ordering::Relaxed);
            let result = self.inner.execute(input);
            format!("Logged: {}", result)
        }
    }

    let mut services = ServiceCollection::new();
    let counter = Arc::new(AtomicU32::new(0));
    let counter_clone = counter.clone();
    
    // Register tool
    services.add_singleton_trait::<dyn Tool>(Arc::new(FileTool));
    
    // Apply logging decoration
    services.decorate_trait::<dyn Tool, _>(move |tool| {
        Arc::new(LoggingWrapper {
            inner: tool,
            counter: counter_clone.clone(),
        })
    });
    
    let provider = services.build();
    let tool = provider.get_required_trait::<dyn Tool>();
    
    let result = tool.execute("test.txt");
    assert_eq!(result, "Logged: File: test.txt");
    assert_eq!(counter.load(Ordering::Relaxed), 1);
}

#[test]
fn test_observer_basic() {
    struct CountingObserver {
        counter: Arc<AtomicU32>,
    }
    
    impl DiObserver for CountingObserver {
        fn resolving(&self, _key: &ferrous_di::Key) {
            self.counter.fetch_add(1, Ordering::Relaxed);
        }
        
        fn resolved(&self, _key: &ferrous_di::Key, _duration: std::time::Duration) {
            self.counter.fetch_add(1, Ordering::Relaxed);
        }
        
        fn factory_panic(&self, _key: &ferrous_di::Key, _message: &str) {
            // Not tested here
        }
    }
    
    let mut services = ServiceCollection::new();
    let counter = Arc::new(AtomicU32::new(0));
    
    services.add_observer(Arc::new(CountingObserver { 
        counter: counter.clone() 
    }));
    services.add_singleton(42usize);
    services.add_transient_factory::<String, _>(|_| "hello".to_string());
    
    let provider = services.build();
    
    // Each resolution should trigger resolving + resolved
    let _num = provider.get_required::<usize>();
    assert_eq!(counter.load(Ordering::Relaxed), 2); // resolving + resolved
    
    let _str = provider.get_required::<String>();
    assert_eq!(counter.load(Ordering::Relaxed), 4); // +2 more
}

#[test]
fn test_prewarm_marks_services() {
    let mut services = ServiceCollection::new();
    services.add_singleton(42usize);
    services.prewarm::<usize>();
    
    // For now, just test that the methods exist and can be called
    // Full integration testing would require completing the ready() implementation
    let _provider = services.build();
    // When ready() is fully implemented: provider.ready().await
}

#[test] 
fn test_multiple_decorations_compose() {
    trait Calculator: Send + Sync {
        fn add(&self, a: i32, b: i32) -> i32;
    }
    
    struct SimpleCalculator;
    impl Calculator for SimpleCalculator {
        fn add(&self, a: i32, b: i32) -> i32 { a + b }
    }
    
    struct MultiplyByTwoWrapper<T: ?Sized>(Arc<T>);
    impl<T: ?Sized + Calculator> Calculator for MultiplyByTwoWrapper<T> {
        fn add(&self, a: i32, b: i32) -> i32 { self.0.add(a, b) * 2 }
    }
    
    struct AddOneWrapper<T: ?Sized>(Arc<T>);
    impl<T: ?Sized + Calculator> Calculator for AddOneWrapper<T> {
        fn add(&self, a: i32, b: i32) -> i32 { self.0.add(a, b) + 1 }
    }
    
    let mut services = ServiceCollection::new();
    services.add_singleton_trait::<dyn Calculator>(Arc::new(SimpleCalculator));
    
    // Apply decorations in order
    services.decorate_trait::<dyn Calculator, _>(|calc| Arc::new(MultiplyByTwoWrapper(calc)));
    services.decorate_trait::<dyn Calculator, _>(|calc| Arc::new(AddOneWrapper(calc)));
    
    let provider = services.build();
    let calc = provider.get_required_trait::<dyn Calculator>();
    
    // Should be: ((3 + 2) * 2) + 1 = 11
    assert_eq!(calc.add(3, 2), 11);
}

#[test]
fn test_scope_local_basic() {
    #[derive(Default)]
    struct RunContext {
        trace_id: String,
        max_steps: u32,
    }

    let mut services = ServiceCollection::new();
    
    // Register scope-local context
    let counter = Arc::new(AtomicU32::new(0));
    let counter_clone = counter.clone();
    
    services.add_scope_local::<RunContext, _>(move |_resolver| {
        let id = counter_clone.fetch_add(1, Ordering::Relaxed);
        Arc::new(RunContext {
            trace_id: format!("trace-{}", id),
            max_steps: 100,
        })
    });

    // Service that uses the context
    services.add_scoped_factory::<String, _>(|resolver| {
        let ctx = resolver.get_required::<ScopeLocal<RunContext>>();
        format!("Processing with {}, max_steps: {}", ctx.trace_id, ctx.max_steps)
    });

    let provider = services.build();
    
    // Each scope gets its own context
    let scope1 = provider.create_scope();
    let scope2 = provider.create_scope();
    
    let result1 = scope1.get_required::<String>();
    let result2 = scope2.get_required::<String>();
    
    // Should have different trace IDs
    assert_eq!(result1.as_str(), "Processing with trace-0, max_steps: 100");
    assert_eq!(result2.as_str(), "Processing with trace-1, max_steps: 100");
    
    // Within the same scope, should get the same context
    let result1_again = scope1.get_required::<String>();
    assert_eq!(result1, result1_again);
}

#[test]
fn test_scope_local_multiple_types() {
    struct TraceContext { trace_id: String }
    struct BudgetContext { tokens: u32 }
    
    let mut services = ServiceCollection::new();
    
    services.add_scope_local::<TraceContext, _>(|_r| {
        Arc::new(TraceContext { 
            trace_id: "trace-123".to_string() 
        })
    });
    
    services.add_scope_local::<BudgetContext, _>(|_r| {
        Arc::new(BudgetContext { tokens: 1000 })
    });
    
    services.add_scoped_factory::<String, _>(|resolver| {
        let trace = resolver.get_required::<ScopeLocal<TraceContext>>();
        let budget = resolver.get_required::<ScopeLocal<BudgetContext>>();
        format!("Trace: {} Budget: {}", trace.trace_id, budget.tokens)
    });
    
    let provider = services.build();
    let scope = provider.create_scope();
    let result = scope.get_required::<String>();
    
    assert_eq!(result.as_str(), "Trace: trace-123 Budget: 1000");
}

#[test]
fn test_capability_discovery_basic() {
    struct FileSearchTool;
    
    impl ToolCapability for FileSearchTool {
        fn name(&self) -> &str { "file_search" }
        fn description(&self) -> &str { "Search for files by pattern" }
        fn version(&self) -> &str { "1.0.0" }
        fn capabilities(&self) -> Vec<&str> { 
            vec!["file_search", "pattern_matching", "filesystem_read"] 
        }
        fn requires(&self) -> Vec<&str> { vec!["filesystem_access"] }
        fn tags(&self) -> Vec<&str> { vec!["core", "files"] }
        fn reliability(&self) -> Option<f64> { Some(0.95) }
    }
    
    struct WebSearchTool;
    
    impl ToolCapability for WebSearchTool {
        fn name(&self) -> &str { "web_search" }
        fn description(&self) -> &str { "Search the web for information" }
        fn version(&self) -> &str { "2.1.0" }
        fn capabilities(&self) -> Vec<&str> { vec!["web_search", "information_retrieval"] }
        fn requires(&self) -> Vec<&str> { vec!["internet_access"] }
        fn tags(&self) -> Vec<&str> { vec!["external", "search"] }
        fn estimated_cost(&self) -> Option<f64> { Some(0.01) }
        fn reliability(&self) -> Option<f64> { Some(0.90) }
    }

    let mut services = ServiceCollection::new();
    services.add_tool_singleton(FileSearchTool);
    services.add_tool_singleton(WebSearchTool);

    let provider = services.build();

    // Test discovery - find search tools
    let criteria = ToolSelectionCriteria::new()
        .require("file_search");

    let result = provider.discover_tools(&criteria);
    
    assert_eq!(result.matching_tools.len(), 1);
    assert_eq!(result.matching_tools[0].name, "file_search");
    assert_eq!(result.unsatisfied_requirements.len(), 0);

    // Test discovery with cost constraint
    let criteria = ToolSelectionCriteria::new()
        .require_with_cost("web_search", 0.005); // Lower than web search cost

    let result = provider.discover_tools(&criteria);
    assert_eq!(result.matching_tools.len(), 0); // Should be filtered out by cost

    // Test discovery with reliability requirement
    let criteria = ToolSelectionCriteria::new()
        .require("file_search")
        .min_reliability(0.9);

    let result = provider.discover_tools(&criteria);
    assert_eq!(result.matching_tools.len(), 1);
    assert!(result.matching_tools[0].reliability.unwrap() >= 0.9);
}

#[test]
fn test_capability_discovery_tags() {
    struct CoreTool;
    impl ToolCapability for CoreTool {
        fn name(&self) -> &str { "core_tool" }
        fn description(&self) -> &str { "Core functionality" }
        fn version(&self) -> &str { "1.0.0" }
        fn capabilities(&self) -> Vec<&str> { vec!["core_capability"] }
        fn requires(&self) -> Vec<&str> { Vec::new() }
        fn tags(&self) -> Vec<&str> { vec!["core", "stable"] }
    }

    struct ExperimentalTool;
    impl ToolCapability for ExperimentalTool {
        fn name(&self) -> &str { "experimental_tool" }
        fn description(&self) -> &str { "Experimental functionality" }
        fn version(&self) -> &str { "0.1.0" }
        fn capabilities(&self) -> Vec<&str> { vec!["experimental_capability"] }
        fn requires(&self) -> Vec<&str> { Vec::new() }
        fn tags(&self) -> Vec<&str> { vec!["experimental", "unstable"] }
    }

    let mut services = ServiceCollection::new();
    services.add_tool_singleton(CoreTool);
    services.add_tool_singleton(ExperimentalTool);

    let provider = services.build();

    // Test tag filtering - only stable tools
    let criteria = ToolSelectionCriteria::new()
        .require_tag("stable")
        .exclude_tag("experimental");

    let result = provider.discover_tools(&criteria);
    assert_eq!(result.matching_tools.len(), 1);
    assert_eq!(result.matching_tools[0].name, "core_tool");

    // Test discovery of all tools
    let all_tools = provider.list_all_tools();
    assert_eq!(all_tools.len(), 2);
}

#[test]
fn test_capability_tool_traits() {
    trait SearchTool: ToolCapability + Send + Sync {
        fn search(&self, query: &str) -> String;
    }

    struct GoogleSearchTool;

    impl ToolCapability for GoogleSearchTool {
        fn name(&self) -> &str { "google_search" }
        fn description(&self) -> &str { "Search using Google" }
        fn version(&self) -> &str { "3.0.0" }
        fn capabilities(&self) -> Vec<&str> { vec!["web_search", "google"] }
        fn requires(&self) -> Vec<&str> { vec!["internet", "api_key"] }
        fn tags(&self) -> Vec<&str> { vec!["search", "web", "google"] }
        fn estimated_cost(&self) -> Option<f64> { Some(0.002) }
    }

    impl SearchTool for GoogleSearchTool {
        fn search(&self, query: &str) -> String {
            format!("Google results for: {}", query)
        }
    }

    let mut services = ServiceCollection::new();
    let tool = Arc::new(GoogleSearchTool);
    services.add_tool_trait::<dyn SearchTool>(tool.clone());

    let provider = services.build();

    // Verify the tool is discoverable
    let criteria = ToolSelectionCriteria::new()
        .require("google");

    let result = provider.discover_tools(&criteria);
    assert_eq!(result.matching_tools.len(), 1);
    assert_eq!(result.matching_tools[0].name, "google_search");

    // Verify we can also resolve the trait
    let search_service = provider.get_required_trait::<dyn SearchTool>();
    let search_result = search_service.search("rust programming");
    assert_eq!(search_result, "Google results for: rust programming");
}

#[test]
fn test_build_time_validation() {
    use ferrous_di::{ValidationBuilder, Lifetime};

    // Test valid configuration
    let result = ValidationBuilder::new()
        .register::<String>(Lifetime::Singleton)
        .register::<u32>(Lifetime::Transient)
        .depends_on::<String, u32>() // Singleton can depend on transient (warning)
        .validate_runtime();

    assert!(result.is_valid()); // No errors
    assert!(result.has_warnings()); // Should have warning about singleton -> transient

    // Test invalid configuration  
    let result = ValidationBuilder::new()
        .register::<String>(Lifetime::Singleton) 
        .register::<u32>(Lifetime::Scoped)
        .depends_on::<String, u32>() // Singleton cannot depend on scoped (error)
        .validate_runtime();

    assert!(!result.is_valid()); // Has errors
    assert_eq!(result.errors.len(), 1);

    match &result.errors[0] {
        ferrous_di::ValidationError::SingletonDependsOnScoped { singleton, scoped, .. } => {
            assert!(singleton.contains("String"));
            assert!(scoped.contains("u32"));
        }
        _ => panic!("Expected SingletonDependsOnScoped error"),
    }
}

#[test]
fn test_fast_singleton_cache() {
    use ferrous_di::{ServiceCollection, FastSingletonCache};
    use std::sync::{Arc, atomic::{AtomicU32, Ordering}};

    struct ExpensiveService {
        init_count: Arc<AtomicU32>,
        value: String,
    }

    impl ExpensiveService {
        fn new(counter: Arc<AtomicU32>) -> Self {
            counter.fetch_add(1, Ordering::Relaxed);
            // Simulate expensive initialization
            Self {
                init_count: counter,
                value: "expensive_resource".to_string(),
            }
        }
    }

    let mut services = ServiceCollection::new();
    let counter = Arc::new(AtomicU32::new(0));
    let counter_clone = counter.clone();

    services.add_singleton_factory::<ExpensiveService, _>(move |_| {
        ExpensiveService::new(counter_clone.clone())
    });

    let provider = services.build();

    // First access - should initialize
    let service1 = provider.get_required::<ExpensiveService>();
    assert_eq!(counter.load(Ordering::Relaxed), 1);

    // Subsequent accesses - should use cached value (fast path)
    for _ in 0..100 {
        let service_n = provider.get_required::<ExpensiveService>();
        assert!(Arc::ptr_eq(&service1, &service_n)); // Same instance
    }

    // Factory should only have been called once
    assert_eq!(counter.load(Ordering::Relaxed), 1);
}

#[test]
fn test_validation_circular_dependency() {
    use ferrous_di::{ValidationBuilder, Lifetime};

    let result = ValidationBuilder::new()
        .register::<String>(Lifetime::Singleton)
        .register::<u32>(Lifetime::Singleton) 
        .depends_on::<String, u32>()
        .depends_on::<u32, String>() // Circular!
        .validate_runtime();

    assert!(!result.is_valid());
    assert_eq!(result.errors.len(), 1);

    match &result.errors[0] {
        ferrous_di::ValidationError::CircularDependency { cycle } => {
            assert!(cycle.len() >= 2);
            // Should contain both String and u32 in the cycle
            let cycle_names: Vec<_> = cycle.iter().map(|(name, _)| *name).collect();
            assert!(cycle_names.iter().any(|name| name.contains("String")));
            assert!(cycle_names.iter().any(|name| name.contains("u32")));
        }
        _ => panic!("Expected CircularDependency error"),
    }
}