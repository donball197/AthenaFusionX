use std::process::Command;
use std::thread;
use std::time::Duration;

pub fn start_duet_listener() {
    println!(">> [SOVEREIGN] DUET LISTENER ACTIVE.");
    loop {
        // Simple Git Pull check for external commands
        let _ = Command::new("git").args(&["fetch", "origin"]).output();
        thread::sleep(Duration::from_secs(60));
    }
}
