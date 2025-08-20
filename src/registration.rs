//! Service registration types.

use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::sync::Arc;
use crate::error::DiResult;
use crate::key::Key;
use crate::lifetime::Lifetime;

#[cfg(feature = "once-cell")]
use once_cell::sync::OnceCell;

// ResolverContext is defined in provider module
pub(crate) use crate::provider::ResolverContext;

// Type-erased Arc for storage
pub(crate) type AnyArc = Arc<dyn Any + Send + Sync>;

/// Service registration with lifetime and constructor
pub(crate) struct Registration {
    pub(crate) lifetime: Lifetime,
    pub(crate) ctor: Arc<dyn for<'a> Fn(&ResolverContext<'a>) -> DiResult<AnyArc> + Send + Sync>,
    /// Optional metadata for diagnostics and introspection
    pub(crate) metadata: Option<Box<dyn Any + Send + Sync>>,
    /// Implementation type ID for diagnostics (helps identify concrete types backing trait registrations)
    pub(crate) impl_id: Option<TypeId>,
    
    // Hot-path runtime fields for performance optimization
    /// Singleton cache - OnceCell for lock-free access after initialization
    #[cfg(feature = "once-cell")]
    pub(crate) single_runtime: Option<OnceCell<AnyArc>>,
    #[cfg(not(feature = "once-cell"))]
    pub(crate) single_runtime: Option<Arc<std::sync::Mutex<Option<AnyArc>>>>,
    
    /// Scoped slot index for O(1) scoped service resolution
    pub(crate) scoped_slot: Option<usize>,
}

impl Registration {
    /// Creates a new registration with runtime optimization fields initialized
    pub(crate) fn new(
        lifetime: Lifetime,
        ctor: Arc<dyn for<'a> Fn(&ResolverContext<'a>) -> DiResult<AnyArc> + Send + Sync>,
    ) -> Self {
        let single_runtime = match lifetime {
            Lifetime::Singleton => {
                #[cfg(feature = "once-cell")]
                { Some(OnceCell::new()) }
                #[cfg(not(feature = "once-cell"))]
                { Some(Arc::new(std::sync::Mutex::new(None))) }
            }
            _ => None,
        };

        Self {
            lifetime,
            ctor,
            metadata: None,
            impl_id: None,
            single_runtime,
            scoped_slot: None,
        }
    }
    
    /// Creates a new registration with metadata
    pub(crate) fn with_metadata(
        lifetime: Lifetime,
        ctor: Arc<dyn for<'a> Fn(&ResolverContext<'a>) -> DiResult<AnyArc> + Send + Sync>,
        metadata: Option<Box<dyn Any + Send + Sync>>,
        impl_id: Option<TypeId>,
    ) -> Self {
        let mut reg = Self::new(lifetime, ctor);
        reg.metadata = metadata;
        reg.impl_id = impl_id;
        reg
    }
}

/// Service registry holding all registrations
pub(crate) struct Registry {
    /// Fast Vec lookup for first N registrations (cache-friendly)
    pub(crate) one_small: Vec<(Key, Registration)>,
    /// HashMap fallback for remaining registrations  
    pub(crate) one_large: HashMap<Key, Registration>,
    /// Multi-binding registrations (append-only)
    pub(crate) many: HashMap<&'static str, Vec<Registration>>,
    /// Total count of scoped registrations for slot allocation
    pub(crate) scoped_count: usize,
    /// Multi-binding scoped slot mapping: (trait_name, index) -> slot
    pub(crate) multi_scoped_slots: HashMap<(&'static str, usize), usize>,
    /// Threshold for Vec vs HashMap (optimize for small collections)
    pub(crate) small_threshold: usize,
}

impl Registry {
    pub(crate) fn new() -> Self {
        Self {
            one_small: Vec::new(),
            one_large: HashMap::new(),
            many: HashMap::new(),
            scoped_count: 0,
            multi_scoped_slots: HashMap::new(),
            small_threshold: 16, // Optimal based on research: Vec faster for ≤15 items
        }
    }
    
    /// Inserts a registration with optimal storage selection
    pub(crate) fn insert(&mut self, key: Key, registration: Registration) {
        if self.one_small.len() < self.small_threshold {
            // Use Vec for small collections (cache-friendly linear search)
            if let Some(pos) = self.one_small.iter().position(|(k, _)| k == &key) {
                // Replace existing
                self.one_small[pos] = (key, registration);
            } else {
                // Add new
                self.one_small.push((key, registration));
            }
        } else {
            // Check if key exists in small Vec first
            if let Some(pos) = self.one_small.iter().position(|(k, _)| k == &key) {
                // Replace in Vec
                self.one_small[pos] = (key, registration);
            } else {
                // Use HashMap for larger collections
                self.one_large.insert(key, registration);
            }
        }
    }
    
    /// Gets a registration with optimal lookup
    #[inline(always)]
    pub(crate) fn get(&self, key: &Key) -> Option<&Registration> {
        // Fast path: linear search through Vec (cache-friendly for small collections)
        for (k, reg) in &self.one_small {
            if k == key {
                return Some(reg);
            }
        }
        
        // Fallback: HashMap lookup
        self.one_large.get(key)
    }
    
    /// Checks if a key exists in the registry
    #[inline(always)]
    pub(crate) fn contains_key(&self, key: &Key) -> bool {
        // Check Vec first
        for (k, _) in &self.one_small {
            if k == key {
                return true;
            }
        }
        
        // Check HashMap
        self.one_large.contains_key(key)
    }
    
    /// Gets a mutable reference to a registration
    pub(crate) fn get_mut(&mut self, key: &Key) -> Option<&mut Registration> {
        // Check Vec first
        for (k, reg) in &mut self.one_small {
            if k == key {
                return Some(reg);
            }
        }
        
        // Check HashMap
        self.one_large.get_mut(key)
    }
    
    /// Iterator over all key-registration pairs
    pub(crate) fn iter(&self) -> impl Iterator<Item = (&Key, &Registration)> {
        self.one_small.iter().map(|(k, r)| (k, r))
            .chain(self.one_large.iter())
    }

    /// Finalizes registry by assigning scoped slot indices and sorting Vec
    pub(crate) fn finalize(&mut self) {
        // Sort small Vec by Key for better cache locality during lookup
        self.one_small.sort_by(|a, b| a.0.cmp(&b.0));
        
        let mut next_scoped_slot = 0;
        
        // Assign slots to Vec registrations
        for (_, reg) in &mut self.one_small {
            if reg.lifetime == Lifetime::Scoped {
                reg.scoped_slot = Some(next_scoped_slot);
                next_scoped_slot += 1;
            }
        }
        
        // Assign slots to HashMap registrations
        for reg in self.one_large.values_mut() {
            if reg.lifetime == Lifetime::Scoped {
                reg.scoped_slot = Some(next_scoped_slot);
                next_scoped_slot += 1;
            }
        }
        
        // Assign slots to multi registrations  
        for (trait_name, regs) in self.many.iter_mut() {
            for (index, reg) in regs.iter_mut().enumerate() {
                if reg.lifetime == Lifetime::Scoped {
                    reg.scoped_slot = Some(next_scoped_slot);
                    // Also store in multi-binding slot map for easy lookup
                    self.multi_scoped_slots.insert((trait_name, index), next_scoped_slot);
                    next_scoped_slot += 1;
                }
            }
        }
        
        self.scoped_count = next_scoped_slot;
    }
}