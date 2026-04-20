/*
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
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

// src/reflex/mod.rs - The FFI Gateway
//
// FIX: Removed the broken extract_reflex declaration.
// execute_extraction now delegates to the thermal pulse pathway, which is the
// actual data source on this substrate.

#[link(name = "muscle")]
extern "C" {
    pub fn extract_thermal_pulse_raw() -> i32;
}

/// execute_extraction: Reads raw thermal pulse data from the Zig kernel and
/// returns it as a byte vector compatible with the existing DNA pipeline.
pub fn execute_extraction(_raw_dna: &[u8]) -> Vec<u8> {
    let raw_millidegrees = unsafe { extract_thermal_pulse_raw() };
    // Encode the i32 as 4 little-endian bytes so callers get a non-empty buffer.
    raw_millidegrees.to_le_bytes().to_vec()
}
