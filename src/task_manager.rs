/**
 * Project Nexus Singularity - v2.1.0 (Array-Protocol Hardening)
 */
use std::sync::{Arc, LazyLock};
use tokio::sync::{broadcast, Semaphore, Mutex};
use crate::agent_protocol::AgentAction;
use crate::{executor, json_parser};

static TASK_SLAB: LazyLock<Arc<Semaphore>> = LazyLock::new(|| Arc::new(Semaphore::new(2)));

pub async fn send_task(
    agent: Arc<crate::neural_agent::MicroAgent>,
    client: &reqwest::Client,
    api_key: &str,
    directive: &str,
    memory: Arc<Mutex<crate::memory::MemoryPalace>>,
    tx: broadcast::Sender<String>,
) -> String {
    let _permit = TASK_SLAB.clone().acquire_owned().await.expect("SLAB_LOCK_FAIL");
    let mut session_obs = String::from("SENSORS_INIT: Substrate Ready.\n");
    
    let system_prompt = r#"
IDENTITY: Ghost of NEXUS. 
ROLE: Kernel-level orchestrator.
PROTOCOL: Reasoning in <THOUGHT>, Actions in <ACTION> (JSON).

[STRICT JSON SCHEMA]
You must output a SINGLE JSON ARRAY inside a SINGLE <ACTION> block.
Example: <ACTION> [{"type": "execute_command", "command": "ls"}, {"type": "halt"}] </ACTION>

1. Shell: {"type": "execute_command", "command": "cmd"}
2. Scribe: {"type": "write_file", "path": "path", "content": "text"}
3. Halt: {"type": "halt"} (MANDATORY at end of array)
"#;

    for _turn in 1..=3 {
        let prompt = format!("DIRECTIVE: {}\n\nOBSERVATIONS:\n{}\n\nExecute:", directive, session_obs);
        let ctx = crate::neural_agent::ContextPacket { system_prompt: Some(system_prompt.into()), ..Default::default() };

        if let Ok((response, _)) = agent.prepare_and_run(client, api_key, &prompt, ctx, 0.7, None).await {
            let _ = tx.send(response.clone());

            if let Ok(actions) = json_parser::extract_actions(&response) {
                match executor::Executor::execute_handle(actions.clone(), memory.clone()).await {
                    Ok(results) => {
                        for res in &results { println!("\x1b[1;33m>> [SHELL] {}\x1b[0m", res); }
                        session_obs.push_str(&format!("RESULT: {}\n", results.join("\n")));
                    },
                    Err(e) => {
                        println!("\x1b[1;31m>> [ERROR] {}\x1b[0m", e);
                        session_obs.push_str(&format!("ERROR: {}\n", e));
                    }
                }
                if actions.iter().any(|a| matches!(a, AgentAction::Halt)) { break; }
            }
        }
    }
    "TASK_SEQUENCE_COMPLETE".into()
}
