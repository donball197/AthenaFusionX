// src/task_manager.rs — v5.3 Observational Sovereignty
// FIX: session_obs capped at 20 entries (oldest pruned) to prevent
//      context window saturation on long missions.
// FIX: After WASM_SYNTHESIS_COMPLETE, injects genesis alias hint.
// FIX: Loop detection — same action type 3x in a row forces halt.
use crate::conductor::Conductor;
use crate::distributed_scheduler::DistributedScheduler;
use crate::executor::{FsmState, RoboticExecutor};
use crate::genesis_broker;
use crate::json_parser;
use crate::router::IntelligentRouter;
use serde_json::json;
use std::process::Command;
use std::sync::{Arc, LazyLock};
use tokio::sync::{broadcast, Mutex, Semaphore};

static TASK_SLAB: LazyLock<Arc<Semaphore>> = LazyLock::new(|| Arc::new(Semaphore::new(1)));

/// MAX_OBS: Maximum entries kept in session_obs.
/// Oldest entries are pruned to prevent context window saturation.
const MAX_OBS: usize = 20;

fn get_current_branch() -> String {
    Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .unwrap_or_else(|| "detached".into())
        .trim()
        .to_string()
}

pub async fn send_task(
    orch: Arc<crate::orchestrator::Orchestrator>,
    client: &reqwest::Client,
    api_key: &str,
    directive: &str,
    memory: Arc<Mutex<crate::memory::MemoryPalace>>,
    vmo_state: Arc<Mutex<crate::server::VmoState>>,
    tx: broadcast::Sender<String>,
    dist: Arc<DistributedScheduler>,
) -> String {
    let _permit = TASK_SLAB.clone().acquire_owned().await.expect("SLAB_LOCK_FAIL");
    let profile = IntelligentRouter::route(directive);
    let mut session_obs: Vec<String> = Vec::new();
    let mut fsm = RoboticExecutor::new();
    let mut last_action_type = String::new();
    let mut same_action_count = 0u8;

    for turn in 1..=profile.max_turns {
        if fsm.current_state == FsmState::Complete || fsm.current_state == FsmState::Fault {
            break;
        }

        let branch = get_current_branch();
        let prompt = IntelligentRouter::build_prompt(directive, &profile, &session_obs);

        let _ = tx.send(
            json!({
                "type": "NEURAL_THOUGHT",
                "payload": format!("── Turn {}/{} | Branch: {} ──", turn, profile.max_turns, branch)
            })
            .to_string(),
        );

        match Conductor::run_turn(
            &profile,
            prompt,
            turn as usize,
            &orch,
            client,
            api_key,
            Some(tx.clone()),
        )
        .await
        {
            Ok(response) => {
                match json_parser::extract_actions(&response) {
                    Ok(actions) if !actions.is_empty() => {
                        // ── Loop guard ────────────────────────────────
                        let action_type = format!("{:?}", actions[0]);
                        let type_tag = action_type.split(' ').next().unwrap_or("").to_string();
                        if type_tag == last_action_type {
                            same_action_count += 1;
                        } else {
                            same_action_count = 1;
                            last_action_type = type_tag.clone();
                        }
                        if same_action_count >= 3 {
                            session_obs.push(format!(
                                "LOOP_GUARD: Action '{}' repeated {} times. \
                                 Breaking loop — choose a different action or halt.",
                                type_tag, same_action_count
                            ));
                            // Inject genesis alias hint if we're stuck in synthesis loop
                            if type_tag.contains("SynthesizeWasm") && fsm.stored_genesis.is_some() {
                                let hint = fsm
                                    .last_func_hint
                                    .clone()
                                    .unwrap_or_else(|| "execute_cell".to_string());
                                genesis_broker::inject_alias(&mut session_obs, &hint);
                            }
                            fsm.transition(FsmState::Fault);
                            break;
                        }

                        match fsm
                            .execute_handle(
                                actions,
                                memory.clone(),
                                vmo_state.clone(),
                                dist.clone(),
                            )
                            .await
                        {
                            Ok(results) => {
                                let wasm_done =
                                    results.iter().any(|r| r.contains("WASM_SYNTHESIS_COMPLETE"));

                                for res in results {
                                    session_obs.push(res);
                                }

                                // After synthesis, inject alias hint so model
                                // knows to call wasm_execute_local next.
                                if wasm_done {
                                    let hint = fsm
                                        .last_func_hint
                                        .clone()
                                        .unwrap_or_else(|| "execute_cell".to_string());
                                    genesis_broker::inject_alias(&mut session_obs, &hint);
                                    same_action_count = 0; // reset loop counter
                                }

                                // Prune oldest entries to keep context lean
                                if session_obs.len() > MAX_OBS {
                                    let drain = session_obs.len() - MAX_OBS;
                                    session_obs.drain(0..drain);
                                }
                            }
                            Err(e) => session_obs.push(format!("Error: {}", e)),
                        }
                    }
                    _ => {
                        session_obs.push(
                            "System: Mission target likely achieved or clarification needed."
                                .into(),
                        );
                        break;
                    }
                }
            }
            Err(e) => {
                let _ = tx.send(
                    json!({
                        "type": "NEURAL_STREAM",
                        "payload": format!("API_FAULT: {}", e)
                    })
                    .to_string(),
                );
                break;
            }
        }
    }

    "TASK_COMPLETE".into()
}
