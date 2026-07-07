use crate::kernel::error::KernelError;
use crate::kernel::{NEXT_HANDLE, VMO_REGISTRY};
use std::sync::atomic::Ordering;

pub struct SafeKernel;

impl SafeKernel {
    pub fn create_vmo(size: usize) -> Result<u32, KernelError> {
        if size == 0 || size > 16_777_216 {
            return Err(KernelError::InvalidArgs);
        }
        let handle = NEXT_HANDLE.fetch_add(1, Ordering::Relaxed);
        VMO_REGISTRY.insert(handle, vec![0u8; size]);
        Ok(handle)
    }

    pub fn close_handle(handle: u32) {
        VMO_REGISTRY.remove(&handle);
    }
}
