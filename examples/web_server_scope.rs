use ferrous_di::{ServiceCollection, Resolver};
use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use std::net::{TcpListener, TcpStream};
use std::io::{Read, Write};
use std::thread;

// ===== Domain Types =====

#[derive(Debug, Clone)]
struct User {
    id: String,
    name: String,
}

#[derive(Debug)]
struct RequestContext {
    request_id: String,
    user: Option<User>,
    path: String,
}

// ===== Services =====

trait UserRepository: Send + Sync {
    fn find_by_id(&self, id: &str) -> Option<User>;
    fn find_all(&self) -> Vec<User>;
}

struct InMemoryUserRepository {
    users: HashMap<String, User>,
}

impl InMemoryUserRepository {
    fn new() -> Self {
        let mut users = HashMap::new();
        users.insert("1".to_string(), User { id: "1".to_string(), name: "Alice".to_string() });
        users.insert("2".to_string(), User { id: "2".to_string(), name: "Bob".to_string() });
        users.insert("3".to_string(), User { id: "3".to_string(), name: "Charlie".to_string() });
        
        Self { users }
    }
}

impl UserRepository for InMemoryUserRepository {
    fn find_by_id(&self, id: &str) -> Option<User> {
        self.users.get(id).cloned()
    }
    
    fn find_all(&self) -> Vec<User> {
        self.users.values().cloned().collect()
    }
}

trait RequestHandler: Send + Sync {
    fn handle(&self, path: &str, query: &str) -> String;
}

struct UserHandler {
    context: Arc<RequestContext>,
    repository: Arc<dyn UserRepository>,
}

impl RequestHandler for UserHandler {
    fn handle(&self, _path: &str, query: &str) -> String {
        let response = if query.starts_with("id=") {
            let id = &query[3..];
            match self.repository.find_by_id(id) {
                Some(user) => format!("User: {} ({})", user.name, user.id),
                None => "User not found".to_string(),
            }
        } else {
            let users = self.repository.find_all();
            let user_list: Vec<String> = users.iter()
                .map(|u| format!("{} ({})", u.name, u.id))
                .collect();
            format!("All users: [{}]", user_list.join(", "))
        };
        
        format!(
            "Request ID: {}\nPath: {}\nResponse: {}",
            self.context.request_id,
            self.context.path,
            response
        )
    }
}

trait Logger: Send + Sync {
    fn log(&self, message: &str);
}

struct ConsoleLogger;
impl Logger for ConsoleLogger {
    fn log(&self, message: &str) {
        println!("[LOG] {}", message);
    }
}

struct RequestLogger {
    context: Arc<RequestContext>,
    logger: Arc<dyn Logger>,
}

impl Logger for RequestLogger {
    fn log(&self, message: &str) {
        self.logger.log(&format!("[{}] {}", self.context.request_id, message));
    }
}

// ===== Application =====

struct WebServer {
    service_provider: ferrous_di::ServiceProvider,
}

impl WebServer {
    fn new(service_provider: ferrous_di::ServiceProvider) -> Self {
        Self { service_provider }
    }
    
    fn handle_request(&self, request: &str) -> String {
        // Create a new scope for this request
        let scope = self.service_provider.create_scope();
        
        // Parse the simple HTTP request
        let lines: Vec<&str> = request.lines().collect();
        let first_line = lines.get(0).unwrap_or(&"");
        let parts: Vec<&str> = first_line.split(' ').collect();
        let path = parts.get(1).unwrap_or(&"/").to_string();
        
        // Extract query string if present
        let (path, query) = if let Some(pos) = path.find('?') {
            (path[..pos].to_string(), path[pos+1..].to_string())
        } else {
            (path, String::new())
        };
        
        // Get scoped services
        let handler = scope.get_required_trait::<dyn RequestHandler>();
        let logger = scope.get_required_trait::<dyn Logger>();
        
        logger.log(&format!("Handling request: {} {}", "GET", path));
        
        let response_body = handler.handle(&path, &query);
        
        logger.log("Request completed");
        
        // Create HTTP response
        format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {}\r\n\r\n{}",
            response_body.len(),
            response_body
        )
    }
    
    fn run(&self) -> Result<(), Box<dyn std::error::Error>> {
        let listener = TcpListener::bind("127.0.0.1:8080")?;
        println!("Server running on http://127.0.0.1:8080");
        println!("Try:");
        println!("  curl http://127.0.0.1:8080/users");
        println!("  curl http://127.0.0.1:8080/users?id=1");
        println!("  curl http://127.0.0.1:8080/users?id=2");
        
        for stream in listener.incoming() {
            match stream {
                Ok(stream) => {
                    let server = WebServer::new(self.service_provider.clone());
                    thread::spawn(move || {
                        if let Err(e) = server.handle_connection(stream) {
                            eprintln!("Error handling connection: {}", e);
                        }
                    });
                }
                Err(e) => {
                    eprintln!("Connection failed: {}", e);
                }
            }
        }
        
        Ok(())
    }
    
    fn handle_connection(&self, mut stream: TcpStream) -> Result<(), Box<dyn std::error::Error>> {
        let mut buffer = [0; 1024];
        let bytes_read = stream.read(&mut buffer)?;
        let request = String::from_utf8_lossy(&buffer[..bytes_read]);
        
        let response = self.handle_request(&request);
        stream.write_all(response.as_bytes())?;
        stream.flush()?;
        
        Ok(())
    }
}

// ===== Configuration =====

fn configure_services() -> ferrous_di::ServiceProvider {
    let mut sc = ServiceCollection::new();
    
    // Singleton services (shared across all requests)
    sc.add_singleton_trait(Arc::new(InMemoryUserRepository::new()) as Arc<dyn UserRepository>);
    
    // Scoped services (per-request)
    let request_counter = Arc::new(Mutex::new(0));
    let counter_clone = request_counter.clone();
    
    sc.add_scoped_factory::<RequestContext, _>(move |_| {
        let mut counter = counter_clone.lock().unwrap();
        *counter += 1;
        RequestContext {
            request_id: format!("req-{}", *counter),
            user: None, // Could be populated from auth middleware
            path: String::new(), // Will be set by the handler
        }
    });
    
    // Request-scoped logger that includes request context
    sc.add_scoped_trait_factory::<dyn Logger, _>(|r| {
        Arc::new(RequestLogger {
            context: r.get_required::<RequestContext>(),
            logger: Arc::new(ConsoleLogger) as Arc<dyn Logger>,
        }) as Arc<dyn Logger>
    });
    
    // Request handler with dependencies
    sc.add_scoped_trait_factory::<dyn RequestHandler, _>(|r| {
        Arc::new(UserHandler {
            context: r.get_required::<RequestContext>(),
            repository: r.get_required_trait::<dyn UserRepository>(),
        }) as Arc<dyn RequestHandler>
    });
    
    sc.build()
}

// ===== Main =====

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Setting up Ferrous DI Web Server Example");
    
    // Configure dependency injection
    let service_provider = configure_services();
    
    // Test the DI setup with a few manual requests
    println!("\n=== Testing DI Setup ===");
    
    let scope1 = service_provider.create_scope();
    let scope2 = service_provider.create_scope();
    
    let handler1 = scope1.get_required_trait::<dyn RequestHandler>();
    let handler2 = scope1.get_required_trait::<dyn RequestHandler>(); // Same scope
    let handler3 = scope2.get_required_trait::<dyn RequestHandler>(); // Different scope
    
    // Test scoped behavior
    println!("Testing scoped services:");
    println!("Handler1 response: {}", handler1.handle("/users", ""));
    println!("Handler2 response: {}", handler2.handle("/users", "id=1"));
    println!("Handler3 response: {}", handler3.handle("/users", "id=2"));
    
    // Verify same instance within scope, different across scopes
    println!("\nScope validation:");
    println!("Handler1 and Handler2 same instance: {}", 
        Arc::ptr_eq(&handler1, &handler2)
    );
    println!("Handler1 and Handler3 different instances: {}", 
        !Arc::ptr_eq(&handler1, &handler3)
    );
    
    println!("\n=== Starting Web Server ===");
    
    // Create and run the web server
    let server = WebServer::new(service_provider);
    server.run()
}