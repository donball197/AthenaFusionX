pub mod vmo;
pub mod vmar;
pub mod error;
pub mod ffi;

use std::collections::HashMap;
use std::sync::RwLock;
use lazy_static::lazy_static;
use ffi::VmoInfo;

extern "C" {
    pub fn zx_vmo_create_native(size: usize) -> u32;
    pub fn zx_handle_close_native(handle: u32);
    pub fn zx_vmo_write_native(handle: u32, offset: usize, buffer: *const u8, size: usize) -> i32;
    pub fn zx_vmo_read_native(handle: u32, offset: usize, buffer: *mut u8, size: usize) -> i32;
    pub fn muscle_get_active_vmos() -> u32;
    pub fn muscle_get_next_handle() -> u32;
    pub fn zx_vmo_get_info(handle: u32, out_info: *mut VmoInfo) -> i32;
    pub fn zx_vmo_get_all_handles(out_handles: *mut u32, max_count: usize) -> usize;
}

lazy_static! {
    pub static ref HANDLE_REGISTRY: RwLock<HashMap<u32, HandleType>> = RwLock::new(HashMap::new());
}

pub enum HandleType {
    Vmo(u32),
    Vmar(vmar::Vmar),
}

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

pub fn get_live_telemetry() -> (u32, u32) {
    unsafe { (muscle_get_active_vmos(), muscle_get_next_handle()) }
}

pub fn close_handle(id: u32) -> bool {
    let mut reg = HANDLE_REGISTRY.write().unwrap();
    if reg.contains_key(&id) {
        unsafe { zx_handle_close_native(id); }
        reg.remove(&id);
        true
    } else {
        false
    }
}

pub fn register_handle(id: u32, h: HandleType) {
    let mut reg = HANDLE_REGISTRY.write().unwrap();
    reg.insert(id, h);
}
