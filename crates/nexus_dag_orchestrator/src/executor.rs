use crate::graph::{DagManager, TaskStatus};
use std::env;
use std::path::PathBuf;
use std::process::{Command, Stdio};

pub struct DagExecutor {
    pub manager: DagManager,
}

impl DagExecutor {
    pub fn new(manager: DagManager) -> Self {
        Self { manager }
    }

    fn get_minion_binary_path() -> PathBuf {
        let mut path =
            env::current_exe().unwrap_or_else(|_| PathBuf::from("target/debug/nexus_minion"));
        path.pop();

        let bin_path = path.join("nexus_minion");
        if bin_path.exists() {
            bin_path
        } else {
            PathBuf::from("target/debug/nexus_minion")
        }
    }

    pub fn run_step(&mut self) -> Result<usize, String> {
        let ready_tasks = self.manager.get_ready_tasks();
        let mut executed_count = 0;

        if ready_tasks.is_empty() {
            return Ok(0);
        }

        let bin_path = Self::get_minion_binary_path();

        for task_id in ready_tasks {
            // 1. Gather upstream evidence
            let mut context_block = String::new();
            {
                let node = self.manager.nodes.get(&task_id).unwrap();
                for dep_id in &node.dependencies {
                    if let Some(dep_node) = self.manager.nodes.get(dep_id) {
                        let out = if dep_node.result_evidence.len() > 4000 {
                            format!("{}... [TRUNCATED]", &dep_node.result_evidence[..4000])
                        } else {
                            dep_node.result_evidence.clone()
                        };
                        context_block.push_str(&format!(
                            "--- [EVIDENCE FROM PREVIOUS STEP: {}] ---\n{}\n\n",
                            dep_id, out
                        ));
                    }
                }
            }

            // 2. Mark in progress and build final payload
            let (tier, description) = {
                let node = self.manager.nodes.get_mut(&task_id).unwrap();
                node.status = TaskStatus::InProgress;
                (node.required_tier.clone(), node.description.clone())
            };

            let combined_mission = if context_block.is_empty() {
                description.clone()
            } else {
                format!("{}\n\nBACKGROUND CONTEXT (Results from previous steps):\n{}", description, context_block)
            };

            println!("\n==================================================");
            println!(
                "[Orchestrator] Dispatching Task: [{}] (Tier: {})",
                task_id,
                tier.to_uppercase()
            );
            println!("Objective: {}", description);
            if !context_block.is_empty() {
                println!("[+] Injecting {} bytes of historical DAG evidence.", context_block.len());
            }
            println!("==================================================");

            // 3. Spawn the minion, piping stdout so we can capture the final evidence
            let output = Command::new(&bin_path)
                .arg("--tier")
                .arg(&tier)
                .arg(&combined_mission)
                .stdout(Stdio::piped())
                .stderr(Stdio::inherit())
                .output();

            match output {
                Ok(out) if out.status.success() => {
                    let minion_stdout = String::from_utf8_lossy(&out.stdout).to_string();
                    print!("{}", minion_stdout); // Stream it back to the console

                    // Only persist the RESULT block as evidence -- not the full
                    // log stream -- so downstream tasks don't inherit exploding
                    // nested logs from every prior step.
                    let evidence = extract_result_block(&minion_stdout);

                    println!("\n[Orchestrator] Task [{}] Completed Successfully.", task_id);
                    self.manager.complete_task(&task_id, evidence);
                    executed_count += 1;
                }
                Ok(out) => {
                    let minion_stdout = String::from_utf8_lossy(&out.stdout).to_string();
                    print!("{}", minion_stdout);
                    eprintln!(
                        "\n[Orchestrator] Task [{}] FAILED with code: {}",
                        task_id, out.status
                    );
                    if let Some(node) = self.manager.nodes.get_mut(&task_id) {
                        node.status = TaskStatus::Failed;
                        node.result_evidence = minion_stdout;
                    }
                }
                Err(e) => {
                    eprintln!("\n[Orchestrator] FATAL: Failed to execute minion daemon: {}", e);
                    if let Some(node) = self.manager.nodes.get_mut(&task_id) {
                        node.status = TaskStatus::Failed;
                    }
                }
            }
        }

        Ok(executed_count)
    }
}

/// Extracts just the text between "---- RESULT ----" and "----------------"
/// markers from a minion's captured stdout. Falls back to the full text if
/// the markers aren't present (e.g. local-edge stress-test output that
/// doesn't go through the gateway).
fn extract_result_block(stdout: &str) -> String {
    const START: &str = "---- RESULT ----";
    const END: &str = "----------------";

    if let Some(start_idx) = stdout.find(START) {
        let after_start = &stdout[start_idx + START.len()..];
        if let Some(end_idx) = after_start.find(END) {
            return after_start[..end_idx].trim().to_string();
        }
    }
    stdout.trim().to_string()
}
