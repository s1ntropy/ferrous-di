//! Durable agent library showcasing ferrous-di's workflow capabilities.
//!
//! This module demonstrates:
//! - Trait-based abstractions for tools and state management
//! - Decorator pattern for cross-cutting concerns
//! - Scope-local context for workflow execution
//! - Extension methods for service registration

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use ferrous_di::*;
use parking_lot::{Mutex, RwLock};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

// ============================ Configuration ============================

/// Engine configuration with validation
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct EngineOptions {
    pub max_steps: u32,
    pub tool_timeout_ms: u64,
    pub enable_checkpointing: bool,
    pub enable_tracing: bool,
}

// ============================ Core Workflow Context ============================

/// Per-run context stored in ScopeLocal
#[derive(Clone, Debug)]
pub struct RunContext {
    pub run_id: String,
    pub workflow_name: String,
    pub step: u32,
    pub started_at: Instant,
    pub metadata: HashMap<String, String>,
}

impl RunContext {
    pub fn new(run_id: impl Into<String>, workflow_name: impl Into<String>) -> Self {
        Self {
            run_id: run_id.into(),
            workflow_name: workflow_name.into(),
            step: 0,
            started_at: Instant::now(),
            metadata: HashMap::new(),
        }
    }

    pub fn with_step(mut self, step: u32) -> Self {
        self.step = step;
        self
    }

    pub fn elapsed(&self) -> std::time::Duration {
        self.started_at.elapsed()
    }
}

impl Dispose for RunContext {
    fn dispose(&self) {
        println!("[RunContext] Disposing run_id={}, steps={}, elapsed={:?}", 
            self.run_id, self.step, self.elapsed());
    }
}

// ============================ State Management Traits ============================

/// Persistent key-value store for workflow state
#[async_trait]
pub trait StateStore: Send + Sync {
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>>;
    async fn put(&self, key: &str, val: Vec<u8>) -> Result<()>;
    async fn delete(&self, key: &str) -> Result<()>;
    async fn list_keys(&self, prefix: &str) -> Result<Vec<String>>;
}

/// Checkpoint service for workflow resumption
#[async_trait]
pub trait CheckpointService: Send + Sync {
    async fn save(&self, run_id: &str, step: u32, checkpoint: Checkpoint) -> Result<()>;
    async fn load_latest(&self, run_id: &str) -> Result<Option<Checkpoint>>;
    async fn list_checkpoints(&self, run_id: &str) -> Result<Vec<CheckpointMetadata>>;
}

/// Serialization service for stable state persistence
pub trait SnapshotSerializer: Send + Sync {
    fn serialize_value(&self, value: &Value) -> Result<Vec<u8>>;
    fn deserialize_value(&self, bytes: &[u8]) -> Result<Value>;
    fn serialize_checkpoint(&self, checkpoint: &Checkpoint) -> Result<Vec<u8>>;
    fn deserialize_checkpoint(&self, bytes: &[u8]) -> Result<Checkpoint>;
}

// ============================ Checkpoint Model ============================

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Checkpoint {
    pub run_id: String,
    pub step: u32,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub tool_name: String,
    pub input: Value,
    pub output: Option<Value>,
    pub error: Option<String>,
    pub metadata: HashMap<String, String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CheckpointMetadata {
    pub step: u32,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub tool_name: String,
}

// ============================ State Implementation ============================

/// In-memory state store for demo purposes
#[derive(Default)]
pub struct InMemoryStateStore {
    data: RwLock<HashMap<String, Vec<u8>>>,
}

/// File-based state store for persistent demo
pub struct FileStateStore {
    base_path: std::path::PathBuf,
}

impl FileStateStore {
    pub fn new(base_path: impl Into<std::path::PathBuf>) -> Self {
        let base_path = base_path.into();
        // Ensure directory exists
        std::fs::create_dir_all(&base_path).ok();
        Self { base_path }
    }
    
    fn key_to_path(&self, key: &str) -> std::path::PathBuf {
        // Simple key to filename mapping (in production, would need proper escaping)
        let filename = key.replace(':', "_").replace('/', "_");
        self.base_path.join(format!("{}.checkpoint", filename))
    }
}

#[async_trait]
impl StateStore for InMemoryStateStore {
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>> {
        Ok(self.data.read().get(key).cloned())
    }

    async fn put(&self, key: &str, val: Vec<u8>) -> Result<()> {
        let len = val.len();
        self.data.write().insert(key.to_string(), val);
        println!("[DEBUG] StateStore PUT: {} -> {} bytes", key, len);
        Ok(())
    }

    async fn delete(&self, key: &str) -> Result<()> {
        self.data.write().remove(key);
        Ok(())
    }

    async fn list_keys(&self, prefix: &str) -> Result<Vec<String>> {
        let keys: Vec<String> = self.data.read()
            .keys()
            .filter(|k| k.starts_with(prefix))
            .cloned()
            .collect();
        println!("[DEBUG] StateStore LIST_KEYS: {} -> {:?}", prefix, keys);
        Ok(keys)
    }
}

#[async_trait]
impl StateStore for FileStateStore {
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>> {
        let path = self.key_to_path(key);
        match tokio::fs::read(&path).await {
            Ok(data) => {
                println!("[DEBUG] FileStore GET: {} -> {} bytes", key, data.len());
                Ok(Some(data))
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                println!("[DEBUG] FileStore GET: {} -> not found", key);
                Ok(None)
            },
            Err(e) => Err(anyhow::anyhow!("Failed to read file: {}", e)),
        }
    }

    async fn put(&self, key: &str, val: Vec<u8>) -> Result<()> {
        let len = val.len();
        let path = self.key_to_path(key);
        tokio::fs::write(&path, val).await?;
        println!("[DEBUG] FileStore PUT: {} -> {} bytes", key, len);
        Ok(())
    }

    async fn delete(&self, key: &str) -> Result<()> {
        let path = self.key_to_path(key);
        tokio::fs::remove_file(&path).await.ok(); // Ignore errors if file doesn't exist
        println!("[DEBUG] FileStore DELETE: {}", key);
        Ok(())
    }

    async fn list_keys(&self, prefix: &str) -> Result<Vec<String>> {
        let mut keys = Vec::new();
        
        match tokio::fs::read_dir(&self.base_path).await {
            Ok(mut dir) => {
                while let Some(entry) = dir.next_entry().await? {
                    if let Some(filename) = entry.file_name().to_str() {
                        if filename.ends_with(".checkpoint") {
                            // Convert filename back to key
                            let key = filename
                                .strip_suffix(".checkpoint")
                                .unwrap_or(filename)
                                .replace('_', ":");
                            
                            if key.starts_with(prefix) {
                                keys.push(key);
                            }
                        }
                    }
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                // Directory doesn't exist yet, return empty list
            }
            Err(e) => return Err(anyhow::anyhow!("Failed to read directory: {}", e)),
        }
        
        println!("[DEBUG] FileStore LIST_KEYS: {} -> {:?}", prefix, keys);
        Ok(keys)
    }
}

/// Simple checkpoint service backed by StateStore
pub struct SimpleCheckpointService {
    store: Arc<dyn StateStore>,
    serializer: Arc<dyn SnapshotSerializer>,
    index: Mutex<HashMap<String, u32>>, // track latest step per run
}

impl SimpleCheckpointService {
    pub fn new(store: Arc<dyn StateStore>, serializer: Arc<dyn SnapshotSerializer>) -> Self {
        Self {
            store,
            serializer,
            index: Mutex::new(HashMap::new()),
        }
    }
}

#[async_trait]
impl CheckpointService for SimpleCheckpointService {
    async fn save(&self, run_id: &str, step: u32, checkpoint: Checkpoint) -> Result<()> {
        let key = format!("checkpoint:{}:{:04}", run_id, step);
        let bytes = self.serializer.serialize_checkpoint(&checkpoint)?;
        self.store.put(&key, bytes).await?;
        
        // Update index
        self.index.lock().insert(run_id.to_string(), step);
        println!("[DEBUG] Saved checkpoint: {} -> step {}", key, step);
        Ok(())
    }

    async fn load_latest(&self, run_id: &str) -> Result<Option<Checkpoint>> {
        // First try to get step from index
        let step_from_index = self.index.lock().get(run_id).copied();
        println!("[DEBUG] Index lookup for {}: {:?}", run_id, step_from_index);
        
        let step = match step_from_index {
            Some(s) => s,
            None => {
                // Try to find from store
                let keys = self.store.list_keys(&format!("checkpoint:{}:", run_id)).await?;
                println!("[DEBUG] Keys found in store: {:?}", keys);
                if keys.is_empty() {
                    return Ok(None);
                }
                // Parse step from last key
                keys.iter()
                    .filter_map(|k| k.split(':').nth(2)?.parse::<u32>().ok())
                    .max()
                    .unwrap_or(0)
            }
        };
        
        let key = format!("checkpoint:{}:{:04}", run_id, step);
        println!("[DEBUG] Looking for checkpoint: {}", key);
        match self.store.get(&key).await? {
            Some(bytes) => {
                println!("[DEBUG] Found checkpoint data, deserializing...");
                Ok(Some(self.serializer.deserialize_checkpoint(&bytes)?))
            },
            None => {
                println!("[DEBUG] No checkpoint data found for key: {}", key);
                Ok(None)
            },
        }
    }

    async fn list_checkpoints(&self, run_id: &str) -> Result<Vec<CheckpointMetadata>> {
        let keys = self.store.list_keys(&format!("checkpoint:{}:", run_id)).await?;
        let mut metas = Vec::new();
        
        for key in keys {
            if let Some(bytes) = self.store.get(&key).await? {
                if let Ok(checkpoint) = self.serializer.deserialize_checkpoint(&bytes) {
                    metas.push(CheckpointMetadata {
                        step: checkpoint.step,
                        timestamp: checkpoint.timestamp,
                        tool_name: checkpoint.tool_name,
                    });
                }
            }
        }
        
        metas.sort_by_key(|m| m.step);
        Ok(metas)
    }
}

/// JSON serializer implementation
pub struct JsonSerializer;

impl SnapshotSerializer for JsonSerializer {
    fn serialize_value(&self, value: &Value) -> Result<Vec<u8>> {
        Ok(serde_json::to_vec(value)?)
    }

    fn deserialize_value(&self, bytes: &[u8]) -> Result<Value> {
        Ok(serde_json::from_slice(bytes)?)
    }

    fn serialize_checkpoint(&self, checkpoint: &Checkpoint) -> Result<Vec<u8>> {
        Ok(serde_json::to_vec(checkpoint)?)
    }

    fn deserialize_checkpoint(&self, bytes: &[u8]) -> Result<Checkpoint> {
        Ok(serde_json::from_slice(bytes)?)
    }
}

// ============================ Tool System ============================

/// Base trait for workflow tools
#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str;
    fn schema(&self) -> &'static str;
    
    async fn invoke(&self, input: Value, context: &ToolContext<'_>) -> Result<Value>;
}

/// Context passed to tools during execution
pub struct ToolContext<'a> {
    pub run_context: Arc<RunContext>,
    pub cancellation: Option<Arc<CancellationToken>>,
    _phantom: std::marker::PhantomData<&'a ()>,
}

impl<'a> ToolContext<'a> {
    pub fn new(resolver: &'a dyn ResolverCore) -> Self {
        let run_context = Arc::new(RunContext::new("context", "workflow"));
        
        Self {
            run_context,
            cancellation: Self::try_get_cancellation_token(resolver),
            _phantom: std::marker::PhantomData,
        }
    }

    pub fn with_run_context(resolver: &'a dyn ResolverCore, run_context: Arc<RunContext>) -> Self {
        Self {
            run_context,
            cancellation: Self::try_get_cancellation_token(resolver),
            _phantom: std::marker::PhantomData,
        }
    }

    pub fn new_with_resolver(resolver: &'a ScopedResolver, run_context: Arc<RunContext>) -> Self {
        Self {
            run_context,
            cancellation: Self::try_get_cancellation_token_from_scoped(resolver),
            _phantom: std::marker::PhantomData,
        }
    }

    fn try_get_cancellation_token(resolver: &dyn ResolverCore) -> Option<Arc<CancellationToken>> {
        use std::any::TypeId;
        let key = Key::Type(TypeId::of::<CancellationToken>(), "CancellationToken");
        resolver.resolve_any(&key).ok()
            .and_then(|any| any.downcast::<CancellationToken>().ok())
    }

    fn try_get_cancellation_token_from_scoped(resolver: &ScopedResolver) -> Option<Arc<CancellationToken>> {
        resolver.get::<CancellationToken>().ok()
    }
}

// ============================ Tool Decorators ============================

/// Decorator for tool execution (ferrous-di's ServiceDecorator pattern)
pub trait ToolDecorator: Send + Sync {
    fn decorate<'a>(&self, tool: Arc<dyn Tool>, context: &ToolContext<'a>) -> Arc<dyn Tool>;
}

/// Logging decorator using ferrous-di's observer pattern
pub struct LoggingDecorator {
    enabled: bool,
}

impl LoggingDecorator {
    pub fn new(enabled: bool) -> Self {
        Self { enabled }
    }
}

/// Checkpoint decorator for durability
pub struct CheckpointDecorator;

impl CheckpointDecorator {
    pub async fn checkpoint_before(
        &self,
        tool_name: &str,
        input: &Value,
        context: &ToolContext<'_>,
        resolver: &ScopedResolver,
    ) -> Result<()> {
        println!("[CHECKPOINT] Before {}: step {}", tool_name, context.run_context.step);
        
        // Create checkpoint with input
        let checkpoint = Checkpoint {
            run_id: context.run_context.run_id.clone(),
            step: context.run_context.step,
            timestamp: chrono::Utc::now(),
            tool_name: tool_name.to_string(),
            input: input.clone(),
            output: None,
            error: None,
            metadata: HashMap::new(),
        };
        
        // Save checkpoint
        if let Ok(checkpoint_service) = resolver.get_trait::<dyn CheckpointService>() {
            checkpoint_service.save(&context.run_context.run_id, context.run_context.step, checkpoint).await?;
        }
        
        Ok(())
    }

    pub async fn checkpoint_after(
        &self,
        tool_name: &str,
        input: &Value,
        output: &Result<Value>,
        context: &ToolContext<'_>,
        resolver: &ScopedResolver,
    ) -> Result<()> {
        println!("[CHECKPOINT] After {}: step {} - {:?}", tool_name, context.run_context.step, output.is_ok());
        
        // Create checkpoint with output or error
        let checkpoint = Checkpoint {
            run_id: context.run_context.run_id.clone(),
            step: context.run_context.step,
            timestamp: chrono::Utc::now(),
            tool_name: tool_name.to_string(),
            input: input.clone(),
            output: output.as_ref().ok().cloned(),
            error: output.as_ref().err().map(|e| e.to_string()),
            metadata: HashMap::new(),
        };
        
        // Save checkpoint with incremented step for next execution
        if let Ok(checkpoint_service) = resolver.get_trait::<dyn CheckpointService>() {
            checkpoint_service.save(&context.run_context.run_id, context.run_context.step + 1, checkpoint).await?;
        }
        
        Ok(())
    }
}

// ============================ Concrete Tools ============================

/// File reading tool
pub struct ReadFileTool;

#[async_trait]
impl Tool for ReadFileTool {
    fn name(&self) -> &'static str { "fs.read" }
    fn description(&self) -> &'static str { "Read a file from the filesystem" }
    fn schema(&self) -> &'static str {
        r#"{"type":"object","properties":{"path":{"type":"string"}},"required":["path"]}"#
    }

    async fn invoke(&self, input: Value, context: &ToolContext<'_>) -> Result<Value> {
        // Check cancellation
        if let Some(token) = &context.cancellation {
            if token.is_cancelled() {
                return Err(anyhow!("Operation cancelled"));
            }
        }

        let path = input["path"]
            .as_str()
            .ok_or_else(|| anyhow!("Missing 'path' parameter"))?;

        // For demo purposes, just use the run context for tracing
        println!("[{}] Reading file: {}", context.run_context.run_id, path);

        let content = tokio::fs::read_to_string(path)
            .await
            .unwrap_or_else(|e| format!("<error: {}>", e));

        Ok(serde_json::json!({
            "type": "fs.read",
            "path": path,
            "size": content.len(),
            "preview": content.chars().take(100).collect::<String>(),
        }))
    }
}

/// Math calculation tool
pub struct CalculatorTool;

#[async_trait]
impl Tool for CalculatorTool {
    fn name(&self) -> &'static str { "math.calculate" }
    fn description(&self) -> &'static str { "Perform mathematical calculations" }
    fn schema(&self) -> &'static str {
        r#"{"type":"object","properties":{"operation":{"type":"string","enum":["add","subtract","multiply","divide"]},"a":{"type":"number"},"b":{"type":"number"}},"required":["operation","a","b"]}"#
    }

    async fn invoke(&self, input: Value, _context: &ToolContext<'_>) -> Result<Value> {
        let op = input["operation"].as_str().unwrap_or("add");
        let a = input["a"].as_f64().unwrap_or(0.0);
        let b = input["b"].as_f64().unwrap_or(0.0);

        let result = match op {
            "add" => a + b,
            "subtract" => a - b,
            "multiply" => a * b,
            "divide" if b != 0.0 => a / b,
            "divide" => return Err(anyhow!("Division by zero")),
            _ => return Err(anyhow!("Unknown operation: {}", op)),
        };

        Ok(serde_json::json!({
            "type": "math.calculate",
            "operation": op,
            "a": a,
            "b": b,
            "result": result,
        }))
    }
}

// ============================ Cancellation Support ============================

/// Cancellation token for workflow execution
#[derive(Clone)]
pub struct CancellationToken {
    inner: Arc<Mutex<bool>>,
}

impl CancellationToken {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(false)),
        }
    }

    pub fn cancel(&self) {
        *self.inner.lock() = true;
    }

    pub fn is_cancelled(&self) -> bool {
        *self.inner.lock()
    }
}

impl Default for CancellationToken {
    fn default() -> Self {
        Self::new()
    }
}

/// HTTP tool for making web requests
pub struct HttpTool;

#[async_trait]
impl Tool for HttpTool {
    fn name(&self) -> &'static str { "http.get" }
    fn description(&self) -> &'static str { "Make HTTP GET requests" }
    fn schema(&self) -> &'static str {
        r#"{"type":"object","properties":{"url":{"type":"string"}},"required":["url"]}"#
    }

    async fn invoke(&self, input: Value, context: &ToolContext<'_>) -> Result<Value> {
        // Check cancellation
        if let Some(token) = &context.cancellation {
            if token.is_cancelled() {
                return Err(anyhow!("Operation cancelled"));
            }
        }

        let url = input["url"]
            .as_str()
            .ok_or_else(|| anyhow!("Missing 'url' parameter"))?;

        // For demo purposes, just use the run context for tracing
        println!("[{}] Making HTTP request: {}", context.run_context.run_id, url);

        // For demo purposes, simulate HTTP request
        // In a real implementation, you'd use reqwest or similar
        let response = serde_json::json!({
            "type": "http.get",
            "url": url,
            "status": 200,
            "headers": {
                "content-type": "application/json",
                "server": "httpbin.org"
            },
            "body": {
                "message": "This is a simulated HTTP response",
                "timestamp": chrono::Utc::now().to_rfc3339(),
                "request_id": uuid::Uuid::new_v4().to_string()
            }
        });

        Ok(response)
    }
}

// ============================ Observation Context ============================

/// Context for correlating workflow execution with DI events
#[derive(Clone, Debug)]
pub struct ObservationContext {
    pub run_id: String,
    pub workflow_name: String,
    pub step: u32,
    pub correlation_id: String,
}

impl ObservationContext {
    pub fn new(run_id: impl Into<String>, workflow_name: impl Into<String>, step: u32) -> Self {
        let run_id = run_id.into();
        let workflow_name = workflow_name.into();
        let correlation_id = format!("{}:{}:{}", run_id, workflow_name, step);
        
        Self {
            run_id,
            workflow_name,
            step,
            correlation_id,
        }
    }

    pub fn correlation_id(&self) -> &str {
        &self.correlation_id
    }
}

impl Default for ObservationContext {
    fn default() -> Self {
        Self {
            run_id: "unknown".to_string(),
            workflow_name: "unknown".to_string(),
            step: 0,
            correlation_id: "unknown:unknown:0".to_string(),
        }
    }
}

// ============================ Workflow Engine ============================

/// Main workflow executor
pub struct WorkflowEngine {
    tools: HashMap<String, Arc<dyn Tool>>,
}

impl WorkflowEngine {
    pub fn new(tools: Vec<Arc<dyn Tool>>) -> Self {
        let tools = tools.into_iter()
            .map(|t| (t.name().to_string(), t))
            .collect();
        Self { tools }
    }

    pub async fn execute_step(
        &self,
        tool_name: &str,
        input: Value,
        resolver: &ScopedResolver,
        run_context: Arc<RunContext>,
    ) -> Result<Value> {
        let tool = self.tools
            .get(tool_name)
            .ok_or_else(|| anyhow!("Unknown tool: {}", tool_name))?;

        let context = ToolContext::new_with_resolver(resolver, run_context);
        
        // Get decorators
        let checkpoint_decorator = CheckpointDecorator;
        
        // For simplicity, use defaults for now
        let logging_decorator = LoggingDecorator::new(true);

        // Apply decorators (manual for now, could use ferrous-di's decoration pipeline)
        if logging_decorator.enabled {
            println!("[TRACE] Executing {} with input: {}", tool_name, input);
        }

        // Checkpoint before
        checkpoint_decorator.checkpoint_before(tool_name, &input, &context, resolver).await?;

        // Execute tool
        let result = tool.invoke(input.clone(), &context).await;

        // Checkpoint after
        checkpoint_decorator.checkpoint_after(tool_name, &input, &result, &context, resolver).await?;

        if logging_decorator.enabled {
            println!("[TRACE] Result: {:?}", result);
        }

        result
    }

    pub async fn run_workflow(
        &self,
        plan: Vec<(String, Value)>,
        resolver: &ScopedResolver,
        run_context: Arc<RunContext>,
        crash_after_step: Option<u32>,
    ) -> Result<Value> {
        let mut current_step = run_context.step;
        let mut transcript = Vec::new();

        for (i, (tool_name, input)) in plan.iter().enumerate() {
            if (current_step as usize) > i {
                // Skip already completed steps (from checkpoint)
                continue;
            }

            println!("[WORKFLOW] Step {}: {}", current_step, tool_name);
            
            // Create updated context for this step
            let step_context = Arc::new(RunContext {
                run_id: run_context.run_id.clone(),
                workflow_name: run_context.workflow_name.clone(),
                step: current_step,
                started_at: run_context.started_at,
                metadata: run_context.metadata.clone(),
            });
            
            let output = self.execute_step(tool_name, input.clone(), resolver, step_context).await?;
            transcript.push(serde_json::json!({
                "step": current_step,
                "tool": tool_name,
                "output": output,
            }));

            current_step += 1;

            // Simulate crash for testing durability
            if let Some(crash_step) = crash_after_step {
                if current_step > crash_step {
                    return Err(anyhow!("Simulated crash after step {}", crash_step));
                }
            }
        }

        Ok(serde_json::json!({
            "run_id": run_context.run_id,
            "workflow": run_context.workflow_name,
            "completed_steps": current_step,
            "transcript": transcript,
            "elapsed_ms": run_context.elapsed().as_millis(),
        }))
    }
}

// ============================ Service Collection Extensions ============================

/// Extension trait for registering durable agent services
pub trait DurableAgentServiceCollectionExt {
    fn add_durable_agent_core(&mut self) -> &mut Self;
    fn add_state_services(&mut self) -> &mut Self;
    fn add_workflow_tools(&mut self) -> &mut Self;
    fn add_workflow_context(&mut self, run_id: String, workflow_name: String) -> &mut Self;
}

impl DurableAgentServiceCollectionExt for ServiceCollection {
    fn add_durable_agent_core(&mut self) -> &mut Self {
        // Register core options
        self.add_options::<EngineOptions>()
            .default_with(|| EngineOptions {
                max_steps: 10,
                tool_timeout_ms: 5000,
                enable_checkpointing: true,
                enable_tracing: true,
            })
            .validate(|opts| {
                if opts.max_steps == 0 {
                    Err("max_steps must be > 0".into())
                } else if opts.tool_timeout_ms == 0 {
                    Err("tool_timeout_ms must be > 0".into())
                } else {
                    Ok(())
                }
            })
            .register();

        // Note: Would add workflow observer here in full implementation
        
        self
    }

    fn add_state_services(&mut self) -> &mut Self {
        // State store
        self.add_singleton_trait::<dyn StateStore>(
            Arc::new(InMemoryStateStore::default())
        );

        // Serializer
        self.add_singleton_trait::<dyn SnapshotSerializer>(
            Arc::new(JsonSerializer)
        );

        // Checkpoint service (depends on state store and serializer)
        self.add_singleton_trait_factory::<dyn CheckpointService, _>(|resolver| {
            let store = resolver.get_required_trait::<dyn StateStore>();
            let serializer = resolver.get_required_trait::<dyn SnapshotSerializer>();
            Arc::new(SimpleCheckpointService::new(store, serializer))
        });

        self
    }

    fn add_workflow_tools(&mut self) -> &mut Self {
        // Register tools as multi-bindings
        self.add_trait_implementation(Arc::new(ReadFileTool) as Arc<dyn Tool>, Lifetime::Singleton);
        self.add_trait_implementation(Arc::new(CalculatorTool) as Arc<dyn Tool>, Lifetime::Singleton);
        self.add_trait_implementation(Arc::new(HttpTool) as Arc<dyn Tool>, Lifetime::Singleton);
        
        self
    }

    fn add_workflow_context(&mut self, run_id: String, workflow_name: String) -> &mut Self {
        // Add workflow context as ScopeLocal
        self.add_scope_local::<RunContext, _>(move |_| {
            Arc::new(RunContext::new(run_id.clone(), workflow_name.clone()))
        });

        // Add cancellation token
        self.add_scoped_factory::<CancellationToken, _>(|_| CancellationToken::new());

        self
    }
}

// ============================ Rehydration Support ============================

/// Rehydrate workflow state from checkpoints
pub async fn rehydrate_workflow(
    run_id: &str,
    resolver: &ScopedResolver,
) -> Result<Option<RunContext>> {
    // Use the resolver's get_trait method instead of resolve_any
    let checkpoint_service = resolver.get_trait::<dyn CheckpointService>()?;
    
    if let Some(checkpoint) = checkpoint_service.load_latest(run_id).await? {
        println!("[REHYDRATE] Found checkpoint at step {}", checkpoint.step);
        Ok(Some(
            RunContext::new(checkpoint.run_id, "rehydrated_workflow")
                .with_step(checkpoint.step)
        ))
    } else {
        println!("[REHYDRATE] No checkpoint found for run_id={}", run_id);
        Ok(None)
    }
}

// ============================ Graph Export Integration ============================

/// Simple graph builder for workflow dependencies
pub struct GraphBuilder;

impl GraphBuilder {
    pub fn new() -> Self {
        Self
    }

    pub fn build_graph(&self, _provider: &ServiceProvider) -> Result<Graph> {
        // For demo purposes, create a simple graph
        // In a real implementation, this would analyze the DI container
        Ok(Graph {
            nodes: vec![
                GraphNode {
                    id: "fs.read".to_string(),
                    type_name: "ReadFileTool".to_string(),
                    lifetime: "Singleton".to_string(),
                    is_trait: true,
                    dependencies: vec![],
                    metadata: HashMap::new(),
                    position: None,
                },
                GraphNode {
                    id: "math.calculate".to_string(),
                    type_name: "CalculatorTool".to_string(),
                    lifetime: "Singleton".to_string(),
                    is_trait: true,
                    dependencies: vec![],
                    metadata: HashMap::new(),
                    position: None,
                },
                GraphNode {
                    id: "http.get".to_string(),
                    type_name: "HttpTool".to_string(),
                    lifetime: "Singleton".to_string(),
                    is_trait: true,
                    dependencies: vec![],
                    metadata: HashMap::new(),
                    position: None,
                },
            ],
            edges: vec![
                GraphEdge {
                    from: "workflow".to_string(),
                    to: "fs.read".to_string(),
                    dependency_type: DependencyType::Required,
                    metadata: HashMap::new(),
                },
                GraphEdge {
                    from: "workflow".to_string(),
                    to: "math.calculate".to_string(),
                    dependency_type: DependencyType::Required,
                    metadata: HashMap::new(),
                },
                GraphEdge {
                    from: "workflow".to_string(),
                    to: "http.get".to_string(),
                    dependency_type: DependencyType::Required,
                    metadata: HashMap::new(),
                },
            ],
            metadata: GraphMetadata {
                title: "Durable Agent Workflow".to_string(),
                description: "Workflow tools and dependencies".to_string(),
                version: "1.0.0".to_string(),
                created_at: chrono::Utc::now(),
            },
            layout: None,
        })
    }
}

/// Simple graph structure for demo purposes
#[derive(serde::Serialize)]
pub struct Graph {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
    pub metadata: GraphMetadata,
    pub layout: Option<GraphLayout>,
}

/// Graph node representing a service
#[derive(serde::Serialize)]
pub struct GraphNode {
    pub id: String,
    pub type_name: String,
    pub lifetime: String,
    pub is_trait: bool,
    pub dependencies: Vec<String>,
    pub metadata: HashMap<String, String>,
    pub position: Option<NodePosition>,
}

/// Graph edge representing a dependency
#[derive(serde::Serialize)]
pub struct GraphEdge {
    pub from: String,
    pub to: String,
    pub dependency_type: DependencyType,
    pub metadata: HashMap<String, String>,
}

/// Dependency type
#[derive(serde::Serialize)]
pub enum DependencyType {
    Required,
    Optional,
    Multiple,
    Trait,
    Factory,
    Scoped,
    Decorated,
}

/// Graph metadata
#[derive(serde::Serialize)]
pub struct GraphMetadata {
    pub title: String,
    pub description: String,
    pub version: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// Graph layout information
#[derive(serde::Serialize)]
pub struct GraphLayout {
    pub width: f64,
    pub height: f64,
    pub direction: String,
}

/// Node position
#[derive(serde::Serialize)]
pub struct NodePosition {
    pub x: f64,
    pub y: f64,
    pub z: Option<f64>,
}

/// Graph exporter trait
pub trait GraphExporter {
    fn export(&self, graph: &Graph, format: ExportFormat, options: &ExportOptions) -> Result<String>;
}

/// Export format
pub enum ExportFormat {
    Dot,
    Mermaid,
    Json,
}

/// Export options
pub struct ExportOptions {
    pub include_metadata: bool,
    pub include_positions: bool,
}

impl Default for ExportOptions {
    fn default() -> Self {
        Self {
            include_metadata: true,
            include_positions: false,
        }
    }
}

/// Default graph exporter
pub struct DefaultGraphExporter;

impl GraphExporter for DefaultGraphExporter {
    fn export(&self, graph: &Graph, format: ExportFormat, _options: &ExportOptions) -> Result<String> {
        match format {
            ExportFormat::Mermaid => {
                let mut mermaid = String::new();
                mermaid.push_str("graph TD\n");
                
                // Add nodes
                for node in &graph.nodes {
                    mermaid.push_str(&format!("    {}[{}]\n", node.id, node.type_name));
                }
                
                // Add edges
                for edge in &graph.edges {
                    mermaid.push_str(&format!("    {} --> {}\n", edge.from, edge.to));
                }
                
                Ok(mermaid)
            }
            ExportFormat::Dot => {
                let mut dot = String::new();
                dot.push_str("digraph G {\n");
                
                // Add nodes
                for node in &graph.nodes {
                    dot.push_str(&format!("    \"{}\" [label=\"{}\"];\n", node.id, node.type_name));
                }
                
                // Add edges
                for edge in &graph.edges {
                    dot.push_str(&format!("    \"{}\" -> \"{}\";\n", edge.from, edge.to));
                }
                
                dot.push_str("}\n");
                Ok(dot)
            }
            ExportFormat::Json => {
                Ok(serde_json::to_string_pretty(graph).unwrap_or_else(|_| "{}".to_string()))
            }
        }
    }
}

/// Export workflow dependencies as a graph (simplified demo)
pub fn export_workflow_graph(provider: &ServiceProvider) -> Result<String> {
    // In a real implementation, would use ferrous-di's graph export features
    let graph = GraphBuilder::new()
        .build_graph(provider)
        .map_err(|e| anyhow::anyhow!("Graph build error: {}", e))?;

    // Export as Mermaid format
    let exporter = DefaultGraphExporter;
    exporter.export(&graph, ExportFormat::Mermaid, &ExportOptions::default())
        .map_err(|e| anyhow::anyhow!("Graph export error: {}", e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_in_memory_state_store() {
        let store = InMemoryStateStore::default();
        
        // Test put and get
        store.put("key1", b"value1".to_vec()).await.unwrap();
        let value = store.get("key1").await.unwrap();
        assert_eq!(value, Some(b"value1".to_vec()));
        
        // Test delete
        store.delete("key1").await.unwrap();
        let value = store.get("key1").await.unwrap();
        assert_eq!(value, None);
        
        // Test list_keys
        store.put("prefix:a", vec![1]).await.unwrap();
        store.put("prefix:b", vec![2]).await.unwrap();
        store.put("other:c", vec![3]).await.unwrap();
        
        let keys = store.list_keys("prefix:").await.unwrap();
        assert_eq!(keys.len(), 2);
        assert!(keys.contains(&"prefix:a".to_string()));
        assert!(keys.contains(&"prefix:b".to_string()));
    }

    #[test]
    fn test_run_context() {
        let ctx = RunContext::new("run-123", "test-workflow");
        assert_eq!(ctx.run_id, "run-123");
        assert_eq!(ctx.workflow_name, "test-workflow");
        assert_eq!(ctx.step, 0);
        
        let ctx = ctx.with_step(5);
        assert_eq!(ctx.step, 5);
    }
}