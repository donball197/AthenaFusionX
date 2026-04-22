//! NEXUS KERNEL — mod.rs v2.0
//! Exports: ffi, HandleType, VmoHandle, register_handle, get_live_telemetry,
//!          zx_vmo_create_native (re-export for crate::kernel::zx_vmo_create_native callers)
use lazy_static::lazy_static;
use parking_lot::RwLock;

pub mod ffi;
pub mod vmo;
pub mod vmar;
pub mod error;
pub mod memory;
pub mod realignment;

// ── Handle type aliases ─────────────────────────────────────────────────
pub type VmoHandle = u32;

#[derive(Debug, Clone, Copy)]
pub enum HandleType {
    Vmo(VmoHandle),
}

// ── Global handle registry ─────────────────────────────────────────────
lazy_static! {
    static ref HANDLE_REGISTRY: RwLock<Vec<VmoHandle>> = RwLock::new(Vec::new());
}

/// register_handle: Inserts a new VMO handle into the sovereign registry.
/// Uses if-let to handle RwLock poisoning gracefully instead of panicking.
pub fn register_handle(handle: VmoHandle, _handle_type: HandleType) {
    if let Ok(mut reg) = HANDLE_REGISTRY.write() {
        reg.push(handle);
    } else {
        eprintln!("\x1b[31m[KERNEL] WARNING: HANDLE_REGISTRY RwLock poisoned — skipping insert for 0x{:x}\x1b[0m", handle);
    }
}

/// get_live_telemetry: Returns (active_vmo_count, next_expected_handle).
/// Uses parking_lot's infallible read() — no unwrap needed.
pub fn get_live_telemetry() -> (usize, usize) {
    let reg = HANDLE_REGISTRY.read();
    let active = reg.len();
    (active, active + 1)
}

/// Convenience re-export so callers using `crate::kernel::zx_vmo_create_native`
/// compile without modification.
#[inline(always)]
pub unsafe fn zx_vmo_create_native(size: usize) -> u32 {
    ffi::zx_vmo_create_native(size as u32)
}
