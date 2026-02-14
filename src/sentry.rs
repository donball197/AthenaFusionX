use std::fs;

pub fn is_fit_for_work() -> bool {
    // Check System Load (Works on nearly all Unix/Linux/Android systems)
    // /proc/loadavg tells us how hard the CPU is working
    if let Ok(load_str) = fs::read_to_string("/proc/loadavg") {
        let load: f32 = load_str.split_whitespace()
            .next()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0.0);

        // If the 1-minute load average is too high (e.g., > 4.0), 
        // the device is likely overheating or overworked.
        if load > 4.0 {
            return false;
        }
    }

    // You can also add a "Time Sentry" here.
    // e.g., only work during certain hours to save power at night.
    true
}
