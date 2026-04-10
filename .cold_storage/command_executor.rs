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


use crate::system_monitor::SystemMonitor;
use std::process::Command;

pub async fn execute_allowed_command(
    _m: &SystemMonitor,
    cmd: &str,
) -> Result<serde_json::Value, String> {
    let allowed = ["cargo", "npm", "ls", "echo", "grep", "find"];
    let parts: Vec<&str> = cmd.split_whitespace().collect();

    if parts.is_empty() {
        return Err("Empty command".into());
    }

    if !allowed.contains(&parts[0]) {
        return Err(format!("SECURITY: Command '{}' is not in the Sovereign Whitelist.", parts[0]));
    }

    let output = Command::new(parts[0])
        .args(&parts[1..])
        .output()
        .map_err(|e| format!("IO Error: {}", e))?;

    Ok(serde_json::json!({
        "success": output.status.success(),
        "stdout": String::from_utf8_lossy(&output.stdout),
        "stderr": String::from_utf8_lossy(&output.stderr)
    }))
}
