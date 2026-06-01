//! Lock-free VMO registry using DashMap.
//!
//! Previously used `RwLock<HashMap<VmoHandle, Arc<SafeVmo>>>` which serialised
//! every write (allocate/release) behind a single global lock. Under concurrent
//! agent workloads this became a hot contention point.
//!
//! DashMap shards the map into N buckets (default: 2× logical CPUs). Each
//! shard has its own RwLock, so concurrent operations on different handles
//! proceed in parallel with no cross-shard blocking.
//!
//! API is fully backward-compatible — callers need zero changes.

use dashmap::DashMap;
use std::sync::{
    atomic::{AtomicU32, Ordering},
    Arc,
};

pub type VmoHandle = u32;

pub struct SafeVmo {
    pub handle: VmoHandle,
    pub data:   Vec<u8>,
    pub timestamp: u64,
}

pub struct VmoHandler {
    /// Sharded concurrent map — no global lock on reads or cross-shard writes.
    registry:    DashMap<VmoHandle, Arc<SafeVmo>>,
    /// Atomic counter replaces the second RwLock entirely.
    next_handle: AtomicU32,
}

impl VmoHandler {
    pub fn new() -> Self {
        Self {
            registry:    DashMap::new(),
            next_handle: AtomicU32::new(1),
        }
    }

    pub fn allocate_cognitive_window(&self, raw_data: &[u8]) -> Result<VmoHandle, String> {
        // fetch_add is lock-free — no mutex involved at all for handle allocation.
        let handle = self.next_handle.fetch_add(1, Ordering::Relaxed);
        let vmo = Arc::new(SafeVmo {
            handle,
            data: raw_data.to_vec(),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        });
        self.registry.insert(handle, vmo);
        Ok(handle)
    }

    pub fn read_cognitive_window(&self, handle: VmoHandle) -> Result<Vec<u8>, String> {
        self.registry
            .get(&handle)
            .map(|v| v.data.clone())
            .ok_or_else(|| format!("Invalid handle: {}", handle))
    }

    pub fn export_provenance_checkpoint(
        &self,
        handle: VmoHandle,
    ) -> Result<(VmoHandle, Vec<u8>, u64), String> {
        self.registry
            .get(&handle)
            .map(|v| (v.handle, v.data.clone(), v.timestamp))
            .ok_or_else(|| format!("Handle not found: {}", handle))
    }

    pub fn release_cognitive_window(&self, handle: VmoHandle) -> Result<(), String> {
        self.registry
            .remove(&handle)
            .map(|_| ())
            .ok_or_else(|| format!("Handle not found on release: {}", handle))
    }

    /// Returns the number of live VMO allocations.
    pub fn active_count(&self) -> usize {
        self.registry.len()
    }
}

impl Default for VmoHandler {
    fn default() -> Self {
        Self::new()
    }
}
