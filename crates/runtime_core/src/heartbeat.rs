use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};
use tokio::time::{sleep, Duration};
use crate::system_monitor::SystemMonitor;

pub struct Heartbeat;

impl Heartbeat {
    pub fn ignite(tx: broadcast::Sender<String>, monitor: Arc<Mutex<SystemMonitor>>) {
        tokio::spawn(async move {
            loop {
                sleep(Duration::from_secs(5)).await;
                
                let vitals = {
                    let m = monitor.lock().await;
                    m.get_vitals()
                };

                let pulse = serde_json::json!({
                    "type": "VITALS_UPDATE",
                    "payload": vitals
                });

                let _ = tx.send(pulse.to_string());
            }
        });
    }
}
