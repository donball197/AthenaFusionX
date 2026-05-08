use crate::agent_protocol::{AgentDirective, AgentStatus};
use crate::agents::NexusAgent;
use anyhow::Result;
use async_trait::async_trait;
use nexus_substrate::kernel;

pub struct ScribeAgent;

impl ScribeAgent {
    pub fn enshrine(handle: u32, offset: usize, data: &str) -> bool {
        let bytes = data.as_bytes();
        unsafe {
            kernel::ffi::zx_vmo_write_native(handle, offset, bytes.as_ptr(), bytes.len()) == 0
        }
    }

    pub fn recall(handle: u32, offset: usize, size: usize) -> Option<String> {
        let mut buffer = vec![0u8; size];
        unsafe {
            if kernel::ffi::zx_vmo_read_native(handle, offset, buffer.as_mut_ptr(), size) == 0 {
                String::from_utf8(buffer).ok()
            } else {
                None
            }
        }
    }

    pub fn scry_vault(handle: u32, offset: usize, size: usize) {
        let mut buffer = vec![0u8; size];
        unsafe {
            if kernel::ffi::zx_vmo_read_native(handle, offset, buffer.as_mut_ptr(), size) == 0 {
                println!("\x1b[1;30m--- [SCRY: 0x{:x} | Offset: {}] ---\x1b[0m", handle, offset);
                for chunk in buffer.chunks(16) {
                    let hex: String = chunk.iter().map(|b| format!("{:02x} ", b)).collect();
                    let ascii: String = chunk
                        .iter()
                        .map(|&b| if (32..=126).contains(&b) { b as char } else { '.' })
                        .collect();
                    println!("\x1b[35m{:32} \x1b[1;37m| {}\x1b[0m", hex, ascii);
                }
            } else {
                println!(
                    "\x1b[31m[SCRY] Access Denied: Handle 0x{:x} is opaque or invalid.\x1b[0m",
                    handle
                );
            }
        }
    }
}

#[async_trait]
impl NexusAgent for ScribeAgent {
    fn name(&self) -> &'static str {
        "scribe_io_expert"
    }
    async fn status(&self) -> AgentStatus {
        AgentStatus::Idle
    }
    async fn execute(&self, _directive: AgentDirective) -> Result<String> {
        Ok("Scribe standing by for commutation.".to_string())
    }
}
