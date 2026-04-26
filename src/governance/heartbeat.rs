use crate::system_monitor::SystemMonitor;
use crate::kernel;
use tracing::warn;
use tokio::time::{sleep, Duration};

pub async fn monitor_sovereignty(mut monitor: SystemMonitor) {
    loop {
        let vitals = monitor.get_vitals();
        
        if vitals.cpu_load > 0.95 || vitals.ram_gb > 3.5 {
            warn!("CRITICAL_RESOURCE_HIJACK: Initiating emergency VMO purge.");
            
            // ⚡ FIX: Collect handles to avoid RwLock Deadlock
            let handles: Vec<u32> = if let Ok(reg) = kernel::HANDLE_REGISTRY.read() {
                reg.keys().cloned().collect()
            } else { Vec::new() };

            for handle in handles {
                kernel::close_handle(handle);
            }
        }
        sleep(Duration::from_millis(1000)).await;
    }
}
