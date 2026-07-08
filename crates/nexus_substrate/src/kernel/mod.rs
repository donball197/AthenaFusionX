pub mod error;
pub mod safe_kernel;
pub mod safe_vmo;
pub mod temp_vmo;

use dashmap::DashMap;
use once_cell::sync::Lazy;
use std::sync::atomic::AtomicU32;

// ⚡ 100% Safe Pure-Rust Virtual Memory Registry (Replaces Zig)
pub static VMO_REGISTRY: Lazy<DashMap<u32, Vec<u8>>> = Lazy::new(DashMap::new);
pub static NEXT_HANDLE: AtomicU32 = AtomicU32::new(100);

pub fn get_active_vmos() -> u32 {
    VMO_REGISTRY.len() as u32
}

pub fn calculate_entropy(handle: u32) -> f32 {
    if let Some(vmo) = VMO_REGISTRY.get(&handle) {
        let data = vmo.value();
        if data.is_empty() {
            return 0.0;
        }

        let mut counts = [0u32; 256];
        for &b in data {
            counts[b as usize] += 1;
        }

        let mut entropy = 0.0f32;
        let len = data.len() as f32;
        for &c in &counts {
            if c > 0 {
                let p = c as f32 / len;
                entropy -= p * p.log2();
            }
        }
        entropy / 8.0
    } else {
        0.0
    }
}
