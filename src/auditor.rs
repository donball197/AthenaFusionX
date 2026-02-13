use std::process::Command;
use std::fs;

pub fn validate_patch(temp_path: &str, code: &str) -> bool {
    println!(">> [AUDITOR] Commencing Evolutionary Check...");

    // Relaxed but Firm Schema Check
    let mandatory = ["struct Hiram", "impl Hiram", "fn new", "fn verify(&mut self)"];
    for token in mandatory {
        if !code.contains(token) {
            println!(">> [AUDITOR] Schema Breach! Missing: '{}'", token);
            return false;
        }
    }

    if let Err(e) = fs::write(temp_path, code) {
        eprintln!(">> [AUDITOR ERROR] Write failed: {}", e);
        return false;
    }

    let output = Command::new("cargo")
        .args(["check", "--color", "never"])
        .output();

    match output {
        Ok(out) => {
            if out.status.success() {
                println!(">> [AUDITOR] Logic is Plumb.");
                true
            } else {
                println!(">> [AUDITOR] Drift Detected in Syntax.");
                false
            }
        }
        Err(_) => false,
    }
}
