/**
 * NEXUS KERNEL — ZMAR Mapping v1.1
 * Virtual Memory Address Region Orchestration
 */
use crate::kernel::error::KernelError;
use crate::kernel::vmo::Vmo;

pub struct Vmar {
    pub addr: usize,
    pub size: usize,
    pub permissions: u32, // R, W, X logic
}

impl Vmar {
    /// Maps a Vmo into this address region
    pub fn map_vmo(&self, vmo: &Vmo, offset: usize, len: usize) -> Result<usize, KernelError> {
        if offset + len > vmo.size {
            return Err(KernelError::InvalidArgs);
        }
        
        let target_addr = self.addr + offset;
        println!("\x1b[35m[VMAR] Mapping VMO handle {} to 0x{:X} [SIZE: {}]\x1b[0m", 
            vmo.handle, target_addr, len);
            
        Ok(target_addr)
    }
}
