//! TelemetryBridge — reads real system vitals every 3 s and broadcasts
//! them as a JSON heartbeat that the frontend's store.svelte.ts consumes.
use opentelemetry::{global, propagation::Injector};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};
use tokio::time::{interval, Duration};

/// Carrier adapter for Reqwest / Axum HeaderMaps to support OpenTelemetry propagation.
pub struct HeaderMapInjector<'a> {
    pub headers: &'a mut HeaderMap,
}

impl<'a> Injector for HeaderMapInjector<'a> {
    fn set(&mut self, key: &str, value: String) {
        if let (Ok(name), Ok(val)) =
            (HeaderName::from_bytes(key.as_bytes()), HeaderValue::from_str(&value))
        {
            self.headers.insert(name, val);
        }
    }
}

/// Injects the current W3C traceparent context into outbound HTTP request headers.
pub fn inject_traceparent(headers: &mut HeaderMap) {
    global::get_text_map_propagator(|propagator| {
        propagator
            .inject_context(&opentelemetry::Context::current(), &mut HeaderMapInjector { headers });
    });
}

pub fn init() {
    tracing::info!(">> [TELEMETRY] Bridge initialised");
}

pub fn spawn(
    tx: broadcast::Sender<String>,
    monitor: Arc<Mutex<nexus_ide_backend::system_monitor::SystemMonitor>>,
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
