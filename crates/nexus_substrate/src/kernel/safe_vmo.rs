use crate::kernel::error::KernelError;
use crate::kernel::{register_handle, HandleType};
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
#[derive(Serialize)]
struct VmoWriteReq {
    handle: u32,
    offset: usize,
    data: Vec<u8>,
}
#[derive(Serialize)]
struct VmoReadReq {
    handle: u32,
    offset: usize,
    size: usize,
}

fn node_url() -> String {
    std::env::var("NEXUS_NODE_URL").unwrap_or_else(|_| "http://127.0.0.1:8080".to_string())
}

pub struct SafeVmo {
    handle: u32,
    pub size: usize,
}

impl SafeVmo {
    pub fn new(size: usize) -> Result<Self, KernelError> {
        let client = Client::builder().build().unwrap();
        let res = client
            .post(format!("{}/api/vmo/create", node_url()))
            .header("Authorization", "SOVEREIGN")
            .json(&VmoCreateReq { size })
            .send()
            .map_err(|_| KernelError::InvalidArgs)?;

        if res.status().is_success() {
            let body: VmoCreateRes = res.json().map_err(|_| KernelError::InvalidArgs)?;
            register_handle(body.handle, HandleType::Vmo(body.handle));
            Ok(Self { handle: body.handle, size })
        } else {
            Err(KernelError::InvalidArgs)
        }
    }

    pub fn write(&self, offset: usize, data: &[u8]) -> Result<(), KernelError> {
        let end = offset.checked_add(data.len()).ok_or(KernelError::InvalidArgs)?;
        if end > self.size {
            return Err(KernelError::InvalidArgs);
        }

        let client = Client::builder().build().unwrap();
        let res = client
            .post(format!("{}/api/vmo/write", node_url()))
            .header("Authorization", "SOVEREIGN")
            .json(&VmoWriteReq { handle: self.handle, offset, data: data.to_vec() })
            .send()
            .map_err(|_| KernelError::InvalidArgs)?;

        if res.status().is_success() {
            Ok(())
        } else {
            Err(KernelError::InvalidArgs)
        }
    }

    pub fn read_into(&self, offset: usize, buffer: &mut [u8]) -> Result<(), KernelError> {
        let end = offset.checked_add(buffer.len()).ok_or(KernelError::InvalidArgs)?;
        if end > self.size {
            return Err(KernelError::InvalidArgs);
        }

        let client = Client::builder().build().unwrap();
        let res = client
            .post(format!("{}/api/vmo/read", node_url()))
            .header("Authorization", "SOVEREIGN")
            .json(&VmoReadReq { handle: self.handle, offset, size: buffer.len() })
            .send()
            .map_err(|_| KernelError::InvalidArgs)?;

        if res.status().is_success() {
            let data: Vec<u8> = res.json().map_err(|_| KernelError::InvalidArgs)?;
            if data.len() == buffer.len() {
                buffer.copy_from_slice(&data);
                Ok(())
            } else {
                Err(KernelError::InvalidArgs)
            }
        } else {
            Err(KernelError::InvalidArgs)
        }
    }

    pub fn handle(&self) -> u32 {
        self.handle
    }
}

impl Drop for SafeVmo {
    fn drop(&mut self) {
        // Drop handled remotely by node lifespan or future close endpoint
    }
}
