/*
 * NEXUS KERNEL — FFI Bridge v1.3 [ZIG_FUSED]
 */
#[link(name = "muscle")]
extern "C" {
    fn zx_vmo_create_native(size: usize) -> u32;
    fn zx_handle_close_native(handle: u32);
}

pub trait KernelOps: Send + Sync {
    fn vmo_clone(&self, parent_handle: u32, size: usize) -> u32;
    fn handle_close(&self, handle: u32);
}

pub struct NativeOps;
impl NativeOps { pub fn new() -> Self { Self } }

impl KernelOps for NativeOps {
    fn vmo_clone(&self, _parent: u32, size: usize) -> u32 {
        unsafe { zx_vmo_create_native(size) }
    }
    fn handle_close(&self, handle: u32) {
        unsafe { zx_handle_close_native(handle) }
    }
}
