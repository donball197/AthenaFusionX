// Janitor with improved thermal-aware heal loop
use crate::server::AppState;
use crate::thermal_governor::ThermalGovernor;
use std::sync::Arc;
use tokio::time::{sleep, Duration};

pub struct Janitor;

impl Janitor {
    pub async fn start_background_maintenance(state: Arc<AppState>) {
        tokio::spawn(async move {
            let governor = ThermalGovernor::new();
            loop {
                governor.apply_cooldown().await;
                let vitals = state.monitor.get_vitals();
                if vitals.cpu_load < 15.0 && governor.get_state() == crate::thermal_governor::CoolingState::Normal {
                    println!(">> [JANITOR] Low load + nominal temp — running maintenance...");
                }
                sleep(Duration::from_secs(60)).await;
            }
        });
    }
}
