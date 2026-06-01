use nexus_substrate::memory::MemoryPalace;
use crate::{
    agent_protocol::AgentAction, json_parser,
    metrics::METRICS,
    orchestrator::Orchestrator, 
    shared_state::VmoState, 
    distributed_scheduler::DistributedScheduler,
    executor::RoboticExecutor,
    conductor::{Conductor, TurnContext}, router::IntelligentRouter,
    orchestrator_v2::{
        planner::PlannerAgent,
        task_graph::TaskStatus,
        scheduler::Scheduler,
        verifier::Verifier
    }
};
use reqwest::Client;
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};

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

    if let Err(e) = graph.resolve_execution_order() {
        return format!("FRACTURE: Graph validation failed: {}", e);
    }

    let plan_id = uuid::Uuid::new_v4().to_string();
    let ts = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs() as i64;

    {
        let mem = ctx.memory.lock().await;
        let graph_json = serde_json::to_string(&graph).unwrap_or_default();
        let _ = mem.upsert_plan_run(&plan_id, raw_directive, &graph_json, "Active", ts).await;
    }

    let _ = ctx.tx.send(serde_json::json!({"type": "LOG_EVENT", "payload": format!("[EXECUTIVE CORTEX] Plan {} persisted. Entering Scheduler loop.", plan_id)}).to_string());

    let mut executor = RoboticExecutor::new(ctx.api_key.clone(), ctx.tx.clone());
    let mut loop_counter = 0;
    
    loop {
        loop_counter += 1;
        if loop_counter > 50 { return format!("MISSION SUSPENDED: Hard loop limit reached."); }

        let ready_tasks = Scheduler::prepare_ready_tasks(&mut graph);
        
        if ready_tasks.is_empty() {
            let all_done = graph.nodes.values().all(|n| n.status == TaskStatus::Completed);
            if all_done { break; }
            
            let has_escalated = graph.nodes.values().any(|n| n.status == TaskStatus::Escalated || n.status == TaskStatus::Failed);
            if has_escalated {
                return format!("MISSION SUSPENDED: DAG blocked by Escalated/Failed tasks.");
            }
            return format!("FRACTURE: DAG Deadlock detected.");
        }

        for task_id in ready_tasks {
            Scheduler::mark_running(&mut graph, &task_id);
            let node = graph.nodes.get(&task_id).unwrap().clone();
            
            let _ = ctx.tx.send(serde_json::json!({"type": "LOG_EVENT", "payload": format!("[SCHEDULER] Dispatching Task: {} via {}", node.objective, node.required_agent)}).to_string());

            let mut task_history = String::new();
            let mut task_resolved = false;
            let mut inner_turn = 0;
            let mut final_stdout = String::new();
            let mut final_stderr = String::new();

            // ⚡ THE INNER REACT LOOP: Allow the agent to iterate on the problem
            while !task_resolved && inner_turn < 20 {
                inner_turn += 1;
                
                let execution_prompt = format!(
                    "DIRECTIVE: Execute the following task objective.\n\
                    OBJECTIVE: {}\n\
                    ROLE: {}\n\
                    PREVIOUS ACTIONS AND OBSERVATIONS:\n{}\n\
                    If you have fully achieved the objective, you MUST include a {{\"type\": \"halt\", \"reasoning\": \"<summary>\"}} action.\n\
                    Otherwise, output the next logical AgentAction to progress (e.g. read the files you just found).\n\
                    You MUST output ONLY a valid JSON array of AgentAction objects.",
                    node.objective, node.required_agent, task_history
                );

                let turn_ctx = TurnContext {
                    orch: &orchestrator,
                    client: &ctx.client,
                    api_key: &ctx.api_key,
                    tx: Some(ctx.tx.clone()),
                };

                let profile = IntelligentRouter::route(&node.objective);
                let mut stdout_comb = String::new();
                let mut stderr_comb = String::new();

                match Conductor::run_turn(&profile, execution_prompt, 4, turn_ctx, node.execution_state.attempts as usize).await {
                    Ok(response) => {
                        let _ = ctx.tx.send(serde_json::json!({"type": "LOG_EVENT", "payload": format!(">> [AGENT RAW OUTPUT Turn {}]:\n{}", inner_turn, response)}).to_string());
                        
                        match json_parser::extract_actions(&response) {
                            Ok(actions) => {
                                if actions.iter().any(|a| matches!(a, AgentAction::Halt { .. })) {
                                    task_resolved = true;
                                    let _ = ctx.tx.send(serde_json::json!({"type": "LOG_EVENT", "payload": ">> [AGENT HALTED]: Objective achieved."}).to_string());
                                }

                                match executor.execute_handle(actions.clone(), ctx.memory.clone(), ctx.vmo_state.clone(), ctx.dist.clone()).await {
                                    Ok(obs) => {
                                        stdout_comb = obs.join("\n");
                                        let _ = ctx.tx.send(serde_json::json!({"type": "LOG_EVENT", "payload": format!(">> [EXECUTION RESULTS]:\n{}", stdout_comb)}).to_string());
                                    },
                                    Err(e) => stderr_comb = e.to_string(),
                                }
                                
                                task_history.push_str(&format!("---\nTURN {}\nACTIONS: {:?}\nSTDOUT:\n{}\nSTDERR:\n{}\n", inner_turn, actions, stdout_comb, stderr_comb));
                                final_stdout.push_str(&stdout_comb);
                                final_stderr.push_str(&stderr_comb);
                            }
                            Err(e) => {
                                stderr_comb = format!("JSON Parse Fracture: {}", e);
                                task_history.push_str(&format!("---\nTURN {}\nSYSTEM ERROR: {}\n", inner_turn, stderr_comb));
                            }
                        }
                    }
                    Err(e) => {
                        stderr_comb = format!("LLM Network Fracture: {}", e);
                        task_history.push_str(&format!("---\nTURN {}\nSYSTEM ERROR: {}\n", inner_turn, stderr_comb));
                    }
                }

                if !stderr_comb.is_empty() {
                    let _ = ctx.tx.send(serde_json::json!({"type": "LOG_EVENT", "payload": format!(">> [TASK FAULT]:\n{}", stderr_comb)}).to_string());
                }
                
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            }

            let result = Verifier::evaluate(&final_stdout, &final_stderr, &node.success_criteria).await;
            Scheduler::process_verification(&mut graph, &task_id, result);
        }

        {
            let mem = ctx.memory.lock().await;
            let graph_json = serde_json::to_string(&graph).unwrap_or_default();
            let _ = mem.upsert_plan_run(&plan_id, raw_directive, &graph_json, "Active", ts).await;
        }
    }

    {
        let mem = ctx.memory.lock().await;
        let graph_json = serde_json::to_string(&graph).unwrap_or_default();
        let _ = mem.upsert_plan_run(&plan_id, raw_directive, &graph_json, "Completed", ts).await;
    }

    format!("DAG Execution Complete. Plan ID: {}", plan_id)
}
