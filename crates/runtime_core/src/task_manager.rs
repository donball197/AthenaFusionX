use crate::{
    cortical_loop::{CortexOutcome, CorticalLoop},
    distributed_scheduler::DistributedScheduler,
    metrics::METRICS,
    orchestrator::Orchestrator,
    orchestrator_v2::planner::PlannerAgent,
    shared_state::VmoState,
    world_model::WorldModel,
};
use nexus_substrate::memory::MemoryPalace;
use reqwest::Client;
use std::collections::HashMap;
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
    _orchestrator: Arc<Orchestrator>,
    ctx: TaskContext,
    raw_directive: &str,
) -> String {
    METRICS.inc_task();
    let _ = ctx.tx.send(serde_json::json!({"type": "LOG_EVENT", "payload": "[EXECUTIVE CORTEX] Initiating DAG Decomposition..."}).to_string());

    // Phase 1: High-Level DAG Decomposition
    let mut graph =
        match PlannerAgent::generate_plan(&ctx.client, &ctx.api_key, raw_directive).await {
            Ok(g) => g,
            Err(e) => return format!("FRACTURE: Planner failed to decompose objective: {}", e),
        };

    graph.compute_critical_path_priorities();
    let order = match graph.resolve_execution_order() {
        Ok(o) => o,
        Err(e) => return format!("FRACTURE: Graph validation failed: {}", e),
    };

    let plan_id = uuid::Uuid::new_v4().to_string();
    let _ = ctx.tx.send(serde_json::json!({"type": "LOG_EVENT", "payload": format!("[EXECUTIVE CORTEX] Plan {} persisted. Fusing Graph of Thoughts Loop...", plan_id)}).to_string());

    let mut task_outputs: HashMap<String, String> = HashMap::new();
    let mut mission_had_failures = false;
    let world_model = Arc::new(WorldModel::new());

    // Phase 2: Topologically Schedule Sub-Tasks via Scoped GoT Loops
    for task_id in order {
        let node = graph.nodes.get(&task_id).unwrap().clone();
        let _ = ctx.tx.send(serde_json::json!({"type": "LOG_EVENT", "payload": format!("[SCHEDULER] Dispatching DAG Node: {}\nvia Subagent: {}", node.objective, node.required_agent)}).to_string());

        let mut task_context_history = String::new();
        for dep in &node.deps {
            if let Some(out) = task_outputs.get(dep) {
                task_context_history
                    .push_str(&format!("---\n[DEPENDENCY FOCUS FROM {}]:\n{}\n---\n", dep, out));
            }
        }

        // Firing structured context directly into the Epistemic Engine
        let combined_mission = format!(
            "OBJECTIVE: {}\nASSIGNED ROLE: {}\n\nLINEAGE CONTEXT:\n{}",
            node.objective, node.required_agent, task_context_history
        );

        // ⚡ THE HYBRID FUSION: Execute this individual DAG node using a full adversarial Cortical Loop!
        let mut loop_cortex = CorticalLoop::new(
            ctx.api_key.clone(),
            world_model.clone(),
            ctx.memory.clone(),
            ctx.vmo_state.clone(),
            ctx.dist.clone(),
            ctx.tx.clone(),
        );

        let _ = ctx.tx.send(serde_json::json!({"type": "LOG_EVENT", "payload": format!("[CORTEX] Igniting GoT Epistemic Loop for Node {}", task_id)}).to_string());

        let execution_evidence = match loop_cortex.ignite(&combined_mission).await {
            Ok(CortexOutcome::Proven { belief }) => {
                format!(
                    "[DAG NODE COMPLETE] Ancestor node '{}' concluded execution (belief {:.2}).",
                    node.objective, belief
                )
            }
            Ok(CortexOutcome::Halted { reasoning }) => {
                format!(
                    "[DAG NODE HALTED] Ancestor node '{}' self-halted: {}",
                    node.objective, reasoning
                )
            }
            Ok(CortexOutcome::TurnsExhausted { best_belief }) => {
                mission_had_failures = true;
                let _ = ctx.tx.send(serde_json::json!({"type": "LOG_EVENT", "payload": format!("[CORTEX INCOMPLETE] Node {} exhausted turns without proving strategy (best belief {:.2}).", task_id, best_belief)}).to_string());
                format!("[DAG NODE INCOMPLETE] Ancestor node '{}' did NOT conclude successfully — best belief {:.2}. Treat this dependency as unresolved.", node.objective, best_belief)
            }
            Err(e) => {
                mission_had_failures = true;
                let _ = ctx.tx.send(serde_json::json!({"type": "LOG_EVENT", "payload": format!("[CORTEX FRACTURE] Node {} loop failed: {}", task_id, e)}).to_string());
                format!("[DAG NODE FAILED] Ancestor node '{}' failed: {}", node.objective, e)
            }
        };

        task_outputs.insert(task_id.clone(), execution_evidence);
    }

    if mission_had_failures {
        format!("DAG/GoT Execution FINISHED WITH INCOMPLETE NODES. Plan ID: {}", plan_id)
    } else {
        format!("DAG/GoT Hybrid Execution Complete. Plan ID: {}", plan_id)
    }
}
