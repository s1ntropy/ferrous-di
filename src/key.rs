//! Service key types for the dependency injection container.

use std::any::TypeId;

/// Key for service storage and lookup.
///
/// Keys uniquely identify services in the container, supporting both
/// unnamed and named service registrations. Each key type serves a
/// specific purpose in the DI system's service resolution mechanism.
///
/// # Key Types
///
/// - **Type**: Concrete types (structs, enums, primitives)
/// - **Trait**: Single trait implementations  
/// - **MultiTrait**: Multiple trait implementations with indexing
/// - **Named variants**: All above with additional string names
///
/// # Examples
///
/// ```rust
/// use ferrous_di::{ServiceCollection, Resolver, Key};
/// use std::sync::Arc;
///
/// // Concrete type keys
/// let mut services = ServiceCollection::new();
/// services.add_singleton(42u32);
/// services.add_named_singleton("config_port", 8080u32);
///
/// // Trait keys  
/// trait Logger: Send + Sync {
///     fn log(&self, msg: &str);
/// }
///
/// struct ConsoleLogger;
/// impl Logger for ConsoleLogger {
///     fn log(&self, msg: &str) {
///         println!("LOG: {}", msg);
///     }
/// }
///
/// services.add_singleton_trait(Arc::new(ConsoleLogger) as Arc<dyn Logger>);
///
/// let provider = services.build();
///
/// // Resolution uses keys internally
/// let number = provider.get_required::<u32>(); // Uses Type key
/// let port = provider.get_named_required::<u32>("config_port"); // Uses TypeNamed key  
/// let logger = provider.get_required_trait::<dyn Logger>(); // Uses Trait key
///
/// assert_eq!(*number, 42);
/// assert_eq!(*port, 8080);
/// logger.log("Service resolution successful");
/// ```
#[derive(Debug, Clone)]
pub enum Key {
    /// Concrete type key with TypeId and name for diagnostics
    ///
    /// Used for registering and resolving concrete types like `String`,
    /// `Database`, custom structs, etc. The TypeId provides fast lookup
    /// while the name helps with debugging.
    Type(TypeId, &'static str),
    /// Single trait binding key
    ///
    /// Used for registering and resolving trait objects like `dyn Logger`.
    /// Only stores the trait name since traits don't have TypeId.
    Trait(&'static str),
    /// Multi-trait binding with index
    ///
    /// Used when multiple implementations are registered for the same trait.
    /// The index distinguishes between different implementations.
    MultiTrait(&'static str, usize),
    
    // Named service variants
    /// Named concrete type key with TypeId, typename, and name
    ///
    /// Like `Type` but with an additional string name for cases where
    /// multiple instances of the same type need different registrations.
    TypeNamed(TypeId, &'static str, &'static str),
    /// Named single trait binding key with trait name and service name
    ///
    /// Like `Trait` but with an additional string name for different
    /// implementations of the same trait.
    TraitNamed(&'static str, &'static str),
    /// Named multi-trait binding with trait name, service name, and index
    ///
    /// Combination of `MultiTrait` and naming for complex scenarios with
    /// multiple named implementations of the same trait.
    MultiTraitNamed(&'static str, &'static str, usize),
}

impl Key {
    /// Get the type or trait name for display
    ///
    /// Returns the human-readable type or trait name for debugging and
    /// error messages. This is the `std::any::type_name` result.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use ferrous_di::Key;
    /// use std::any::TypeId;
    ///
    /// let type_key = Key::Type(TypeId::of::<String>(), "alloc::string::String");
    /// assert_eq!(type_key.display_name(), "alloc::string::String");
    ///
    /// let trait_key = Key::Trait("dyn core::fmt::Debug");
    /// assert_eq!(trait_key.display_name(), "dyn core::fmt::Debug");
    ///
    /// let named_key = Key::TypeNamed(TypeId::of::<u32>(), "u32", "port");
    /// assert_eq!(named_key.display_name(), "u32");
    /// ```
    pub fn display_name(&self) -> &'static str {
        match self {
            Key::Type(_, name) => name,
            Key::Trait(name) => name,
            Key::MultiTrait(name, _) => name,
            Key::TypeNamed(_, name, _) => name,
            Key::TraitNamed(name, _) => name,
            Key::MultiTraitNamed(name, _, _) => name,
        }
    }
    
    /// Get the service name for named services, or None for unnamed services
    ///
    /// Returns the service name for keys that represent named service
    /// registrations, or `None` for unnamed services.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use ferrous_di::Key;
    /// use std::any::TypeId;
    ///
    /// // Unnamed services return None
    /// let unnamed_key = Key::Type(TypeId::of::<String>(), "alloc::string::String");
    /// assert_eq!(unnamed_key.service_name(), None);
    ///
    /// let trait_key = Key::Trait("dyn core::fmt::Debug");
    /// assert_eq!(trait_key.service_name(), None);
    ///
    /// // Named services return Some(name)
    /// let named_type = Key::TypeNamed(TypeId::of::<u32>(), "u32", "database_port");
    /// assert_eq!(named_type.service_name(), Some("database_port"));
    ///
    /// let named_trait = Key::TraitNamed("dyn myapp::Logger", "console_logger");
    /// assert_eq!(named_trait.service_name(), Some("console_logger"));
    /// ```
    pub fn service_name(&self) -> Option<&'static str> {
        match self {
            Key::Type(_, _) | Key::Trait(_) | Key::MultiTrait(_, _) => None,
            Key::TypeNamed(_, _, name) => Some(name),
            Key::TraitNamed(_, name) => Some(name),
            Key::MultiTraitNamed(_, name, _) => Some(name),
        }
    }
}

// Ultra-optimized equality for hot path: TypeId-only comparison for concrete types
impl PartialEq for Key {
    #[inline(always)]
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            // Hot path: TypeId comparison only (ignore string for performance)
            (Key::Type(a, _), Key::Type(b, _)) => a == b,
            (Key::TypeNamed(a, _, name_a), Key::TypeNamed(b, _, name_b)) => a == b && name_a == name_b,
            
            // Multi-bindings and traits (less common)
            (Key::Trait(a), Key::Trait(b)) => a == b,
            (Key::TraitNamed(a, name_a), Key::TraitNamed(b, name_b)) => a == b && name_a == name_b,
            (Key::MultiTrait(a, idx_a), Key::MultiTrait(b, idx_b)) => a == b && idx_a == idx_b,
            (Key::MultiTraitNamed(a, name_a, idx_a), Key::MultiTraitNamed(b, name_b, idx_b)) => {
                a == b && name_a == name_b && idx_a == idx_b
            }
            
            // Different variants never equal
            _ => false
        }
    }
}

impl Eq for Key {}

// Ordering for sorting in hybrid registry
impl PartialOrd for Key {
    #[inline(always)]
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Key {
    #[inline(always)]
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        use std::cmp::Ordering;
        
        match (self, other) {
            // Compare by TypeId for concrete types (most common)
            (Key::Type(a, _), Key::Type(b, _)) => a.cmp(b),
            (Key::TypeNamed(a, _, name_a), Key::TypeNamed(b, _, name_b)) => {
                a.cmp(b).then_with(|| name_a.cmp(name_b))
            }
            
            // Different variants - order by variant type
            (Key::Type(_, _), _) => Ordering::Less,
            (_, Key::Type(_, _)) => Ordering::Greater,
            (Key::TypeNamed(_, _, _), _) => Ordering::Less,
            (_, Key::TypeNamed(_, _, _)) => Ordering::Greater,
            
            // Handle remaining variants
            (Key::Trait(a), Key::Trait(b)) => a.cmp(b),
            (Key::TraitNamed(a, name_a), Key::TraitNamed(b, name_b)) => {
                a.cmp(b).then_with(|| name_a.cmp(name_b))
            }
            (Key::MultiTrait(a, idx_a), Key::MultiTrait(b, idx_b)) => {
                a.cmp(b).then_with(|| idx_a.cmp(idx_b))
            }
            (Key::MultiTraitNamed(a, name_a, idx_a), Key::MultiTraitNamed(b, name_b, idx_b)) => {
                a.cmp(b).then_with(|| name_a.cmp(name_b)).then_with(|| idx_a.cmp(idx_b))
            }
            
            // All other cases use ordering based on variant position
            _ => Ordering::Equal, // Should not reach here with exhaustive match
        }
    }
}

// Ultra-optimized hash for hot path: TypeId-only hash for concrete types
impl std::hash::Hash for Key {
    #[inline(always)]
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match self {
            // Hot path: Hash TypeId only (ignore string for performance)
            Key::Type(id, _) => {
                0u8.hash(state); // Discriminant
                id.hash(state);
            }
            Key::TypeNamed(id, _, name) => {
                1u8.hash(state);
                id.hash(state);
                name.hash(state);
            }
            
            // Multi-bindings and traits (less common)
            Key::Trait(name) => {
                2u8.hash(state);
                name.hash(state);
            }
            Key::TraitNamed(name, named) => {
                3u8.hash(state);
                name.hash(state);
                named.hash(state);
            }
            Key::MultiTrait(name, idx) => {
                4u8.hash(state);
                name.hash(state);
                idx.hash(state);
            }
            Key::MultiTraitNamed(name, named, idx) => {
                5u8.hash(state);
                name.hash(state);
                named.hash(state);
                idx.hash(state);
            }
        }
    }
}

// Helper function for creating type keys - add aggressive inlining
#[inline(always)]
pub fn key_of_type<T: 'static>() -> Key {
    Key::Type(std::any::TypeId::of::<T>(), std::any::type_name::<T>())
}