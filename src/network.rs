use std::process::Command;
use std::thread;
use std::time::Duration;

pub fn start_duet_listener() {
    println!(">> [SOVEREIGN] DUET NODE LISTENING...");
    
    loop {
        // 1. Check for updates from the Motorola
        let pull = Command::new("git").args(&["pull", "origin", "main"]).output();
        
        if let Ok(out) = pull {
            let status = String::from_utf8_lossy(&out.stdout);
            if !status.contains("Already up to date.") {
                println!(">> [SYNC] New DNA detected. Rebuilding...");
                
                // 2. Recompile automatically
                let _ = Command::new("cargo").arg("build").status();
                
                // 3. Signal health back to the Motorola
                let _ = Command::new("gh").args(&["issue", "comment", "1", "--body", ">> [DUET] Sync Complete. Operational."]).output();
            }
        }
        
        thread::sleep(Duration::from_secs(15));
    }
}
