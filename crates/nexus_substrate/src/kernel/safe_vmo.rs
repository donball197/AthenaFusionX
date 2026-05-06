use crate::kernel::{register_handle, HandleType};
use crate::kernel::ffi;
use crate::kernel::error::KernelError;

pub struct SafeVmo {
    handle: u32,
    pub size: usize,
}

impl SafeVmo {
    pub fn new(size: usize) -> Result<Self, KernelError> {
        let handle = unsafe { ffi::zx_vmo_create_native(size) };
        if handle == 0 {
            return Err(KernelError::InvalidArgs);
        }
        register_handle(handle, HandleType::Vmo(handle));
        Ok(Self { handle, size })
    }

    pub fn write(&self, offset: usize, data: &[u8]) -> Result<(), KernelError> {
        if offset + data.len() > self.size {
            return Err(KernelError::InvalidArgs);
        }
        let status = unsafe {
            ffi::zx_vmo_write_native(self.handle, offset, data.as_ptr(), data.len())
        };
        if status == 0 { Ok(()) } else { Err(KernelError::InvalidArgs) }
    }

    /// ⚡ ZERO-ALLOCATION READ FIX: Prevents heap fragmentation on the 4GB Duet
    pub fn read_into(&self, offset: usize, buffer: &mut [u8]) -> Result<(), KernelError> {
        if offset + buffer.len() > self.size {
            return Err(KernelError::InvalidArgs);
        }
        let status = unsafe {
            ffi::zx_vmo_read_native(self.handle, offset, buffer.as_mut_ptr(), buffer.len())
        };
        if status == 0 { Ok(()) } else { Err(KernelError::InvalidArgs) }
    }

    pub fn handle(&self) -> u32 {
        self.handle
    }
}

impl Drop for SafeVmo {
    fn drop(&mut self) {
        unsafe { ffi::zx_handle_close_native(self.handle) };
    }
}
