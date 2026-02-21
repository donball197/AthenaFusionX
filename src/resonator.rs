use std::ffi::CString;
use std::os::raw::{c_char, c_int, c_float};
use anyhow::Result;

extern "C" {
    fn load_model(path: *const c_char) -> c_int;
    fn run_inference(input: *const c_float, len: usize) -> c_float;
}

pub struct ResonatorCore;

impl ResonatorCore {
    pub fn ignite_vault(model_path: &str) -> Result<String> {
        let c_path = CString::new(model_path).expect("CString failed");
        unsafe {
            let status = load_model(c_path.as_ptr());
            if status == 0 {
                Ok("Neural Matrix Loaded".to_string())
            } else {
                Ok(format!("Vault Link Warning: Zig Code {}", status))
            }
        }
    }

    pub fn vision_pulse(pixel_data: Vec<f32>) -> Result<f32> {
        unsafe {
            let result = run_inference(pixel_data.as_ptr(), pixel_data.len());
            Ok(result)
        }
    }
}
