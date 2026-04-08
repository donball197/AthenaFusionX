/// Standard alignment check to address CVSS 5.3 Alignment Anomaly
pub fn is_aligned(ptr: usize, align: usize) -> bool {
    if align == 0 || (align & (align - 1)) != 0 {
        return false;
    }
    ptr % align == 0
}

/// Secure wrapper for memory mapping offsets
pub fn validate_vmo_offset(offset: u64, size: u64, map_len: u64) -> Result<(), String> {
    if offset.checked_add(map_len).map_or(true, |end| end > size) {
        return Err("KERNEL_SECURITY: Out of bounds VMO mapping".to_string());
    }
    if !is_aligned(offset as usize, 4096) {
        return Err("KERNEL_SECURITY: Misaligned VMO mapping".to_string());
    }
    Ok(())
}
