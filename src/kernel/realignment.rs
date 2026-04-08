use crate::kernel::error::KernelError;

pub const CACHE_LINE_SIZE: usize = 64;
pub const ALIGN_MASK: usize = CACHE_LINE_SIZE - 1;

pub struct RealignmentEngine;

impl RealignmentEngine {
    /// check_alignment_fast: Deterministically verifies 64-byte boundaries.
    #[inline(always)]
    pub fn check_alignment_fast(address: *const u8) -> Result<(), KernelError> {
        let addr_val = address as usize;
        if (addr_val & ALIGN_MASK) == 0 {
            Ok(())
        } else {
            Err(KernelError::AlignmentAnomaly { address: addr_val })
        }
    }
}
