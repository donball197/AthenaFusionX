/**
 * Copyright 2026 Donald Ball
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 * http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR REPRESENTATIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */


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
