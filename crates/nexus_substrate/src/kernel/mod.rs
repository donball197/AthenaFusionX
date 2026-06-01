pub mod error;
pub mod ffi;
pub mod safe_vmo;

use once_cell::sync::Lazy;
use parking_lot::RwLock;
use std::collections::HashMap;

pub enum HandleType {
    Vmo(u32),
}

pub static HANDLE_REGISTRY: Lazy<RwLock<HashMap<u32, HandleType>>> =
    Lazy::new(|| RwLock::new(HashMap::new()));

pub fn register_handle(id: u32, h: HandleType) {
    HANDLE_REGISTRY.write().insert(id, h);
}

pub mod safe_kernel;
pub mod temp_vmo;
pub mod ffi_tests;
