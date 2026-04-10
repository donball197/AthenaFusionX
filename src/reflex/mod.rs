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


// src/reflex/mod.rs - The FFI Gateway

extern "C" {
    pub fn extract_reflex(
        input_ptr: *const u8,
        input_len: usize,
        output_ptr: *mut u8,
        output_max_len: usize,
    ) -> usize;
}

pub fn execute_extraction(raw_dna: &[u8]) -> Vec<u8> {
    let mut buffer = vec![0u8; raw_dna.len() * 2]; 
    
    unsafe {
        let result_len = extract_reflex(
            raw_dna.as_ptr(),
            raw_dna.len(),
            buffer.as_mut_ptr(),
            buffer.len(),
        );
        buffer.truncate(result_len);
    }
    
    buffer
}
