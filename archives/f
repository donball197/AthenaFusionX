use prometheus::{Registry, Encoder, TextEncoder};
use std::time::Duration;
use tokio::time::sleep;
use std::sync::Arc;
use tokio::sync::Mutex;
use crate::auto_healer::HealerState;

pub async fn expose_metrics(state: Arc<Mutex<HealerState>>) {
    let registry = Registry::new();
    let healer_state_metric = prometheus::Gauge::with_name("healer_status").with_label("status");

    tokio::spawn(async move {
        loop {
            let healer_state = state.lock().await;
            let status_value = match healer_state.status {
                crate::auto_healer::DiagnosticStatus::Nominal => 1.0,
                crate::auto_healer::DiagnosticStatus::Degraded => 0.5,
                crate::auto_healer::DiagnosticStatus::Critical => 0.0,
            };
            healer_state_metric.set(status_value);

            let encoder = TextEncoder::new();
            let mut buffer = Vec::new();
            encoder.encode(&registry, &mut buffer).unwrap();

            println!("{}", String::from_utf8_lossy(&buffer));

            sleep(Duration::from_secs(5)).await;
        }
    });
}