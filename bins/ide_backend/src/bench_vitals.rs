use std::time::Instant;
use ide_backend::system_monitor::{SystemMonitor, Vitals};

fn main() {
    let mut monitor = SystemMonitor::new();
    let n = 1000;
    let start = Instant::now();
    for _ in 0..n {
        let _ = monitor.get_vitals();
    }
    let duration = start.elapsed();
    println!("Average time per get_vitals: {:?}", duration / n);
}