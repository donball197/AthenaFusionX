// src/reflex/mod.rs — v3.1 Sovereign Reflex
//
// FIX: Removed the standalone #[link(name = "muscle")] extern block that
//      redeclared extract_thermal_pulse_raw. That symbol is already declared
//      in src/kernel/ffi.rs with the correct link attribute. Two extern blocks
//      for the same symbol produce duplicate-symbol linker warnings (hard errors
//      on LLD). We now import it through crate::kernel::ffi.
//
// The public API (execute_extraction) is unchanged so all callers compile
// without modification.

use crate::kernel::ffi;

/// execute_extraction: Reads the raw thermal pulse from the Zig substrate and
/// returns it as a 4-byte little-endian buffer compatible with the DNA pipeline.
pub fn execute_extraction(_raw_dna: &[u8]) -> Vec<u8> {
    let raw_millidegrees = unsafe { ffi::extract_thermal_pulse_raw() };
    raw_millidegrees.to_le_bytes().to_vec()
}
