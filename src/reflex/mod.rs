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
