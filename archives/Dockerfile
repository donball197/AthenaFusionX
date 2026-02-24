use std::time::Duration;
use tokio::time::sleep;
use std::sync::Arc;
use tokio::sync::Mutex;

mod server;
mod auto_healer;
mod drones;
mod hiram;

use drones::{DevOpsDrone, IndustrialDrone};
use hiram::Hiram;
use auto_healer::Diagnostic;

#[tokio::main]
async fn main() {
    let state = Arc::new(Mutex::new(Vec::new()));
    let hiram_arc = Arc::new(Mutex::new(Hiram::new()));
    let healer_state = Arc::new(Mutex::new(Diagnostic::Ok));

    // 4. AUTONOMOUS WATCHDOGS
    tokio::spawn(async move {
        loop {
            let diag = crate::auto_healer::AutoHealer::run_diagnostic().await;
            {
                let mut hiram = hiram_arc.lock().await;
                hiram.verify();
            }
            DevOpsDrone::run_background_check(".", "nexus-core");
            let _ = IndustrialDrone::auto_throttle().await;

            *healer_state.lock().await = diag;
            sleep(Duration::from_secs(10)).await;
        }
    });

    println!(">> [NEXUS] SYSTEMS GREEN. LISTENING ON PORT 3000.");
    server::start_server(state).await;
}