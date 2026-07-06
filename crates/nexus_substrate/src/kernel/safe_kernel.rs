use crate::kernel::error::KernelError;
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
struct VmoCreateReq {
    size: usize,
}
#[derive(Deserialize)]
struct VmoCreateRes {
    handle: u32,
}

pub struct SafeKernel;

impl SafeKernel {
    pub fn create_vmo(size: usize) -> Result<u32, KernelError> {
        if size == 0 || size > 16_777_216 {
            return Err(KernelError::InvalidArgs);
        }

        let client = Client::builder().build().unwrap();
        let url =
            std::env::var("NEXUS_NODE_URL").unwrap_or_else(|_| "http://127.0.0.1:8080".to_string());

        let res = client
            .post(format!("{}/api/vmo/create", url))
            .header("Authorization", "SOVEREIGN")
            .json(&VmoCreateReq { size })
            .send()
            .map_err(|_| KernelError::InvalidArgs)?;

        if res.status().is_success() {
            let body: VmoCreateRes = res.json().map_err(|_| KernelError::InvalidArgs)?;
            Ok(body.handle)
        } else {
            Err(KernelError::InvalidArgs)
        }
    }

    pub fn close_handle(_handle: u32) {}
}
