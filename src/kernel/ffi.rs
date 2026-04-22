#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct VmoInfo {
    pub handle: u32,
    pub size: usize,
    pub committed_pages: usize,
    pub is_cow: u8,
    pub parent_handle: u32,
}

#[link(name = "muscle")]
extern "C" {
    pub fn zx_vmo_create_native(size: usize) -> u32;
    pub fn zx_vmo_clone_native(parent_handle: u32, size: usize) -> u32;
    pub fn zx_handle_close_native(handle: u32);
    pub fn zx_vmo_write_native(handle: u32, offset: usize, buffer: *const u8, size: usize) -> i32;
    pub fn zx_vmo_read_native(handle: u32, offset: usize, buffer: *mut u8, size: usize) -> i32;
    pub fn muscle_get_active_vmos() -> u32;
    pub fn muscle_get_next_handle() -> u32;
    pub fn set_substrate_load(load: f32);
    pub fn extract_thermal_pulse_raw() -> i32;
    pub fn zx_vmo_get_info(handle: u32, out_info: *mut VmoInfo) -> i32;
    pub fn zx_vmo_get_all_handles(out_handles: *mut u32, max_count: usize) -> usize;
}

pub trait KernelOps: Send + Sync {
    fn vmo_create(&self, size: usize) -> u32;
    fn vmo_clone(&self, parent_handle: u32, size: usize) -> u32;
    fn handle_close(&self, handle: u32);
    fn vmo_get_info(&self, handle: u32, out_info: *mut VmoInfo) -> i32;
}

pub struct NativeKernelOps;
impl KernelOps for NativeKernelOps {
    fn vmo_create(&self, size: usize) -> u32 {
        unsafe { zx_vmo_create_native(size) }
    }
    fn vmo_clone(&self, p: u32, s: usize) -> u32 {
        unsafe { zx_vmo_clone_native(p, s) }
    }
    fn handle_close(&self, h: u32) {
        unsafe { zx_handle_close_native(h) }
    }
    fn vmo_get_info(&self, h: u32, i: *mut VmoInfo) -> i32 {
        unsafe { zx_vmo_get_info(h, i) }
    }
}
