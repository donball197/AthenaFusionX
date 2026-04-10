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
use anyhow::Context;

/// ZigAutofix: The interface for low-level ARM64 optimizations.
pub struct ZigAutofix;

impl ZigAutofix {
    /// align_memory_strands: Calls the Zig-compiled binary to purge fragmentation.
    pub fn align_memory_strands() -> anyhow::Result<String> {
        println!("\x1b[33m>> [ZIG] Initiating Hardware-Level Realignment...\x1b[0m");

        // Attempt to run the Zig realignment tool
        let output = Command::new("zig")
            .args(["run", "src/kernel/realignment.zig", "--optimize", "ReleaseFast"])
            .output()
            .context("Failed to execute Zig realignment binary")?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            let err = String::from_utf8_lossy(&output.stderr);
            Err(anyhow::anyhow!("Zig Realignment Failed: {}", err))
        }
    }
}
