use std::fs;

pub fn is_fit_for_work() -> bool {
    // Check Load Average
    if let Ok(load_str) = fs::read_to_string("/proc/loadavg") {
        let load: f32 = load_str.split_whitespace()
            .next()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0.0);

        // Throttle if load > 4.0
        if load > 4.0 {
            return false;
        }
    }
    true
}
