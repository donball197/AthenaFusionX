// src/task_manager.rs — v4.1 SOVEREIGN CORTEX
// Copyright 2026 Donald Ball — Apache 2.0
//
// FIX: Removed unused imports `AgentAction` and `LogicEngine` — these
//      generated compiler warnings that polluted the build output and
//      masked real errors.
//
// UPGRADE: Model escalation flows via ctx.model_override (set by
//          AgentFactory) rather than embedded strings in system_prompt.
//
// UPGRADE: Max turns 12 → 16. Hard stop on > 5 consecutive API errors.
//          THOUGHT blocks extracted and broadcast live to UI.
//          Every turn result persisted to MemoryPalace.
//
// MODEL LADDER REMINDER:
//   Turns 1-5, 0-1 errors → gemma-4-26b-a4b-it (MoE primary)
//   Any turn,   errors > 1 → gemma-4-31b-it     (dense fallback)
//   Turn >= 8 OR errors > 3 → gemini-2.5-pro    (emergency only)

use std::sync::{Arc, LazyLock};
use tokio::sync::{broadcast, Semaphore, Mutex};
use crate::json_parser;
use crate::executor::{RoboticExecutor, FsmState};
use crate::factory::AgentFactory;
use crate::neural_agent::ContextPacket;
use serde_json::json;

/// Global concurrency slab — max 2 parallel tasks on 4GB metal.
static TASK_SLAB: LazyLock<Arc<Semaphore>> =
    LazyLock::new(|| Arc::new(Semaphore::new(2)));

pub async fn send_task(
    orch: Arc<crate::orchestrator::Orchestrator>,
    client: &reqwest::Client,
    api_key: &str,
    directive: &str,
    memory: Arc<Mutex<crate::memory::MemoryPalace>>,
    vmo_state: Arc<Mutex<crate::server::VmoState>>,
    tx: broadcast::Sender<String>,
) -> String {
    let _permit = TASK_SLAB
        .clone()
        .acquire_owned()
        .await
        .expect("SLAB_LOCK_FAIL");

    let mut session_obs = String::from("SENSORS_INIT: Substrate Ready.\n");
    let mut fsm = RoboticExecutor::new();
    let mut error_count: usize = 0;

    // ── DevOps System Persona ────────────────────────────────────────────────
    // Injected once per task — not per turn — to save context window tokens.
    let system_persona = r#"You are NEXUS DEVOPS — an autonomous DevOps agent on ARM64.
You have a 256K token context window. Use it fully when needed.

OUTPUT PROTOCOL (STRICT — violations cause escalation to gemma-4-31b-it):
- Reasoning:  ONLY inside <THOUGHT></THOUGHT> (max 3 short sentences).
- Actions:    ONLY inside <ACTION></ACTION> as a valid JSON array.
- Never emit raw shell commands or markdown outside these tags.
- Signal mission complete with: [{"type": "halt"}]

AVAILABLE ACTIONS:
  {"type": "git_action", "command": "status|commit|branch", "message": "..."}
  {"type": "execute_command", "command": "cargo check | cargo test | ls -la | ..."}
  {"type": "write_file", "path": "src/module.rs", "content": "..."}
  {"type": "lazarus_snapshot", "path": "src/", "tag": "backup_tag"}
  {"type": "vmo_create"}
  {"type": "vmo_write", "handle": "0xNN", "data": "..."}
  {"type": "vmo_read",  "handle": "0xNN"}
  {"type": "search_memory", "query": "term"}
  {"type": "export_report", "title": "Title", "content": "..."}
  {"type": "halt"}

FACTORY PROTOCOL:
1. Snapshot with lazarus_snapshot before modifying any source file.
2. After every write_file, emit execute_command: cargo check to verify.
3. Create a git branch before new features — never commit to main.
4. If cargo check fails, read the stderr, fix the file, re-verify.
5. Emit halt only when the directive is 100% satisfied."#;

    for turn in 1_u8..=16 {
        if fsm.current_state == FsmState::Complete
            || fsm.current_state == FsmState::Fault
        {
            break;
        }

        let model = AgentFactory::get_model_for_turn(turn as usize, error_count);

        // Broadcast turn metadata to UI
        let _ = tx.send(json!({
            "type": "NEURAL_THOUGHT",
            "payload": format!(
                "── Turn {}/16 | {} | errors={} ──",
                turn, model, error_count
            )
        }).to_string());

        let prompt = format!(
            "DIRECTIVE: {}\n\nOBSERVATIONS SO FAR:\n{}\n\nProduce <THOUGHT> then <ACTION> array:",
            directive, session_obs
        );

        let ctx = ContextPacket {
            system_prompt: Some(system_persona.to_string()),
            // Clean escalation — no string injection, just the model string
            model_override: Some(model.to_string()),
            recent_logs: session_obs
                .lines()
                .rev()
                .take(12)
                .map(|s| s.to_string())
                .collect(),
            ..Default::default()
        };

        match orch.run(prompt, ctx, client, api_key, Some(tx.clone())).await {
            Ok(response) => {
                // Extract THOUGHT and broadcast for UI transparency
                if let (Some(ts), Some(te)) = (
                    response.find("<THOUGHT>"),
                    response.find("</THOUGHT>"),
                ) {
                    let thought = response[ts + 9..te].trim();
                    let _ = tx.send(
                        json!({"type": "NEURAL_THOUGHT", "payload": thought}).to_string(),
                    );
                }

                match json_parser::extract_actions(&response) {
                    Ok(actions) if !actions.is_empty() => {
                        match fsm
                            .execute_handle(actions, memory.clone(), vmo_state.clone())
                            .await
                        {
                            Ok(results) => {
                                let combined = results.join("\n");
                                session_obs.push_str(&format!(
                                    "TURN_{}_RESULT: {}\n",
                                    turn, combined
                                ));
                                error_count = 0;

                                // Persist to MemoryPalace for cross-session recall
                                let palace = memory.lock().await;
                                let _ = palace
                                    .store_memory("TASK_MANAGER", directive, &combined)
                                    .await;
                            }
                            Err(e) => {
                                error_count += 1;
                                session_obs.push_str(&format!(
                                    "TURN_{}_EXEC_ERROR: {}\n",
                                    turn, e
                                ));
                            }
                        }
                    }
                    Ok(_) => {
                        // Empty action array — model produced no actions
                        let _ = tx.send(json!({
                            "type": "NEURAL_STREAM",
                            "payload": "\n\x1b[33m>> [CORTEX] Empty <ACTION> array — retrying with dense model...\x1b[0m\n"
                        }).to_string());
                        session_obs.push_str(&format!(
                            "TURN_{}_SYSTEM: Empty <ACTION> array received. \
                             You MUST provide at least one action or halt.\n",
                            turn
                        ));
                        error_count += 1;
                    }
                    Err(_) => {
                        // Parse failed — malformed JSON or missing tags
                        let _ = tx.send(json!({
                            "type": "NEURAL_STREAM",
                            "payload": "\n\x1b[33m>> [CORTEX] <ACTION> parse failed — escalating to dense model...\x1b[0m\n"
                        }).to_string());
                        session_obs.push_str(&format!(
                            "TURN_{}_SYSTEM: <ACTION> block missing or contains invalid JSON. \
                             Wrap your action array in <ACTION>[...]</ACTION> exactly.\n",
                            turn
                        ));
                        error_count += 1;
                    }
                }
            }
            Err(e) => {
                let _ = tx.send(json!({
                    "type": "NEURAL_STREAM",
                    "payload": format!("\n\x1b[31m>> [API_ERROR]: {}\x1b[0m\n", e)
                }).to_string());
                session_obs.push_str(&format!("TURN_{}_API_ERROR: {}\n", turn, e));
                error_count += 1;

                // Hard stop — API is down, key invalid, or rate limited
                if error_count > 5 {
                    let _ = tx.send(json!({
                        "type": "NEURAL_STREAM",
                        "payload": "\n\x1b[31m>> [CORTEX] 5 consecutive API errors — aborting task.\x1b[0m\n"
                    }).to_string());
                    break;
                }
            }
        }
    }

    let status = if fsm.current_state == FsmState::Complete {
        "TASK_COMPLETE"
    } else {
        "TASK_MAX_TURNS_REACHED"
    };

    let _ = tx.send(json!({"type": "TASK_STATUS", "payload": status}).to_string());
    status.into()
}
