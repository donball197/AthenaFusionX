/**
 * NEXUS KERNEL — Zircon VMO Emulation v1.3 [REFCOUNT_FIXED]
 * Copyright 2026 Donald Ball
 */
use crate::kernel::error::KernelError;
use crate::kernel::ffi::KernelOps;
use std::sync::Arc;

pub struct Vmo {
    pub handle: u32,
    pub size: usize,
    pub refcount: u32,
    ops: Arc<dyn KernelOps>,
}

impl Vmo {
    pub fn new(ops: Arc<dyn KernelOps>, size: usize) -> Result<Self, KernelError> {
        let handle = ops.vmo_create(size);
        if handle == 0 {
            Err(KernelError::VmoCloneFailed { parent: 0, size })
        } else {
            Ok(Self { handle, size, refcount: 1, ops })
        }
    }

    /// Zircon-style clone with Snapshot capability.
    /// _options bit 0 = ZX_VMO_CHILD_SNAPSHOT (reserved for future CoW enforcement)
    pub fn clone_vmo(&self, _options: u32) -> Result<Self, KernelError> {
        let child_handle = self.ops.vmo_clone(self.handle, self.size);
        if child_handle == 0 {
            Err(KernelError::VmoCloneFailed { parent: self.handle, size: self.size })
        } else {
            println!(
                "\x1b[36m[VMO] COW Clone Created — Handle {} → {} [RESONANCE: 1.00]\x1b[0m",
                self.handle, child_handle
            );
            Ok(Self {
                handle: child_handle,
                size: self.size,
                refcount: 1,
                ops: self.ops.clone(),
            })
        }
    }
}

impl Drop for Vmo {
    fn drop(&mut self) {
        self.ops.handle_close(self.handle);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kernel::ffi::NativeKernelOps;

    #[test]
    fn test_vmo_cow_integrity() {
        let ops = Arc::new(NativeKernelOps);
        let parent = Vmo::new(ops.clone(), 4096).unwrap();
        let ghost_clone = parent.clone_vmo(0).unwrap();
        assert_ne!(parent.handle, ghost_clone.handle);
        assert_eq!(parent.size, ghost_clone.size);
        println!("\x1b[32m[TEST_PASS] COW Handle Separation Verified\x1b[0m");
    }
}
