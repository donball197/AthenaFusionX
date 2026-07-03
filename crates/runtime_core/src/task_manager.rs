use crate::executor::RoboticExecutor;
use nexus_substrate::memory::MemoryPalace;
use crate::{
    agent_protocol::AgentAction, json_parser,
    metrics::METRICS, orchestrator::Orchestrator, 
    shared_state::VmoState, distributed_scheduler::DistributedScheduler,
    conductor::{Conductor, TurnContext}, router::IntelligentRouter,
    orchestrator_v2::planner::PlannerAgent
};
use reqwest::Client;
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};
use std::collections::HashMap;

pub struct TaskContext {
    pub client: Client,
    pub api_key: String,
    pub memory: Arc<Mutex<MemoryPalace>>,
    pub vmo_state: Arc<Mutex<VmoState>>,
    pub tx: broadcast::Sender<String>,
    pub dist: Arc<DistributedScheduler>,
}

pub async fn send_task(
    orchestrator: Arc<Orchestrator>,
    ctx: TaskContext,
    raw_directive: &str,
) -> String {
    METRICS.inc_task();
    let _ = ctx.tx.send(serde_json::json!({"type": "LOG_EVENT", "payload": "[EXECUTIVE CORTEX] Initiating DAG Decomposition..."}).to_string());

    let mut graph = match PlannerAgent::generate_plan(&ctx.client, &ctx.api_key, raw_directive).await {
        Ok(g) => g,
        Err(e) => return format!("FRACTURE: Planner failed to decompose objective: {}", e),
    };

    graph.compute_critical_path_priorities();
    let order = match graph.resolve_execution_order() {
        Ok(o) => o,
        Err(e) => return format!("FRACTURE: Graph validation failed: {}", e),
    };

    let plan_id = uuid::Uuid::new_v4().to_string();
    let _ = ctx.tx.send(serde_json::json!({"type": "LOG_EVENT", "payload": format!("[EXECUTIVE CORTEX] Plan {} persisted. Entering Scheduler loop.", plan_id)}).to_string());

    let mut executor = RoboticExecutor::new(ctx.api_key.clone(), ctx.tx.clone());
    let mut task_outputs: HashMap<String, String> = HashMap::new();
    
    for task_id in order {
        let node = graph.nodes.get(&task_id).unwrap().clone();
        let _ = ctx.tx.send(serde_json::json!({"type": "LOG_EVENT", "payload": format!("[SCHEDULER] Dispatching Task: {}\nvia {}", node.objective, node.required_agent)}).to_string());

        let mut task_history = String::new();
        
        for dep in &node.deps {
            if let Some(out) = task_outputs.get(dep) {
                task_history.push_str(&format!("---\n[PREVIOUS DAG TASK OUTPUT GATHERED FROM {}]\n{}\n---\n", dep, out));
            }
        }

        let mut task_resolved = false;
        let mut inner_turn = 0;
        let mut final_stdout = String::new();
        
        while !task_resolved && inner_turn < 6 {
            inner_turn += 1;
            
            let execution_prompt = format!(
                "CURRENT TASK OBJECTIVE: {}\n\
                ROLE: {}\n\
                \n\
                DAG CONTEXT & OBSERVATIONS:\n\
                {}\n\
                \n\
                CRITICAL DIRECTIVES:\n\
                1. Look at the DAG CONTEXT above. If the data you need was already gathered, USE IT.\n\
                2. If you have achieved the CURRENT TASK OBJECTIVE, you MUST output exactly: [{{\"type\": \"halt\", \"reasoning\": \"Objective complete\"}}]. Do not repeat commands.\n\
                3. If a validation command (like clippy or check) returns [CMD_OK] with empty output, IT PASSED. Immediately issue a halt! Do NOT run it again.\n\
                4. Output ONLY a valid JSON array matching the schema.",
                node.objective, node.required_agent, task_history
            );

            let turn_ctx = TurnContext { orch: &orchestrator, client: &ctx.client, api_key: &ctx.api_key, tx: Some(ctx.tx.clone()) };
            let profile = IntelligentRouter::route(&node.objective);
            
            match Conductor::run_turn(&profile, execution_prompt, inner_turn, turn_ctx, 0).await {
                Ok(response) => {
                    match json_parser::extract_actions(&response) {
                        Ok(actions) => {
                            if actions.iter().any(|a| matches!(a, AgentAction::Halt { .. })) {
                                let verdict = crate::critic::Critic::evaluate(&node.objective, &task_history, "").await.unwrap_or_default();
                                if !verdict.approved {
                                    let _ = ctx.tx.send(serde_json::json!({
                                        "type": "LOG_EVENT",
                                        "payload": format!("[CRITIC] Halt check rejected: {}", verdict.reason)
                                    }).to_string());
                                    task_history.push_str(&format!("[CRITIC_DENIAL]: {}\n", verdict.reason));
                                } else {
                                    task_resolved = true;
                                }
                            }
                            match executor.execute_handle(actions.clone(), ctx.memory.clone(), ctx.vmo_state.clone(), ctx.dist.clone()).await {
                                Ok(obs) => {
                                    let obs_str = obs.join("\n");
                                    task_history.push_str(&format!("[ACTION RESULTS]:\n{}\n", obs_str));
                                    final_stdout.push_str(&obs_str);
                                    final_stdout.push('\n');
                                },
                                Err(e) => { task_history.push_str(&format!("[ACTION FAILED]: {}\n", e)); }
                            }
                        }
                        Err(e) => {
                            let _ = ctx.tx.send(serde_json::json!({"type": "LOG_EVENT", "payload": format!("[FORMAT FRACTURE] Retrying turn {}", inner_turn)}).to_string());
                            task_history.push_str(&format!("[FORMAT_VIOLATION] You output invalid JSON. Error: {}. Fix your format and output ONLY raw JSON.\n", e));
                        }
                    }
                }
                Err(e) => { task_history.push_str(&format!("[NETWORK_FAULT] {}\n", e)); }
            }
        }
        task_outputs.insert(task_id.clone(), final_stdout);
    }

    format!("DAG Execution Complete. Plan ID: {}", plan_id)
}
