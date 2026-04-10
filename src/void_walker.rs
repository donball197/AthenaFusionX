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


use crate::agent_protocol::AgentAction;
use crate::component_runner::AgentHandle;
use anyhow::{anyhow, Result};
use regex::Regex;

/// VoidWalker: A Micro-Agent specialized in identifying logic entropy.
/// It hunts for unused variables and empty function stubs to maintain code purity.
pub struct VoidWalker;

impl VoidWalker {
    pub fn new() -> Self {
        Self
    }

    /// Scans the workspace for dead code using a combination of Clippy telemetry
    /// and pattern-based structural analysis via the provided AgentHandle.
    pub async fn hunt_dead_code(&self, handle: &AgentHandle) -> Result<String> {
        let mut report = String::from(">> [VOID_WALKER] Initiating Entropy Scan...\n");

        // 1. Detect Unused Variables via Clippy
        let clippy_cmd = "cargo clippy -- -W unused_variables -W unused_must_use 2>&1".to_string();
        let clippy_actions = vec![AgentAction::ExecuteCommand { command: clippy_cmd }];

        let clippy_results = handle
            .execute_actions(clippy_actions)
            .await
            .map_err(|e| anyhow!("CLIPP_EXEC_FAILURE: {}", e))?;

        let clippy_output = clippy_results.join("\n");
        let unused_re = Regex::new(r"warning: unused (variable|must_use) .* in (.*)").unwrap();

        let mut unused_found = false;
        for cap in unused_re.captures_iter(&clippy_output) {
            unused_found = true;
            report.push_str(&format!(
                "- [UNUSED_LOGIC] {} detected in {}\n",
                &cap[1],
                cap[2].trim()
            ));
        }

        if !unused_found {
            report.push_str("- [UNUSED_LOGIC] No unused variables detected.\n");
        }

        // 2. Detect Empty Functions via grep
        let empty_fn_cmd = "grep -rE 'fn [a-zA-Z0-9_]+\\(.*?\\) \\{\\s*\\}' src/".to_string();
        let empty_actions = vec![AgentAction::ExecuteCommand { command: empty_fn_cmd }];

        let empty_results = handle
            .execute_actions(empty_actions)
            .await
            .map_err(|e| anyhow!("EMPTY_FUNC_SCAN_FAILURE: {}", e))?;

        if !empty_results.is_empty() && !empty_results[0].trim().is_empty() {
            report.push_str("- [EMPTY_STUB] Found empty function definitions:\n");
            report.push_str(&empty_results[0]);
        } else {
            report.push_str("- [EMPTY_STUB] No empty function stubs detected.\n");
        }

        report.push_str(">> [VOID_WALKER] Scan Complete.\n");
        Ok(report)
    }
}
