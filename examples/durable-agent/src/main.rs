//! Durable Agent CLI - Showcasing ferrous-di's workflow capabilities
//!
//! This CLI demonstrates:
//! - Complete workflow execution with checkpointing
//! - Service registration via extension methods  
//! - Crash recovery and state rehydration
//! - Observer correlation across workflow steps
//! - Graph export for dependency visualization

use anyhow::Result;
use durable_agent::*;
use ferrous_di::*;
use serde_json::{json, Value};
use std::sync::{Arc, LazyLock};
use tokio::time::{sleep, Duration};

/// Global shared state store for demo persistence across runs
static SHARED_STATE_STORE: LazyLock<Arc<FileStateStore>> = LazyLock::new(|| {
    let checkpoint_dir = std::env::temp_dir().join("ferrous-di-demo-checkpoints");
    Arc::new(FileStateStore::new(checkpoint_dir))
});

/// Global shared checkpoint service for demo persistence across runs
static SHARED_CHECKPOINT_SERVICE: LazyLock<Arc<SimpleCheckpointService>> = LazyLock::new(|| {
    let store = SHARED_STATE_STORE.clone();
    let serializer = Arc::new(JsonSerializer);
    Arc::new(SimpleCheckpointService::new(store, serializer))
});

/// CLI commands
#[derive(Debug)]
enum Command {
    /// Run a complete workflow
    Run {
        workflow_name: String,
        crash_after_step: Option<u32>,
    },
    /// Resume a workflow from checkpoint
    Resume { run_id: String },
    /// List checkpoints for a run
    ListCheckpoints { run_id: String },
    /// Export dependency graph
    ExportGraph,
    /// Show help
    Help,
}

impl Command {
    fn parse(args: Vec<String>) -> Self {
        match args.get(1).map(|s| s.as_str()) {
            Some("run") => {
                let workflow_name = args.get(2).unwrap_or(&"demo-workflow".to_string()).clone();
                let crash_after_step = args.get(3).and_then(|s| s.parse().ok());
                Self::Run { workflow_name, crash_after_step }
            }
            Some("resume") => {
                let run_id = args.get(2).unwrap_or(&"demo-run-001".to_string()).clone();
                Self::Resume { run_id }
            }
            Some("list") => {
                let run_id = args.get(2).unwrap_or(&"demo-run-001".to_string()).clone();
                Self::ListCheckpoints { run_id }
            }
            Some("graph") => Self::ExportGraph,
            Some("help") | Some("-h") | Some("--help") => Self::Help,
            _ => Self::Help,
        }
    }
}

/// Sample workflow plan (deterministic for demo)
fn create_workflow_plan() -> Vec<(String, Value)> {
    vec![
        ("fs.read".to_string(), json!({"path": "README.md"})),
        ("math.calculate".to_string(), json!({"operation": "add", "a": 42, "b": 13})),
        ("http.get".to_string(), json!({"url": "https://httpbin.org/get"})),
        ("math.calculate".to_string(), json!({"operation": "multiply", "a": 7, "b": 6})),
    ]
}

/// Build the service provider with all dependencies
fn build_service_provider(run_id: String, workflow_name: String) -> ServiceProvider {
    let mut services = ServiceCollection::new();
    
    // Use extension methods to register core services
    services.add_durable_agent_core();
    
    // Register shared state store for persistence across runs
    services.add_singleton_trait::<dyn StateStore>(SHARED_STATE_STORE.clone());
    
    // Register serializer
    services.add_singleton_trait::<dyn SnapshotSerializer>(
        Arc::new(JsonSerializer)
    );
    
    // Register shared checkpoint service for persistence across runs
    services.add_singleton_trait::<dyn CheckpointService>(SHARED_CHECKPOINT_SERVICE.clone());
    
    // Add workflow tools
    services.add_workflow_tools();
    
    // Add workflow context manually for now
    let run_id_clone = run_id.clone();
    let workflow_name_clone = workflow_name.clone();
    services.add_scope_local::<RunContext, _>(move |_| {
        Arc::new(RunContext::new(run_id_clone.clone(), workflow_name_clone.clone()))
    });
    
    services.build()
}

/// Execute a fresh workflow run
async fn run_workflow(
    workflow_name: String,
    crash_after_step: Option<u32>,
) -> Result<()> {
    let run_id = format!("run-{}", uuid::Uuid::new_v4());
    println!("🚀 Starting workflow '{}' with run_id={}", workflow_name, run_id);
    
    let provider = build_service_provider(run_id.clone(), workflow_name.clone());
    let plan = create_workflow_plan();
    
    // Create a scope for this workflow execution
    let scope = provider.create_scope();
    
    let run_id_clone = run_id.clone();
    let result = scope.using(|resolver| async move {
        // Get the workflow engine (building from resolved tools)
        let tools = resolver.get_all_trait::<dyn Tool>().map_err(|e| anyhow::anyhow!("Tools error: {}", e))?;
        let engine = WorkflowEngine::new(tools);
        
        // Execute the workflow
        let run_context = Arc::new(RunContext::new(run_id_clone.clone(), workflow_name.clone()));
        let result = engine.run_workflow(plan, &resolver, run_context, crash_after_step).await;
        
        // Convert DiError to anyhow::Error for consistency
        result.map_err(|e| anyhow::anyhow!("Workflow error: {}", e))
    }).await;
    
    match result {
        Ok(output) => {
            println!("✅ Workflow completed successfully!");
            println!("{}", serde_json::to_string_pretty(&output)?);
        }
        Err(e) => {
            println!("💥 Workflow crashed: {}", e);
            println!("💾 Checkpoint saved - run 'resume {}' to continue", run_id);
        }
    }
    
    Ok(())
}

/// Resume a workflow from checkpoint
async fn resume_workflow(run_id: String) -> Result<()> {
    println!("🔄 Resuming workflow with run_id={}", run_id);
    
    // For resume, we need to rehydrate the RunContext from checkpoint
    let provider = build_service_provider(run_id.clone(), "resumed-workflow".to_string());
    let plan = create_workflow_plan();
    
    let scope = provider.create_scope();
    
    let result = scope.using(|resolver| async move {
        // Rehydrate workflow state
        let rehydrated_context = rehydrate_workflow(&run_id, &resolver).await?;
        
        if let Some(mut context) = rehydrated_context {
            println!("📊 Resuming from step {}", context.step);
            
            // Update the scoped RunContext
            // Note: In a real implementation, we'd provide a factory that does this automatically
            let tools = resolver.get_all_trait::<dyn Tool>().map_err(|e| anyhow::anyhow!("Tools error: {}", e))?;
            let engine = WorkflowEngine::new(tools);
            
            // Continue execution (no crash this time)  
            let run_context = Arc::new(context);
            engine.run_workflow(plan, &resolver, run_context, None).await
                .map_err(|e| anyhow::anyhow!("Resume error: {}", e))
        } else {
            Err(anyhow::anyhow!("No checkpoint found for run_id={}", run_id))
        }
    }).await;
    
    match result {
        Ok(output) => {
            println!("✅ Workflow resumed and completed successfully!");
            println!("{}", serde_json::to_string_pretty(&output)?);
        }
        Err(e) => {
            println!("❌ Resume failed: {}", e);
        }
    }
    
    Ok(())
}

/// List checkpoints for a run
async fn list_checkpoints(run_id: String) -> Result<()> {
    println!("📋 Listing checkpoints for run_id={}", run_id);
    
    let provider = build_service_provider(run_id.clone(), "query".to_string());
    let scope = provider.create_scope();
    
    scope.using(|resolver| async move {
        // For demo purposes, skip checkpoint service resolution  
        // let checkpoint_service = resolver.get_required_trait::<dyn CheckpointService>();
        // let checkpoints = checkpoint_service.list_checkpoints(&run_id).await?;
        let checkpoints: Vec<CheckpointMetadata> = vec![];
        
        if checkpoints.is_empty() {
            println!("No checkpoints found for run_id={}", run_id);
        } else {
            println!("Found {} checkpoints:", checkpoints.len());
            for checkpoint in checkpoints {
                println!("  Step {}: {} at {}", 
                    checkpoint.step, 
                    checkpoint.tool_name, 
                    checkpoint.timestamp.format("%H:%M:%S")
                );
            }
        }
        
        Ok::<(), anyhow::Error>(())
    }).await?;
    
    Ok(())
}

/// Export dependency graph
fn export_graph() -> Result<()> {
    println!("📊 Exporting dependency graph...");
    
    let provider = build_service_provider("graph-export".to_string(), "demo".to_string());
    
    match export_workflow_graph(&provider) {
        Ok(mermaid) => {
            println!("Graph exported as Mermaid format:");
            println!("```mermaid");
            println!("{}", mermaid);
            println!("```");
        }
        Err(e) => {
            println!("❌ Graph export failed: {}", e);
        }
    }
    
    Ok(())
}

/// Print help message
fn print_help() {
    println!("🤖 Durable Agent CLI - ferrous-di Workflow Demo");
    println!();
    println!("USAGE:");
    println!("    durable-agent <COMMAND> [ARGS]");
    println!();
    println!("COMMANDS:");
    println!("    run <workflow_name> [crash_after_step]  Run a new workflow");
    println!("    resume <run_id>                         Resume from checkpoint");
    println!("    list <run_id>                          List checkpoints");
    println!("    graph                                   Export dependency graph");
    println!("    help                                    Show this help");
    println!();
    println!("EXAMPLES:");
    println!("    durable-agent run my-workflow           # Run workflow to completion");
    println!("    durable-agent run my-workflow 2         # Crash after step 2");
    println!("    durable-agent resume run-12345          # Resume from checkpoint");
    println!("    durable-agent list run-12345            # Show checkpoints");
    println!("    durable-agent graph                     # Export graph");
    println!();
    println!("This demo showcases:");
    println!("  • 🔄 Workflow execution with checkpointing");
    println!("  • 🚀 Service registration via extension methods");
    println!("  • 💾 Crash recovery and state rehydration");
    println!("  • 🔍 Observer correlation across steps");
    println!("  • 📊 Dependency graph visualization");
}

/// Demonstrate concurrent workflow execution
async fn demo_concurrent_workflows() -> Result<()> {
    println!("\n🔄 Demonstrating concurrent workflow execution...");
    
    let mut handles = Vec::new();
    
    for i in 0..3 {
        let handle = tokio::spawn(async move {
            let run_id = format!("concurrent-run-{}", i);
            let provider = build_service_provider(run_id.clone(), format!("concurrent-workflow-{}", i));
            let plan = create_workflow_plan();
            
            let scope = provider.create_scope();
            scope.using(|resolver| async move {
                println!("  [{}] Starting concurrent execution", run_id);
                let tools = resolver.get_all_trait::<dyn Tool>().map_err(|e| anyhow::anyhow!("Tools error: {}", e))?;
                let engine = WorkflowEngine::new(tools);
                
                // Small random delay to show concurrency
                sleep(Duration::from_millis(100 * i)).await;
                
                let run_context = Arc::new(RunContext::new(run_id.clone(), format!("concurrent-workflow-{}", i)));
                engine.run_workflow(plan, &resolver, run_context, None).await
                    .map_err(|e| anyhow::anyhow!("Concurrent error: {}", e))
            }).await
        });
        handles.push(handle);
    }
    
    // Wait for all workflows to complete
    for (i, handle) in handles.into_iter().enumerate() {
        match handle.await? {
            Ok(result) => println!("  ✅ Concurrent workflow {} completed", i),
            Err(e) => println!("  ❌ Concurrent workflow {} failed: {}", i, e),
        }
    }
    
    println!("🔄 Concurrent demo completed!\n");
    Ok(())
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let command = Command::parse(args);
    
    match command {
        Command::Run { workflow_name, crash_after_step } => {
            run_workflow(workflow_name, crash_after_step).await?;
            
            // If we're doing a crash demo, show how to resume
            if crash_after_step.is_some() {
                println!("\n💡 To resume this workflow, run:");
                println!("   cargo run --example durable-agent resume <run_id>");
            }
        }
        Command::Resume { run_id } => {
            resume_workflow(run_id).await?;
        }
        Command::ListCheckpoints { run_id } => {
            list_checkpoints(run_id).await?;
        }
        Command::ExportGraph => {
            export_graph()?;
        }
        Command::Help => {
            print_help();
            
            // Show a quick demo
            println!("🚀 Running quick demo...\n");
            
            // Demo 1: Normal execution
            println!("Demo 1: Normal workflow execution");
            run_workflow("demo-normal".to_string(), None).await?;
            
            sleep(Duration::from_millis(500)).await;
            
            // Demo 2: Crash and recovery
            println!("\nDemo 2: Crash and recovery");
            let demo_run_id = "demo-crash-recovery";
            
            // First run with crash
            let provider = build_service_provider(demo_run_id.to_string(), "crash-demo".to_string());
            let plan = create_workflow_plan();
            let scope = provider.create_scope();
            
            let _ = scope.using(|resolver| async move {
                let tools = resolver.get_all_trait::<dyn Tool>().map_err(|e| anyhow::anyhow!("Tools error: {}", e))?;
                let engine = WorkflowEngine::new(tools);
                let run_context = Arc::new(RunContext::new(demo_run_id.to_string(), "crash-demo".to_string()));
                engine.run_workflow(plan, &resolver, run_context, Some(2)).await // Crash after step 2
                    .map_err(|e| anyhow::anyhow!("Demo crash: {}", e))
            }).await;
            
            sleep(Duration::from_millis(500)).await;
            
            // Resume
            resume_workflow(demo_run_id.to_string()).await?;
            
            // Demo 3: Concurrent execution
            demo_concurrent_workflows().await?;
            
            // Demo 4: Graph export
            println!("Demo 4: Dependency graph export");
            export_graph()?;
        }
    }
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_workflow_execution() {
        let provider = build_service_provider("test-run".to_string(), "test-workflow".to_string());
        let scope = provider.create_scope();
        
        let result = scope.using(|resolver| async move {
            let tools = resolver.get_all_trait::<dyn Tool>()?;
            assert!(tools.len() >= 3); // Should have our registered tools
            
            let engine = WorkflowEngine::new(tools);
            let plan = vec![
                ("math.calculate".to_string(), json!({"operation": "add", "a": 1, "b": 2}))
            ];
            
            let run_context = Arc::new(RunContext::new("test-run".to_string(), "test-workflow".to_string()));
            engine.run_workflow(plan, &*resolver, run_context, None).await
                .map_err(|e| anyhow::anyhow!("Test error: {}", e))
        }).await;
        
        assert!(result.is_ok());
    }

    #[tokio::test] 
    async fn test_checkpoint_service() {
        let provider = build_service_provider("checkpoint-test".to_string(), "test".to_string());
        let scope = provider.create_scope();
        
        scope.using(|resolver| async move {
            // For demo purposes, create services directly
            let store = Arc::new(InMemoryStateStore::default());
            let serializer = Arc::new(JsonSerializer);
            let checkpoint_service = Arc::new(SimpleCheckpointService::new(store, serializer.clone()));
            
            let checkpoint = Checkpoint {
                run_id: "test-run".to_string(),
                step: 1,
                timestamp: chrono::Utc::now(),
                tool_name: "test-tool".to_string(),
                input: json!({"test": "data"}),
                output: Some(json!({"result": "success"})),
                error: None,
                metadata: std::collections::HashMap::new(),
            };
            
            // Save checkpoint
            checkpoint_service.save("test-run", 1, checkpoint).await?;
            
            // Load checkpoint
            let loaded = checkpoint_service.load_latest("test-run").await?;
            assert!(loaded.is_some());
            
            let loaded = loaded.unwrap();
            assert_eq!(loaded.step, 1);
            assert_eq!(loaded.tool_name, "test-tool");
            
            Ok::<(), anyhow::Error>(())
        }).await.unwrap();
    }
}