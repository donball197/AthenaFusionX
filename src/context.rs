use std::process::Command;
use std::fs;

pub fn get_system_vitals() -> String {
    let ram = Command::new("free").arg("-m").output().map(|o| String::from_utf8_lossy(&o.stdout).to_string()).unwrap_or_default();
    let disk = Command::new("df").arg("-h").arg("/").output().map(|o| String::from_utf8_lossy(&o.stdout).to_string()).unwrap_or_default();
    let cargo = fs::read_to_string("Cargo.toml").unwrap_or_default();

    format!("--- HARDWARE LIMITS (MOTOROLA G PRIMARY) ---\n{}\n{}\n\n--- ENGINE LIMITS ---\n{}", ram, disk, cargo)
}
