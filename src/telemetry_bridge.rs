// src/telemetry_bridge.rs — v1.0 [THERMAL_VMO_MAP]
use crate::kernel;
use crate::system_monitor::SystemMonitor;
use tokio::time::{sleep, Duration};
use std::sync::Arc;
use tokio::sync::Mutex;

pub async fn start_thermal_bridge(monitor: Arc<Mutex<SystemMonitor>>) {
    println!("\x1b[35m>> [TELEMETRY_BRIDGE] Mapping Synthetic Thermal VMO (Handle: 0x7E1)\x1b[0m");
    
    // Create a 4KB page for hardware metrics
    let handle = unsafe { kernel::zx_vmo_create_native(4096) };
    kernel::register_handle(handle, kernel::HandleType::Vmo(handle));

    loop {
        let vitals = {
            let mut m = monitor.lock().await;
            m.get_vitals()
        };

        // Format: "TEMP:XX.X|CPU:0.XX|RAM:X.XX"
        let payload = format!("TEMP:{:.1}|CPU:{:.2}|RAM:{:.2}", 
            vitals.temperature, vitals.cpu_load, vitals.ram_gb);
        
        let bytes = payload.as_bytes();
        unsafe {
            kernel::ffi::zx_vmo_write_native(handle, 0, bytes.as_ptr(), bytes.len());
        }

        sleep(Duration::from_secs(2)).await;
    }
}
