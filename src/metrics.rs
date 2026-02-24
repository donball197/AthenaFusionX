use prometheus::{Registry, Encoder, TextEncoder};
use std::time::Duration;
use tokio::time::sleep;
use std::sync::Arc;
use tokio::sync::Mutex;
use crate::auto_healer::{HealerState, DiagnosticStatus};

pub async fn expose_metrics(state: Arc<Mutex<HealerState>>) {
    let registry = Registry::new();
    let healer_state_metric = prometheus::Gauge::with_name("healer_status").with_label("status");
    
    tokio::spawn(async move {
        loop {
            let status_value = {
                let healer_state = state.lock().await;
                match healer_state.overall_status {
                    DiagnosticStatus::Nominal => 1.0,
                    DiagnosticStatus::Degraded => 0.5,
                    DiagnosticStatus::Critical => 0.0,
                }
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
