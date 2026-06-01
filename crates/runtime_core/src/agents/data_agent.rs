//! DataAgent — allocates a private 64KB VMO vault on startup.
use nexus_substrate::kernel::{register_handle, HandleType, ffi};

pub struct DataAgent {
    pub id: String,
    pub vmo_handle: Option<u32>,
}

impl DataAgent {
    pub fn new(id: &str) -> Self {
        Self { id: id.to_string(), vmo_handle: None }
    }

    pub fn initialize_vault(&mut self) {
        let size = 64 * 1024;
        let handle = unsafe { ffi::zx_vmo_create_native(size) };
        if handle != 0 {
            register_handle(handle, HandleType::Vmo(handle));
            self.vmo_handle = Some(handle);
            println!(
                "\x1b[35m[DATA_AGENT:{}] Vault carved at handle 0x{:x}\x1b[0m",
                self.id, handle
            );
        } else {
            eprintln!("[DATA_AGENT:{}] VMO allocation failed", self.id);
        }
    }
}
