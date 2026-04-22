pub mod error;
pub mod ffi;
pub mod memory;
pub mod realignment;
pub mod vmar;
pub mod vmo;

use lazy_static::lazy_static;
use std::collections::HashMap;
use std::sync::RwLock;

pub use ffi::{
    muscle_get_active_vmos, muscle_get_next_handle, zx_handle_close_native, zx_vmo_create_native,
    zx_vmo_get_all_handles, zx_vmo_get_info, zx_vmo_read_native, zx_vmo_write_native,
};

lazy_static! {
    pub static ref HANDLE_REGISTRY: RwLock<HashMap<u32, HandleType>> = RwLock::new(HashMap::new());
}

pub enum HandleType {
    Vmo(u32),
    Vmar(vmar::Vmar),
}
pub struct VmoGuard {
    pub id: u32,
}

impl Drop for VmoGuard {
    fn drop(&mut self) {
        if let Ok(mut reg) = HANDLE_REGISTRY.write() {
            if reg.contains_key(&self.id) {
                unsafe {
                    zx_handle_close_native(self.id);
                }
                reg.remove(&self.id);
            }
        }
    }
}

pub fn get_live_telemetry() -> (u32, u32) {
    unsafe { (muscle_get_active_vmos(), muscle_get_next_handle()) }
}

pub fn close_handle(id: u32) -> bool {
    if let Ok(mut reg) = HANDLE_REGISTRY.write() {
        if reg.contains_key(&id) {
            unsafe {
                zx_handle_close_native(id);
            }
            reg.remove(&id);
            true
        } else {
            false
        }
    } else {
        false
    }
}

pub fn register_handle(id: u32, h: HandleType) {
    if let Ok(mut reg) = HANDLE_REGISTRY.write() {
        reg.insert(id, h);
    }
}
