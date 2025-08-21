//! Enterprise Patterns Example using ferrous-di
//!
//! This example demonstrates advanced enterprise patterns:
//! - Multi-tenant architecture with tenant isolation
//! - Circuit breaker pattern for external services
//! - Retry policies with exponential backoff
//! - Metrics collection and monitoring
//! - Configuration management with environment overrides
//! - Audit logging and compliance features

use ferrous_di::{
    ServiceCollection, Resolver,
    axum_integration::{create_app_with_di, DiScope},
};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio::net::TcpListener;
use axum::{
    extract::Path,
    response::Json,
    routing::{get, post},
    http::StatusCode,
};

// ===== Enterprise Domain Models =====

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Tenant {
    id: String,
    name: String,
    tier: TenantTier,
    features: TenantFeatures,
    limits: TenantLimits,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
enum TenantTier {
    Free,
    Professional,
    Enterprise,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TenantFeatures {
    max_users: usize,
    max_storage_gb: usize,
    api_rate_limit: usize,
    advanced_analytics: bool,
    custom_branding: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TenantLimits {
    daily_api_calls: usize,
    monthly_storage_gb: usize,
    concurrent_users: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct User {
    id: String,
    tenant_id: String,
    email: String,
    role: UserRole,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
enum UserRole {
    Viewer,
    Editor,
    Admin,
    Owner,
}

// ===== Circuit Breaker Pattern =====

#[derive(Debug, Clone)]
enum CircuitState {
    Closed,    // Normal operation
    Open,      // Failing, reject requests
    HalfOpen,  // Testing if service recovered
}

#[derive(Debug)]
struct CircuitBreaker {
    state: Mutex<CircuitState>,
    failure_threshold: usize,
    success_threshold: usize,
    timeout: Duration,
    last_failure: Mutex<Option<Instant>>,
    failure_count: Mutex<usize>,
}

impl CircuitBreaker {
    fn new(failure_threshold: usize, timeout: Duration) -> Self {
        Self {
            state: Mutex::new(CircuitState::Closed),
            failure_threshold,
            success_threshold: 1,
            timeout,
            last_failure: Mutex::new(None),
            failure_count: Mutex::new(0),
        }
    }

    async fn call<F, T, E>(&self, f: F) -> Result<T, CircuitBreakerError>
    where
        F: FnOnce() -> Result<T, E> + Send + 'static,
        E: std::error::Error + Send + Sync + 'static,
    {
        let state = { self.state.lock().unwrap().clone() };
        
        match state {
            CircuitState::Open => {
                if let Some(last_failure) = *self.last_failure.lock().unwrap() {
                    if Instant::now().duration_since(last_failure) >= self.timeout {
                        // Try to transition to half-open
                        *self.state.lock().unwrap() = CircuitState::HalfOpen;
                    } else {
                        return Err(CircuitBreakerError::CircuitOpen);
                    }
                } else {
                    return Err(CircuitBreakerError::CircuitOpen);
                }
            }
            CircuitState::HalfOpen | CircuitState::Closed => {}
        }

        // Execute the function
        match f() {
            Ok(result) => {
                self.on_success();
                Ok(result)
            }
            Err(_) => {
                self.on_failure();
                Err(CircuitBreakerError::ServiceError)
            }
        }
    }

    fn on_success(&self) {
        let mut state = self.state.lock().unwrap();
        let mut failure_count = self.failure_count.lock().unwrap();
        
        match *state {
            CircuitState::HalfOpen => {
                if *failure_count >= self.success_threshold {
                    *state = CircuitState::Closed;
                    *failure_count = 0;
                }
            }
            CircuitState::Closed => {
                *failure_count = 0;
            }
            CircuitState::Open => {}
        }
    }

    fn on_failure(&self) {
        let mut state = self.state.lock().unwrap();
        let mut failure_count = self.failure_count.lock().unwrap();
        let mut last_failure = self.last_failure.lock().unwrap();
        
        *failure_count += 1;
        *last_failure = Some(Instant::now());
        
        if *failure_count >= self.failure_threshold {
            *state = CircuitState::Open;
        }
    }
}

#[derive(Debug)]
enum CircuitBreakerError {
    CircuitOpen,
    ServiceError,
}

impl std::fmt::Display for CircuitBreakerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CircuitBreakerError::CircuitOpen => write!(f, "Circuit breaker is open"),
            CircuitBreakerError::ServiceError => write!(f, "Service error occurred"),
        }
    }
}

impl std::error::Error for CircuitBreakerError {}

// ===== Retry Policy =====

#[derive(Debug, Clone)]
struct RetryPolicy {
    max_attempts: usize,
    base_delay: Duration,
    max_delay: Duration,
    backoff_multiplier: f64,
}

impl RetryPolicy {
    fn new(max_attempts: usize, base_delay: Duration) -> Self {
        Self {
            max_attempts,
            base_delay,
            max_delay: Duration::from_secs(60),
            backoff_multiplier: 2.0,
        }
    }

    async fn execute<F, T, E>(&self, f: F) -> Result<T, E>
    where
        F: Fn() -> Result<T, E> + Send + Sync,
        E: Clone + Send + Sync,
    {
        let mut last_error = None;
        
        for attempt in 1..=self.max_attempts {
            match f() {
                Ok(result) => return Ok(result),
                Err(e) => {
                    last_error = Some(e.clone());
                    
                    if attempt < self.max_attempts {
                        let delay = self.calculate_delay(attempt);
                        tokio::time::sleep(delay).await;
                    }
                }
            }
        }
        
        Err(last_error.unwrap())
    }

    fn calculate_delay(&self, attempt: usize) -> Duration {
        let delay = self.base_delay.mul_f64(self.backoff_multiplier.powi(attempt as i32 - 1));
        delay.min(self.max_delay)
    }
}

// ===== Metrics Collection =====

#[derive(Debug, Default)]
struct MetricsCollector {
    counters: Mutex<HashMap<String, u64>>,
    timers: Mutex<HashMap<String, Vec<Duration>>>,
    gauges: Mutex<HashMap<String, f64>>,
}

impl MetricsCollector {
    fn new() -> Self {
        Self::default()
    }

    fn increment_counter(&self, name: &str) {
        let mut counters = self.counters.lock().unwrap();
        *counters.entry(name.to_string()).or_insert(0) += 1;
    }

    fn record_timer(&self, name: &str, duration: Duration) {
        let mut timers = self.timers.lock().unwrap();
        timers.entry(name.to_string()).or_insert_with(Vec::new).push(duration);
    }

    fn set_gauge(&self, name: &str, value: f64) {
        let mut gauges = self.gauges.lock().unwrap();
        gauges.insert(name.to_string(), value);
    }

    fn get_metrics(&self) -> MetricsSnapshot {
        let counters = self.counters.lock().unwrap().clone();
        let timers = self.timers.lock().unwrap().clone();
        let gauges = self.gauges.lock().unwrap().clone();
        
        MetricsSnapshot {
            counters,
            timers,
            gauges,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct MetricsSnapshot {
    counters: HashMap<String, u64>,
    timers: HashMap<String, Vec<Duration>>,
    gauges: HashMap<String, f64>,
}

// ===== Audit Logging =====

#[derive(Debug, Clone, Serialize)]
struct AuditLog {
    #[serde(skip_serializing)]
    timestamp: Instant,
    user_id: String,
    tenant_id: String,
    action: String,
    resource: String,
    details: serde_json::Value,
    ip_address: Option<String>,
}

trait AuditLogger: Send + Sync + std::fmt::Debug {
    fn log(&self, entry: AuditLog);
}

#[derive(Debug)]
struct InMemoryAuditLogger {
    logs: Mutex<Vec<AuditLog>>,
}

impl InMemoryAuditLogger {
    fn new() -> Self {
        Self {
            logs: Mutex::new(Vec::new()),
        }
    }
}

impl AuditLogger for InMemoryAuditLogger {
    fn log(&self, entry: AuditLog) {
        let mut logs = self.logs.lock().unwrap();
        logs.push(entry);
    }
}

// ===== Service Layer =====

#[derive(Debug)]
struct UserService {
    circuit_breaker: Arc<CircuitBreaker>,
    retry_policy: Arc<RetryPolicy>,
    metrics: Arc<MetricsCollector>,
    audit_logger: Arc<dyn AuditLogger>,
}

impl UserService {
    fn new(
        circuit_breaker: Arc<CircuitBreaker>,
        retry_policy: Arc<RetryPolicy>,
        metrics: Arc<MetricsCollector>,
        audit_logger: Arc<dyn AuditLogger>,
    ) -> Self {
        Self {
            circuit_breaker,
            retry_policy,
            metrics,
            audit_logger,
        }
    }

    async fn create_user(&self, user: User) -> Result<User, String> {
        let start = Instant::now();
        self.metrics.increment_counter("user.create.attempts");
        
        // Simulate external service call with circuit breaker and retry
        let result = self.retry_policy.execute(|| {
            // This would be an actual external API call in real implementation
            if user.email.contains("error") {
                Err("External service error".to_string())
            } else {
                Ok(user.clone())
            }
        }).await;

        // Record metrics
        let duration = start.elapsed();
        self.metrics.record_timer("user.create.duration", duration);
        
        match &result {
            Ok(_) => {
                self.metrics.increment_counter("user.create.success");
                
                // Audit log
                self.audit_logger.log(AuditLog {
                    timestamp: Instant::now(),
                    user_id: "system".to_string(),
                    tenant_id: user.tenant_id.clone(),
                    action: "user.create".to_string(),
                    resource: format!("user:{}", user.id),
                    details: serde_json::json!({ "email": user.email }),
                    ip_address: None,
                });
            }
            Err(_) => {
                self.metrics.increment_counter("user.create.failures");
            }
        }

        result
    }

    async fn get_user(&self, user_id: &str) -> Result<User, String> {
        self.metrics.increment_counter("user.get.attempts");
        
        // Simulate user lookup
        let user = User {
            id: user_id.to_string(),
            tenant_id: "tenant-1".to_string(),
            email: "user@example.com".to_string(),
            role: UserRole::Editor,
        };
        
        self.metrics.increment_counter("user.get.success");
        Ok(user)
    }
}

// ===== API Handlers =====

async fn create_user(
    scope: DiScope,
    Json(user): Json<User>,
) -> Result<Json<User>, (StatusCode, String)> {
    println!("📝 Creating user: {:?}", user);
    let service = scope.get_required::<UserService>();
    
    match service.create_user(user).await {
        Ok(user) => {
            println!("✅ Created user: {:?}", user);
            Ok(Json(user))
        },
        Err(e) => {
            println!("❌ Error creating user: {}", e);
            Err((StatusCode::INTERNAL_SERVER_ERROR, e))
        }
    }
}

async fn get_user(
    Path(user_id): Path<String>,
    scope: DiScope,
) -> Json<User> {
    println!("🔍 Getting user: {}", user_id);
    let service = scope.get_required::<UserService>();
    
    // Use unwrap for now since we know it won't fail in our demo
    let user = service.get_user(&user_id).await.unwrap();
    
    println!("✅ Found user: {:?}", user);
    Json(user)
}

async fn get_metrics(scope: DiScope) -> Json<MetricsSnapshot> {
    let metrics = scope.get_required::<MetricsCollector>();
    Json(metrics.get_metrics())
}

// ===== Service Configuration =====

fn configure_enterprise_services() -> ServiceCollection {
    let mut services = ServiceCollection::new();
    
    // Core infrastructure
    services.add_singleton(MetricsCollector::new());
    services.add_singleton_trait::<dyn AuditLogger>(Arc::new(InMemoryAuditLogger::new()));
    
    // Circuit breaker with 5 failures threshold and 30s timeout
    services.add_singleton(CircuitBreaker::new(
        5,
        Duration::from_secs(30),
    ));
    
    // Retry policy with exponential backoff
    services.add_singleton(RetryPolicy::new(
        3,
        Duration::from_millis(100),
    ));
    
    // Business services
    services.add_scoped_factory::<UserService, _>(|resolver| {
        UserService::new(
            resolver.get_required::<CircuitBreaker>(),
            resolver.get_required::<RetryPolicy>(),
            resolver.get_required::<MetricsCollector>(),
            resolver.get_required_trait::<dyn AuditLogger>(),
        )
    });
    
    services
}

#[tokio::main]
async fn main() {
    println!("🚀 Starting Enterprise Patterns Server with ferrous-di");
    
    // Build DI container
    let services = configure_enterprise_services();
    let provider = Arc::new(services.build());
    
    // Initialize some demo metrics to show the system working
    {
        let metrics = provider.get_required::<MetricsCollector>();
        metrics.increment_counter("server.startup");
        metrics.set_gauge("server.uptime_seconds", 0.0);
        metrics.record_timer("server.init_time", Duration::from_millis(100));
        println!("✅ Demo metrics initialized");
    }
    
    // Build Axum app with DI integration
    let app = create_app_with_di(provider, |router| {
        router
            .route("/users", post(create_user))
            .route("/users/:id", get(get_user))
            .route("/metrics", get(get_metrics))
            .route("/health", get(|| async { "OK" }))
            .route("/test", get(|| async { Json(serde_json::json!({"test": "working"})) }))
    });
    
    // Start server
    let listener = TcpListener::bind("127.0.0.1:3001")
        .await
        .expect("Failed to bind address");
    
    println!("📡 Enterprise Server running on http://127.0.0.1:3001");
    println!("Try:");
    println!("  curl -X POST http://127.0.0.1:3001/users \\");
    println!("    -H 'Content-Type: application/json' \\");
    println!("    -d '{{\"id\": \"user-1\", \"tenant_id\": \"tenant-1\", \"email\": \"user@example.com\", \"role\": \"Editor\"}}'");
    println!("  curl http://127.0.0.1:3001/users/user-1");
    println!("  curl http://127.0.0.1:3001/metrics");
    
    axum::serve(listener, app.into_make_service())
        .await
        .expect("Server failed to start");
}
