use crate::kernel::error::KernelError;

/// RealignmentEngine: Native Rust implementation for ARM64 cache alignment.
pub struct RealignmentEngine;

impl RealignmentEngine {
    /// CACHE_LINE_SIZE: Standard ARM64 cache line size in bytes.
    pub const CACHE_LINE_SIZE: usize = 64;
    /// ALIGN_MASK: Mask for bitwise alignment check (64 - 1).
    pub const ALIGN_MASK: usize = Self::CACHE_LINE_SIZE - 1;

    /// check_alignment_fast: Deterministically verifies 64-byte boundaries.
    /// AUDITOR FIX: Uses bitwise logic and includes a sanity assertion.
    #[inline(always)]
    pub fn check_alignment_fast(address: *const u8) -> Result<(), KernelError> {
        let addr_val = address as usize;
        
        // Ensure the mask is valid for the constant cache size
        debug_assert_eq!(Self::ALIGN_MASK, 63);

        if (addr_val & Self::ALIGN_MASK) == 0 {
            Ok(())
        } else {
            Err(KernelError::AlignmentAnomaly { address: addr_val })
        }
    }
}
