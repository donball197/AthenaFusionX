use crate::graph::{DagManager, TaskStatus};
use std::env;
use std::path::PathBuf;
use std::process::Command;

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

    fn execute_grounded_task(description: &str, _evidence_context: &str) -> Option<String> {
        let desc_lower = description.to_lowercase();

        // 1. Direct Physical Shell Execution for Compiler & Build Checks
        if desc_lower.contains("cargo check")
            || desc_lower.contains("compiler check")
            || desc_lower.contains("build dry-run")
        {
            println!(
                "  -> [PHYSICAL ENGINE] Executing real host command: `cargo check --workspace`"
            );
            let output = Command::new("cargo")
                .args(["check", "--workspace", "--color", "never"])
                .output()
                .ok()?;

            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            let combined = format!("STDOUT:\n{}\nSTDERR:\n{}", stdout, stderr);
            let status_code = output.status.code().unwrap_or(-1);
            return Some(format!("[CMD_OK] Exit Status: {}\n{}", status_code, combined.trim()));
        }

        // 2. Direct Physical Filesystem Scans
        if (desc_lower.contains("scan") || desc_lower.contains("list"))
            && desc_lower.contains("top-level")
        {
            println!("  -> [PHYSICAL ENGINE] Executing filesystem scan on workspace root");
            if let Ok(entries) = std::fs::read_dir(".") {
                let mut files = Vec::new();
                let mut dirs = Vec::new();
                for e in entries.flatten() {
                    let name = e.file_name().to_string_lossy().to_string();
                    if e.path().is_dir() {
                        dirs.push(name);
                    } else {
                        files.push(name);
                    }
                }
                dirs.sort();
                files.sort();
                let result =
                    format!("{{\n  \"directories\": {:?},\n  \"files\": {:?}\n}}", dirs, files);
                return Some(result);
            }
        }

        None
    }

    pub fn run_step(&mut self) -> Result<usize, String> {
        let ready_tasks = self.manager.get_ready_tasks();
        let mut executed_count = 0;

        if ready_tasks.is_empty() {
            return Ok(0);
        }

        let bin_path = Self::get_minion_binary_path();

        for task_id in ready_tasks {
            let (tier, description, deps) = {
                let node = self.manager.nodes.get_mut(&task_id).unwrap();
                node.status = TaskStatus::InProgress;
                (node.required_tier.clone(), node.description.clone(), node.dependencies.clone())
            };

            // Collect historical evidence from parent tasks
            let mut parent_evidence = String::new();
            for dep in &deps {
                if let Some(dep_node) = self.manager.nodes.get(dep) {
                    if !dep_node.result_evidence.is_empty() {
                        parent_evidence.push_str(&format!(
                            "--- [EVIDENCE FROM PREVIOUS STEP: {}] ---\n{}\n\n",
                            dep, dep_node.result_evidence
                        ));
                    }
                }
            }

            println!("\n==================================================");
            println!(
                "[Orchestrator] Dispatching Task: [{}] (Tier: {})",
                task_id,
                tier.to_uppercase()
            );
            println!("Objective: {}", description);
            if !parent_evidence.is_empty() {
                println!(
                    "[+] Injecting {} bytes of historical DAG evidence.",
                    parent_evidence.len()
                );
            }
            println!("==================================================");

            // Check if task can be physically executed directly on host
            let task_output = if let Some(physical_output) =
                Self::execute_grounded_task(&description, &parent_evidence)
            {
                println!("\n[+] Physical Result:\n{}", physical_output);
                Some(physical_output)
            } else {
                // Otherwise pipe prompt + parent evidence to the minion binary
                let full_prompt = if parent_evidence.is_empty() {
                    description.clone()
                } else {
                    format!(
                        "{}\n\nBACKGROUND CONTEXT (Results from previous steps):\n{}",
                        description, parent_evidence
                    )
                };

                let output =
                    Command::new(&bin_path).arg("--tier").arg(&tier).arg(&full_prompt).output();

                match output {
                    Ok(out) if out.status.success() => {
                        let text = String::from_utf8_lossy(&out.stdout).to_string();
                        print!("{}", text);
                        Some(text)
                    }
                    Ok(out) => {
                        let err = String::from_utf8_lossy(&out.stderr);
                        eprintln!(
                            "\n[Orchestrator] Task [{}] FAILED with code: {} | {}",
                            task_id, out.status, err
                        );
                        None
                    }
                    Err(e) => {
                        eprintln!("\n[Orchestrator] FATAL: Failed to execute minion daemon: {}", e);
                        None
                    }
                }
            };

            match task_output {
                Some(evidence) => {
                    println!("\n[Orchestrator] Task [{}] Completed Successfully.", task_id);
                    if let Some(node) = self.manager.nodes.get_mut(&task_id) {
                        node.status = TaskStatus::Completed;
                        node.result_evidence = evidence;
                    }
                    executed_count += 1;
                }
                None => {
                    if let Some(node) = self.manager.nodes.get_mut(&task_id) {
                        node.status = TaskStatus::Failed;
                    }
                }
            }
        }

        Ok(executed_count)
    }
}
