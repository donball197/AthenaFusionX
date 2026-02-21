use std::ffi::CString;
use std::os::raw::{c_char, c_int, c_float};
use anyhow::Result;
use memmap2::Mmap;
use std::fs::File;

extern "C" {
    fn load_model(path: *const c_char) -> c_int;
    fn run_inference(input: *const c_float, len: usize) -> c_float;
    fn calculate_resonance_e2b(ptr: *const u8, len: usize) -> f32;
}

pub struct ResonatorCore;

impl ResonatorCore {
    pub fn ignite_vault(model_path: &str) -> Result<String> {
        let c_path = CString::new(model_path).expect("CString failed");
        unsafe { load_model(c_path.as_ptr()); }
        Ok("Vault Linked".to_string())
    }

    pub fn scan_local_energy(path: &str) -> Result<f32> {
        let file = File::open(path)?;
        let mmap = unsafe { Mmap::map(&file)? };
        
        // Scan a 10MB slice from the middle of the brain
        let start = mmap.len() / 2;
        let scan_size = 10 * 1024 * 1024; 
        let len = std::cmp::min(mmap.len() - start, scan_size);
        
        let resonance = unsafe { calculate_resonance_e2b(mmap.as_ptr().add(start), len) };
        Ok(resonance)
    }
}
