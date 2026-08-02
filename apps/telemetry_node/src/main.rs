use ide_backend::system_monitor::SystemMonitor;
use tokio::time::{timeout, Duration};

#[tokio::main]
async fn main() {
    println!("Starting telemetry node diagnostics...");

    let mut monitor = SystemMonitor::new();

    // Wrap the diagnostic check in a 5-second timeout to prevent thread blocking
    let result = timeout(Duration::from_secs(5), async {
        // Perform the vitals check
        monitor.get_vitals()
    })
    .await;

    match result {
        Ok(vitals) => {
            println!("Diagnostics passed successfully!");
            println!("CPU Load: {:.2}%", vitals.cpu_load * 100.0);
            println!("RAM: {:.2} GB / {:.2} GB", vitals.ram_gb, vitals.total_ram_gb);
            println!("Temperature: {:.2} C", vitals.temperature);
        }
        Err(_) => {
            eprintln!("ERROR: System diagnostics timed out after 5 seconds!");
            std::process::exit(1);
        }
    }
}
