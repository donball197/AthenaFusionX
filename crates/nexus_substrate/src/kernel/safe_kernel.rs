use crate::kernel::ffi;
use crate::kernel::error::KernelError;

pub struct SafeKernel;

impl SafeKernel {
    pub fn create_vmo(size: usize) -> Result<u32, KernelError> {
        if size == 0 || size > 16_777_216 { return Err(KernelError::InvalidArgs); }
        let handle = unsafe { ffi::zx_vmo_create_native(size) };
        if handle == 0 { return Err(KernelError::InvalidArgs); }
        Ok(handle)
    }

    pub fn close_handle(handle: u32) {
        unsafe { ffi::zx_handle_close_native(handle) }
    }
}
