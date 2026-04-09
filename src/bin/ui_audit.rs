// src/bin/ui_audit.rs - Native UI Build Interrogator
fn main() {
    println!(">> [INQUISITOR] Initiating UI Build Audit...");
    match nexus_singularity::inquisitor::Inquisitor::audit_ui() {
        Ok(logs) => {
            println!("--- RAW VITE/SVELTE LOGS START ---");
            println!("{}", logs);
            println!("--- RAW VITE/SVELTE LOGS END ---");
        }
        Err(e) => {
            eprintln!("!! [FRACTURE] UI Audit failed: {}", e);
        }
    }
}
