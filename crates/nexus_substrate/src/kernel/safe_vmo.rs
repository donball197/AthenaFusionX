use crate::kernel::error::KernelError;
use crate::kernel::safe_kernel::SafeKernel;
use crate::kernel::VMO_REGISTRY;

pub struct SafeVmo {
    handle: u32,
    pub size: usize,
}

impl SafeVmo {
    pub fn new(size: usize) -> Result<Self, KernelError> {
        let handle = SafeKernel::create_vmo(size)?;
        Ok(Self { handle, size })
    }

    pub fn write(&self, offset: usize, data: &[u8]) -> Result<(), KernelError> {
        let end = offset.checked_add(data.len()).ok_or(KernelError::InvalidArgs)?;
        if end > self.size {
            return Err(KernelError::InvalidArgs);
        }

        if let Some(mut vmo) = VMO_REGISTRY.get_mut(&self.handle) {
            vmo[offset..end].copy_from_slice(data);
            Ok(())
        } else {
            Err(KernelError::InvalidArgs)
        }
    }

    pub fn read_into(&self, offset: usize, buffer: &mut [u8]) -> Result<(), KernelError> {
        let end = offset.checked_add(buffer.len()).ok_or(KernelError::InvalidArgs)?;
        if end > self.size {
            return Err(KernelError::InvalidArgs);
        }

        if let Some(vmo) = VMO_REGISTRY.get(&self.handle) {
            buffer.copy_from_slice(&vmo[offset..end]);
            Ok(())
        } else {
            Err(KernelError::InvalidArgs)
        }
    }

    pub fn handle(&self) -> u32 {
        self.handle
    }
}

impl Drop for SafeVmo {
    fn drop(&mut self) {
        SafeKernel::close_handle(self.handle);
    }
}
