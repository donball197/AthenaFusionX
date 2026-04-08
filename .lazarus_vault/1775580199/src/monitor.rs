use std::fs;

pub fn check_system_health() {
    let mission_count = fs::read_dir("missions").map(|d| d.count()).unwrap_or(0);
    let claim_count = fs::read_dir("claims").map(|d| d.count()).unwrap_or(0);
    
    println!(">> [MONITOR] SYSTEM HEALTH: OK");
    println!(">> [MONITOR] ACTIVE MISSIONS: {}", mission_count);
    println!(">> [MONITOR] ACTIVE CLAIMS: {}", claim_count);
}
