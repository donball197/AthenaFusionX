//! TelemetryBridge — reads real system vitals every 3 s and broadcasts
//! them as a JSON heartbeat that the frontend's store.svelte.ts consumes.
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};
use tokio::time::{interval, Duration};

pub fn init() {
    tracing::info!(">> [TELEMETRY] Bridge initialised");
}

pub fn spawn(
    tx: broadcast::Sender<String>,
    monitor: Arc<Mutex<crate::system_monitor::SystemMonitor>>,
    resonance: Arc<std::sync::atomic::AtomicU32>,
) {
    tokio::spawn(async move {
        let mut ticker = interval(Duration::from_secs(3));
        loop {
            ticker.tick().await;
            let vitals = {
                let mut m = monitor.lock().await;
                m.get_vitals()
            };
            let res = resonance.load(std::sync::atomic::Ordering::Relaxed) as f32 / 1000.0;
            let active_vmos = nexus_substrate::kernel::get_active_vmos();

            let msg = serde_json::json!({
                "type":        "VITALS",
                "cpu_load":    vitals.cpu_load,
                "ram_gb":      vitals.ram_gb,
                "total_ram_gb": vitals.total_ram_gb,
                "temperature": vitals.temperature,
                "resonance":   res,
                "active_vmos": active_vmos,
            });
            let _ = tx.send(msg.to_string());
        }
    });
}
