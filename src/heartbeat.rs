use std::process::Command;
use chrono::{DateTime, Utc, Duration};

pub fn check_cluster_sync() -> bool {
    let output = Command::new("gh")
        .args(&["issue", "view", "1", "--json", "comments", "-q", ".comments[-1].createdAt"])
        .output();

    if let Ok(out) = output {
        let timestamp_str = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if let Ok(last_heartbeat) = timestamp_str.parse::<DateTime<Utc>>() {
            let drift = Utc::now() - last_heartbeat;
            return drift < Duration::try_minutes(5).unwrap();
        }
    }
    false
}
