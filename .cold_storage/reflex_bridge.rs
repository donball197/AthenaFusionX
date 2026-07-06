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


// src/reflex_bridge.rs - v1.0 (Static Linkage FFI Bridge)

extern "C" {
    pub fn extract_thermal_pulse_raw() -> i32;

    pub fn extract_payload_strands(
        input_ptr: *const u8,
        input_len: usize,
        target_ptr: *const u8,
        target_len: usize,
        output_ptr: *mut u8,
        output_max: usize,
    ) -> usize;
}

// Wrapper for safe Rust consumption
pub fn get_system_temperature() -> f32 {
    unsafe {
        let raw = extract_thermal_pulse_raw();
        if raw > 0 {
            (raw as f32) / 1000.0
        } else {
            0.0
        }
    }
}
