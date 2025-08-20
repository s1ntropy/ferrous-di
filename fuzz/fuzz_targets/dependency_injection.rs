#![no_main]

use libfuzzer_sys::fuzz_target;
use ferrous_di::{ServiceCollection, Resolver};
use std::sync::Arc;

fuzz_target!(|data: &[u8]| {
    if data.len() < 12 {
        return;
    }
    
    // Parse input bytes
    let config_value = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
    let service_id = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
    let pattern = u32::from_le_bytes([data[8], data[9], data[10], data[11]]);
    
    let mut services = ServiceCollection::new();
    
    // Register dependencies
    services.add_singleton(Config { value: config_value });
    
    match pattern % 4 {
        0 => {
            // Simple dependency injection
            services.add_singleton_factory::<DatabaseService, _>(|r| {
                let config = r.get_required::<Config>();
                DatabaseService { 
                    config: Arc::clone(&config),
                    connection_id: format!("conn_{}", config.value),
                }
            });
            
            if let Ok(provider) = std::panic::catch_unwind(|| services.build()) {
                let _ = std::panic::catch_unwind(|| {
                    let db = provider.get_required::<DatabaseService>();
                    assert_eq!(db.config.value, config_value);
                    assert_eq!(db.connection_id, format!("conn_{}", config_value));
                });
            }
        },
        1 => {
            // Chain of dependencies
            services.add_singleton_factory::<DatabaseService, _>(|r| {
                let config = r.get_required::<Config>();
                DatabaseService { 
                    config: Arc::clone(&config),
                    connection_id: format!("conn_{}", config.value),
                }
            });
            
            services.add_singleton_factory::<BusinessService, _>(|r| {
                let db = r.get_required::<DatabaseService>();
                BusinessService { 
                    db: Arc::clone(&db),
                    service_id,
                }
            });
            
            if let Ok(provider) = std::panic::catch_unwind(|| services.build()) {
                let _ = std::panic::catch_unwind(|| {
                    let business = provider.get_required::<BusinessService>();
                    assert_eq!(business.service_id, service_id);
                    assert_eq!(business.db.config.value, config_value);
                });
            }
        },
        2 => {
            // Mixed lifetimes with dependencies
            services.add_scoped_factory::<SessionService, _>(|r| {
                let config = r.get_required::<Config>();
                SessionService {
                    session_id: format!("session_{}", config.value),
                    config: Arc::clone(&config),
                }
            });
            
            services.add_transient_factory::<RequestService, _>(|r| {
                let session = r.get_required::<SessionService>();
                RequestService {
                    request_id: service_id,
                    session: Arc::clone(&session),
                }
            });
            
            if let Ok(provider) = std::panic::catch_unwind(|| services.build()) {
                let _ = std::panic::catch_unwind(|| {
                    let scope = provider.create_scope();
                    let request = scope.get_required::<RequestService>();
                    assert_eq!(request.request_id, service_id);
                    assert_eq!(request.session.config.value, config_value);
                });
            }
        },
        3 => {
            // Trait-based dependencies
            services.add_singleton_trait::<dyn Logger>(Arc::new(ConsoleLogger { 
                prefix: format!("LOG_{}", config_value) 
            }));
            
            services.add_singleton_factory::<AppService, _>(|r| {
                let logger = r.get_required_trait::<dyn Logger>();
                AppService {
                    logger: Arc::clone(&logger),
                    app_id: service_id,
                }
            });
            
            if let Ok(provider) = std::panic::catch_unwind(|| services.build()) {
                let _ = std::panic::catch_unwind(|| {
                    let app = provider.get_required::<AppService>();
                    assert_eq!(app.app_id, service_id);
                    let log_result = app.logger.log("test");
                    assert!(log_result.contains(&format!("LOG_{}", config_value)));
                });
            }
        },
        _ => unreachable!(),
    }
});

#[derive(Debug, Clone)]
struct Config {
    value: u32,
}

#[derive(Debug)]
struct DatabaseService {
    config: Arc<Config>,
    connection_id: String,
}

#[derive(Debug)]
struct BusinessService {
    db: Arc<DatabaseService>,
    service_id: u32,
}

#[derive(Debug)]
struct SessionService {
    session_id: String,
    config: Arc<Config>,
}

#[derive(Debug)]
struct RequestService {
    request_id: u32,
    session: Arc<SessionService>,
}

#[derive(Debug)]
struct AppService {
    logger: Arc<dyn Logger>,
    app_id: u32,
}

trait Logger: Send + Sync {
    fn log(&self, message: &str) -> String;
}

#[derive(Debug)]
struct ConsoleLogger {
    prefix: String,
}

impl Logger for ConsoleLogger {
    fn log(&self, message: &str) -> String {
        format!("{}: {}", self.prefix, message)
    }
}