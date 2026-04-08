use crate::kernel::error::KernelError;
use crate::kernel::ffi::{self, ZX_VM_PERM_READ, ZX_VM_PERM_WRITE, ZX_VM_MAP_RANGE};
use crate::kernel::vmo::Vmo;
use std::ptr;

/// VirtualMemoryAddressRegion: Orchestrates address space mapping.
pub struct Vmar;

impl Vmar {
    /// map: Maps a VMO into the specified VMAR handle using centralized permissions.
    pub fn map(vmar_handle: u32, vmo: &Vmo) -> Result<*mut u8, KernelError> {
        let options = ZX_VM_PERM_READ | ZX_VM_PERM_WRITE | ZX_VM_MAP_RANGE;
        
        let addr = unsafe {
            ffi::raw::nexus_vmar_map(vmar_handle, options, vmo.handle, vmo.size)
        };

        if addr == ptr::null_mut() {
            Err(KernelError::VmarMapFailed { 
                vmar_handle,
                size: vmo.size 
            })
        } else {
            Ok(addr)
        }
    }
}
