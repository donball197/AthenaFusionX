use crate::kernel;

pub struct DataAgent {
    pub id: String,
    pub vmo_handle: Option<u32>,
}

impl DataAgent {
    pub fn new(id: &str) -> Self {
        Self { id: id.to_string(), vmo_handle: None }
    }

    /// Commands the muscle to carve out a 64KB sovereign vault
    pub fn initialize_vault(&mut self) {
        let size = 64 * 1024; // 64KB agent vault
        let handle = unsafe { kernel::zx_vmo_create_native(size) };
        kernel::register_handle(handle, kernel::HandleType::Vmo(handle));
        self.vmo_handle = Some(handle);
        println!("\x1b[35m[DATA_AGENT:{}] Vault carved at handle 0x{:x}\x1b[0m", self.id, handle);
    }
}
