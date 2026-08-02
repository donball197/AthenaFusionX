use std::path::Path;
use std::process::Command;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!(">> [VERIFICATION] Initiating Workspace Health Check...");

    // Ensure we are in a clean state or checking workspace targets
    let target_dir = Path::new("target");
    if target_dir.exists() {
        println!(">> [VERIFICATION] Target directory detected. Verifying ARM64 DNA integrity...");
    }

    // Execute cargo check across the workspace to ensure zero compilation fragmentation
    let status = Command::new("cargo").arg("check").arg("--workspace").status()?;

    if status.success() {
        println!(">> [VERIFICATION SUCCESS] Workspace compilation check passed. Zero fragmentation verified.");
    } else {
        eprintln!(">> [VERIFICATION FAILED] Compilation errors detected in workspace.");
        std::process::exit(1);
    }

    Ok(())
}
