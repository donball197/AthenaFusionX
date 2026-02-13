use std::process::Command;

pub fn check_storage_safety() -> bool {
    let output = Command::new("df")
        .arg("/sdcard")
        .output()
        .expect("Failed to execute df");

    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        if line.contains("/sdcard") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            // Typically the percentage is the 5th column or contains '%'
            if let Some(pct_str) = parts.iter().find(|p| p.contains('%')) {
                let pct: u32 = pct_str.replace('%', "").parse().unwrap_or(0);
                println!(">> [JANITOR] Storage utilization at {}%", pct);
                if pct > 90 { return false; }
            }
        }
    }
    true
}
