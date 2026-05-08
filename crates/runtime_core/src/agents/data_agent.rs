use nexus_substrate::kernel::safe_vmo::SafeVmo;
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct DataAgent {
    pub id: String,
    pub vmo_vault: Option<Arc<Mutex<SafeVmo>>>,
}

impl DataAgent {
    pub fn new(id: &str) -> Self {
        Self { id: id.to_string(), vmo_vault: None }
    }

    pub fn initialize_vault(&mut self) {
        let size = 64 * 1024; // 64KB agent vault
        match SafeVmo::new(size) {
            Ok(vmo) => {
                let handle = vmo.handle();
                self.vmo_vault = Some(Arc::new(Mutex::new(vmo)));
                println!(
                    "\x1b[35m[DATA_AGENT:{}] Vault carved at handle 0x{:x}\x1b[0m",
                    self.id, handle
                );
            }
            Err(e) => {
                println!("\x1b[31m[DATA_AGENT:{}] Vault allocation failed: {:?}\x1b[0m", self.id, e)
            }
        }
    }
}
