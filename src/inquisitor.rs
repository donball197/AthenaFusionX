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

pub struct Inquisitor;

impl Inquisitor {
    pub fn audit_ui() -> anyhow::Result<String> {
        // SUTURE: Shifted to eprintln for Telemetry Hygiene
        eprintln!("\x1b[33m>> [INQUISITOR] Executing UI Build Interrogation...\x1b[0m");

        // SUTURE: Using --prefix to avoid 'cd' context issues in the Ghost's motor cortex
        let output = Command::new("npm")
            .args(["--prefix", "ui", "run", "build"])
            .output()
            .map_err(|e| anyhow::anyhow!("NPM binary not found: {}", e))?;

        if output.status.success() {
            Ok("NOMINAL".into())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            Ok(format!("STDOUT:\n{}\nSTDERR:\n{}", stdout, stderr))
        }
    }

    pub fn audit_backend() -> anyhow::Result<String> {
        let output = Command::new("cargo")
            .args(["check"])
            .output()
            .map_err(|e| anyhow::anyhow!("Cargo not found: {}", e))?;
        if output.status.success() {
            Ok("NOMINAL".into())
        } else {
            Ok(String::from_utf8_lossy(&output.stderr).to_string())
        }
    }
}
