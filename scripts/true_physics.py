import os

# 1. Upgrade universal src/executor.rs
with open("src/executor.rs", "r") as f:
    exec_code = f.read()

exec_start = exec_code.find("pub async fn execute_handle(")
exec_end = exec_code.find("async fn execute_single_action(")

new_exec = r"""pub async fn execute_handle(
        &mut self,
        actions: Vec<AgentAction>,
        memory: Arc<Mutex<crate::memory::MemoryPalace>>,
        vmo_state: Arc<Mutex<VmoState>>,
        dist: Arc<DistributedScheduler>,
    ) -> Result<Vec<String>> {
        self.current_state = FsmState::Executing;
        let mut observations = Vec::new();
        let total = actions.len();

        for (idx, action) in actions.into_iter().enumerate() {
            let _ = self.tx.send(serde_json::json!({
                "type": "TASK_STATUS",
                "payload": format!("Executing → {:?}", action)
            }).to_string());

            let is_halt = matches!(action, AgentAction::Halt { .. });

            match self.execute_single_action(action, memory.clone(), vmo_state.clone(), dist.clone()).await {
                Ok(obs) => {
                    observations.push(obs.clone());
                    tracing::info!("Action observation: {}", obs);

                    // ⚡ TRUE ANTI-BATCHING PHYSICS
                    if obs.contains("[READ_OK]") || obs.contains("[CMD:") || obs.contains("[DIFF_PATCH") || obs.contains("[WRITE_OK]") {
                        let skipped = total.saturating_sub(idx + 1);
                        if skipped > 0 {
                            let yield_msg = format!(">> [YIELD] Action completed. Suspending batch. {} subsequent action(s) skipped.", skipped);
                            observations.push(yield_msg.clone());
                            tracing::info!("{}", yield_msg);
                        }
                        break;
                    }

                    if is_halt {
                        break;
                    }
                }
                Err(e) => {
                    let skipped = total.saturating_sub(idx + 1);
                    let err_obs = format!("ACTION_FAILED: {} ({} action(s) skipped)", e, skipped);
                    observations.push(err_obs.clone());
                    tracing::warn!("Action failed: {}", e);
                    self.current_state = FsmState::Fault;
                    return Ok(observations);
                }
            }
        }
        self.current_state = FsmState::Complete;
        Ok(observations)
    }

"""
if exec_start != -1 and exec_end != -1:
    with open("src/executor.rs", "w") as f:
        f.write(exec_code[:exec_start] + new_exec + exec_code[exec_end:])
    print("✅ universal src/executor.rs upgraded (True Anti-Batching)")
else:
    print("❌ Could not find boundaries in executor.rs")


# 2. Upgrade universal src/task_manager.rs
with open("src/task_manager.rs", "r") as f:
    tm_code = f.read()

tm_start = tm_code.find("                match executor.execute_handle(")
tm_end = tm_code.find("            Err(e) => {")

new_tm = r"""                match executor.execute_handle(
                    actions.clone(),
                    memory.clone(),
                    vmo_state.clone(),
                    dist.clone()
                ).await {
                    Ok(observations) => {
                        for obs in observations {
                            session_obs.push(obs.clone());
                            tracing::info!("{}", &obs[..obs.len().min(200)]);
                            
                            // ⚡ TRUE HALT DETECTION: Only halt if the executor actually ran it
                            if obs.starts_with("[HALT]") {
                                halted = true;
                            }
                        }
                    }
                    Err(e) => {
                        let err_msg = format!("EXECUTOR_FAILURE: {}", e);
                        session_obs.push(err_msg.clone());
                        tracing::warn!("{}", err_msg);
                    }
                }

                if halted {
                    tracing::info!("\x1b[32m>> [MISSION COMPLETE] Agent halted successfully.\x1b[0m");
                    break;
                }
            }
"""
if tm_start != -1 and tm_end != -1:
    with open("src/task_manager.rs", "w") as f:
        f.write(tm_code[:tm_start] + new_tm + tm_code[tm_end:])
    print("✅ universal src/task_manager.rs upgraded (Halt Detection)")
else:
    print("❌ Could not find boundaries in task_manager.rs")
