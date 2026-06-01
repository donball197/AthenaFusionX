use std::collections::HashMap;
use tracing::warn;

pub struct LogicEngine;

impl LogicEngine {
    pub fn linearize_handoff(
        action: &crate::agent_protocol::AgentAction,
        result: crate::agent_protocol::SubagentReturn,
    ) -> String {
        format!(
            "[SYSTEM: DELEGATION REPORT]\nAction: {:?}\nStatus: {}\nOutput: {}\n[DIRECTIVE]: Matriarch, resume execution based on this report.",
            action, result.status, result.output
        )
    }

    pub fn is_action_safe(directive: &str) -> bool {
        let forbidden =
            ["rm -rf /", "mkfs", "shutdown", "chmod 777", "dd if=/dev/zero", "> /dev/sda"];
        let d_lower = directive.to_lowercase();
        !forbidden.iter().any(|&p| d_lower.contains(p))
    }

    pub fn detect_deadlock(observations: &[String]) -> bool {
        if observations.len() < 3 {
            return false;
        }

        let mut read_counts: HashMap<String, usize> = HashMap::new();

        for obs in observations {
            if let Some(after) = obs.strip_prefix("[READ_OK] ") {
                let path = after.split_whitespace().next().unwrap_or("").to_string();
                if !path.is_empty() {
                    *read_counts.entry(path).or_insert(0) += 1;
                }
            }
        }

        for (path, count) in read_counts {
            if count >= 3 {
                let file_progress = observations.iter().any(|obs| {
                    (obs.contains("[WRITE_OK]")
                        || obs.contains("[DIFF_PATCH_OK]")
                        || obs.contains("[PATCH_OK]"))
                        && obs.contains(&path)
                });

                if !file_progress {
                    warn!(
                        "[LOGIC] Localized Deadlock: {} read {} times with zero progress.",
                        path, count
                    );
                    return true;
                }
            }
        }

        let mut last_cmd_sig = String::new();
        let mut cmd_repeat_count = 0;

        for obs in observations {
            if let Some(start) = obs.find("[CMD:") {
                let sig: String = obs[start..]
                    .lines()
                    .filter(|line| !line.contains("Finished") && !line.contains("Blocking waiting"))
                    .collect::<Vec<&str>>()
                    .join("\n");

                if sig == last_cmd_sig && !sig.is_empty() {
                    cmd_repeat_count += 1;
                } else {
                    last_cmd_sig = sig;
                    cmd_repeat_count = 1;
                }
            }
        }

        if cmd_repeat_count >= 2 {
            warn!("[LOGIC] Deadlock: Time-invariant command repetition detected.");
            return true;
        }
        false
    }
}
