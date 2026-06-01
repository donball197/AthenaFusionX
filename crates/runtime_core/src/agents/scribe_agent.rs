//! ScribeAgent — VMO read/write helper
//! Removed broken NexusAgent trait impl (trait was never defined).
//! ScribeAgent methods are called directly by the executor.

pub struct ScribeAgent;

impl ScribeAgent {
    pub fn enshrine(handle: u32, offset: usize, data: &str) -> bool {
        let bytes = data.as_bytes();
        unsafe {
            nexus_substrate::kernel::ffi::zx_vmo_write_native(
                handle, offset, bytes.as_ptr(), bytes.len()
            ) == 0
        }
    }

    pub fn recall(handle: u32, offset: usize, size: usize) -> Option<String> {
        let mut buffer = vec![0u8; size];
        unsafe {
            if nexus_substrate::kernel::ffi::zx_vmo_read_native(
                handle, offset, buffer.as_mut_ptr(), size
            ) == 0 {
                String::from_utf8(buffer).ok()
            } else {
                None
            }
        }
    }
}
