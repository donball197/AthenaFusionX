use crate::graph::{DagManager, TaskStatus};
use std::process::{Command, Stdio};
use std::path::PathBuf;
use std::env;

pub struct DagExecutor {
    pub manager: DagManager,
}

impl DagExecutor {
    pub fn new(manager: DagManager) -> Self {
        Self { manager }
    }

    fn get_minion_binary_path() -> PathBuf {
        let mut path = env::current_exe().unwrap_or_else(|_| PathBuf::from("target/debug/nexus_minion"));
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
            let (tier, description) = {
                let node = self.manager.nodes.get_mut(&task_id).unwrap();
                node.status = TaskStatus::InProgress;
                (node.required_tier.clone(), node.description.clone())
            };

            println!("\n==================================================");
            println!("[Orchestrator] Dispatching Task: [{}] (Tier: {})", task_id, tier.to_uppercase());
            println!("Objective: {}", description);
            println!("==================================================");

            // Spawn the minion with live stdout inheritance so ReAct sandbox logs stream in real-time
            let status = Command::new(&bin_path)
                .arg("--tier")
                .arg(&tier)
                .arg(&description)
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .status();

            match status {
                Ok(exit_status) if exit_status.success() => {
                    println!("\n[Orchestrator] Task [{}] Completed Successfully.", task_id);
                    self.manager.complete_task(&task_id);
                    executed_count += 1;
                }
                Ok(exit_status) => {
                    eprintln!("\n[Orchestrator] Task [{}] FAILED with code: {}", task_id, exit_status);
                    if let Some(node) = self.manager.nodes.get_mut(&task_id) {
                        node.status = TaskStatus::Failed;
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
