pub mod error;
pub mod ffi;
pub mod memory;
pub mod realignment;
pub mod vmar;
pub mod vmo;

use parking_lot::RwLock;
use std::collections::HashMap;
use lazy_static::lazy_static;

pub use ffi::{
    muscle_get_active_vmos, muscle_get_next_handle, zx_handle_close_native,
    zx_vmo_create_native, zx_vmo_get_all_handles, zx_vmo_get_info,
    zx_vmo_read_native, zx_vmo_write_native,
};

lazy_static! {
    pub static ref HANDLE_REGISTRY: RwLock<HashMap<u32, HandleType>> =
        RwLock::new(HashMap::new());
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
        let was_registered = {
            let mut reg = HANDLE_REGISTRY.write();
            reg.remove(&self.id).is_some()
        };

        if was_registered {
            unsafe { zx_handle_close_native(self.id); }
        }
    }
}

pub fn get_live_telemetry() -> (u32, u32) {
    unsafe { (muscle_get_active_vmos(), muscle_get_next_handle()) }
}

pub fn register_handle(id: u32, h: HandleType) {
    HANDLE_REGISTRY.write().insert(id, h);
}

pub fn close_handle(id: u32) -> bool {
    let removed = {
        let mut reg = HANDLE_REGISTRY.write();
        reg.remove(&id).is_some()
    };
    if removed {
        unsafe { zx_handle_close_native(id); }
        true
    } else {
        false
    }
}
