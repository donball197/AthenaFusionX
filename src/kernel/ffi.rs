// src/kernel/ffi.rs — v3.1
//
// FIX: VmoInfo.is_cow was `bool`. The C ABI does not guarantee a stable size
//      for bool across compilers/platforms (it is 1 byte in practice on x86-64
//      and aarch64, but the Rust reference calls it "implementation defined"
//      when used in #[repr(C)] structs). Changed to u8 (0 = false, 1 = true)
//      which has a guaranteed ABI size. The Zig side must match.
//
// UPGRADE: Added extract_thermal_pulse_raw to the extern block so that any
//          module that needs the raw pulse value can import it from here
//          without declaring its own extern block.

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct VmoInfo {
    pub handle: u32,
    pub size: usize,
    pub committed_pages: usize,
    /// FIX: was `bool` — changed to u8 for guaranteed C ABI compatibility.
    /// 0 = not a COW child, 1 = COW child.
    pub is_cow: u8,
    pub parent_handle: u32,
}

#[link(name = "muscle")]
extern "C" {
    pub fn zx_vmo_create_native(size: usize) -> u32;
    pub fn zx_vmo_clone_native(parent_handle: u32, size: usize) -> u32;
    pub fn zx_handle_close_native(handle: u32);
    pub fn zx_vmo_write_native(
        handle: u32,
        offset: usize,
        buffer: *const u8,
        size: usize,
    ) -> i32;
    pub fn zx_vmo_read_native(
        handle: u32,
        offset: usize,
        buffer: *mut u8,
        size: usize,
    ) -> i32;
    pub fn muscle_get_active_vmos() -> u32;
    pub fn muscle_get_next_handle() -> u32;
    pub fn set_substrate_load(load: f32);
    /// ADDED: Centralised here so no other module needs its own extern block.
    pub fn extract_thermal_pulse_raw() -> i32;
    pub fn zx_vmo_get_info(handle: u32, out_info: *mut VmoInfo) -> i32;
    pub fn zx_vmo_get_all_handles(out_handles: *mut u32, max_count: usize) -> usize;
}

pub trait KernelOps: Send + Sync {
    fn vmo_create(&self, size: usize) -> u32;
    fn vmo_clone(&self, parent_handle: u32, size: usize) -> u32;
    fn handle_close(&self, handle: u32);
    fn get_active_vmos(&self) -> u32;
    fn get_next_handle(&self) -> u32;
    fn set_substrate_load(&self, load: f32);
    fn vmo_get_info(&self, handle: u32, out_info: *mut VmoInfo) -> i32;
    fn vmo_get_all_handles(
        &self,
        out_handles: *mut u32,
        max_count: usize,
    ) -> usize;
}

pub struct NativeKernelOps;

impl KernelOps for NativeKernelOps {
    fn vmo_create(&self, size: usize) -> u32 {
        unsafe { zx_vmo_create_native(size) }
    }
    fn vmo_clone(&self, parent_handle: u32, size: usize) -> u32 {
        unsafe { zx_vmo_clone_native(parent_handle, size) }
    }
    fn handle_close(&self, handle: u32) {
        unsafe { zx_handle_close_native(handle) }
    }
    fn get_active_vmos(&self) -> u32 {
        unsafe { muscle_get_active_vmos() }
    }
    fn get_next_handle(&self) -> u32 {
        unsafe { muscle_get_next_handle() }
    }
    fn set_substrate_load(&self, load: f32) {
        unsafe { set_substrate_load(load) }
    }
    fn vmo_get_info(&self, handle: u32, out_info: *mut VmoInfo) -> i32 {
        unsafe { zx_vmo_get_info(handle, out_info) }
    }
    fn vmo_get_all_handles(
        &self,
        out_handles: *mut u32,
        max_count: usize,
    ) -> usize {
        unsafe { zx_vmo_get_all_handles(out_handles, max_count) }
    }
}
