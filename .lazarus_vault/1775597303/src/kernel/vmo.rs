use crate::kernel::error::KernelError;
use crate::kernel::ffi::KernelOps;
use std::sync::Arc;

/// Vmo: Virtual Memory Object owning a kernel handle.
pub struct Vmo {
    pub handle: u32,
    pub size: usize,
    ops: Arc<dyn KernelOps>,
}

impl Vmo {
    /// clone: Generates a child VMO using the provided hardware interface.
    pub fn clone(ops: Arc<dyn KernelOps>, parent: u32, size: usize) -> Result<Self, KernelError> {
        if parent == 0 { return Err(KernelError::InvalidHandle(0)); }
        
        let handle = ops.vmo_clone(parent, size);
        if handle == 0 {
            Err(KernelError::VmoCloneFailed { parent, size })
        } else {
            Ok(Self { handle, size, ops })
        }
    }
}

impl Drop for Vmo {
    fn drop(&mut self) {
        self.ops.handle_close(self.handle);
    }
}
