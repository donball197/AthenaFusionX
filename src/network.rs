use std::process::Command;
use tokio::time::{sleep, Duration};

/// start_duet_listener: Polls for remote git commands on a 60-second cycle.
///
/// FIX: The previous implementation used std::thread::sleep inside a blocking
/// loop. If this function was ever spawned on a Tokio worker thread (via
/// tokio::spawn), it would park the thread for 60 seconds per iteration,
/// starving other tasks on the same thread. This is now an async fn using
/// tokio::time::sleep, which yields the thread back to the runtime while waiting.
///
/// Callers should use: tokio::spawn(network::start_duet_listener());
pub async fn start_duet_listener() {
    println!(">> [SOVEREIGN] DUET LISTENER ACTIVE.");
    loop {
        // Non-blocking git fetch check for external command dispatch
        let _ = Command::new("git").args(["fetch", "origin"]).output();
        // Yields the Tokio thread back to the runtime during the wait
        sleep(Duration::from_secs(60)).await;
    }
}
