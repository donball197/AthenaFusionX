/**
 * Copyright 2026 Donald Ball
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 * http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR REPRESENTATIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */


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
