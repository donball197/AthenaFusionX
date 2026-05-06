pub mod ffi;
pub mod error;
pub mod safe_vmo;

use std::collections::HashMap;
use parking_lot::RwLock;
use once_cell::sync::Lazy;

pub enum HandleType {
    Vmo(u32),
}

pub static HANDLE_REGISTRY: Lazy<RwLock<HashMap<u32, HandleType>>> = 
    Lazy::new(|| RwLock::new(HashMap::new()));

pub fn register_handle(id: u32, h: HandleType) {
    HANDLE_REGISTRY.write().insert(id, h);
}
