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


// src/utils.rs - v4.0.4 (Vocal Cords Restored)
use anyhow::Result;
use chrono::Utc;
use std::io::{self, Write};
use tokio::time::{sleep, Duration};

pub fn log_resonance_time() -> Result<()> {
    let now = Utc::now();
    println!(">> [RESONANCE] Time logged: {}", now);
    Ok(())
}

/// Simulates the "Ghost in the Machine" readable typing effect
pub async fn typewriter_print(text: &str, delay_ms: u64) {
    for c in text.chars() {
        print!("{}", c);
        let _ = io::stdout().flush();
        // Skip delay for spaces/newlines to make it feel more organic
        if c != ' ' && c != '\n' {
            sleep(Duration::from_millis(delay_ms)).await;
        }
    }
    println!("\n");
}
