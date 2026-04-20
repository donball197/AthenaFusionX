pub mod vmo;
pub mod vmar;
pub mod error;
pub mod ffi;

use std::collections::HashMap;
use std::sync::RwLock;
use lazy_static::lazy_static;

pub use ffi::{
    zx_vmo_create_native, zx_handle_close_native, zx_vmo_write_native,
    zx_vmo_read_native, muscle_get_active_vmos, muscle_get_next_handle,
    zx_vmo_get_info, zx_vmo_get_all_handles
};

lazy_static! {
    pub static ref HANDLE_REGISTRY: RwLock<HashMap<u32, HandleType>> = RwLock::new(HashMap::new());
}

pub enum HandleType { Vmo(u32), Vmar(vmar::Vmar) }
pub struct VmoGuard { pub id: u32 }

impl Drop for VmoGuard {
    fn drop(&mut self) {
        let mut reg = HANDLE_REGISTRY.write().unwrap();
        if reg.contains_key(&self.id) {
            unsafe { zx_handle_close_native(self.id); }
            reg.remove(&self.id);
        }
    }
}

pub fn get_live_telemetry() -> (u32, u32) { unsafe { (muscle_get_active_vmos(), muscle_get_next_handle()) } }
pub fn close_handle(id: u32) -> bool {
    let mut reg = HANDLE_REGISTRY.write().unwrap();
    if reg.contains_key(&id) { unsafe { zx_handle_close_native(id); } reg.remove(&id); true } else { false }
}
pub fn register_handle(id: u32, h: HandleType) { HANDLE_REGISTRY.write().unwrap().insert(id, h); }
