use crate::cognition::belief_engine::BeliefEngine;
use crate::cognition::prediction_engine::PredictionEngine;
use crate::cognition::reflection_engine::ReflectionEngine;
use crate::cognition::traits::{MissionOutcome, Reflector};
use crate::executive_cortex::{ExecutiveCommand, ExecutiveCortex};
use crate::memory_cortex::MemoryCortex;
use crate::{
    cortical_loop::{CortexOutcome, CorticalLoop},
    distributed_scheduler::DistributedScheduler,
    metrics::METRICS,
    orchestrator::Orchestrator,
    orchestrator_v2::planner::PlannerAgent,
    shared_state::VmoState,
    world_model::WorldModel,
};
use reqwest::Client;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};

pub struct TaskContext {
    pub client: Client,
    pub api_key: String,
    pub memory: Arc<MemoryCortex>,
    pub vmo_state: Arc<Mutex<VmoState>>,
    pub tx: broadcast::Sender<String>,
    pub dist: Arc<DistributedScheduler>,
    pub belief_engine: Arc<BeliefEngine>,
    pub prediction_engine: Arc<PredictionEngine>,
    pub reflection_engine: Arc<ReflectionEngine>,
}

pub async fn send_task(
    _orchestrator: Arc<Orchestrator>,
    ctx: TaskContext,
    raw_directive: &str,
) -> String {
    METRICS.inc_task();

    // ⚡ 1. EXECUTIVE CORTEX GOAL INJECTION
    let (executive_cortex, command_tx) = ExecutiveCortex::new(ctx.memory.clone());
    let goal_id = uuid::Uuid::new_v4();
    let _ = command_tx
        .send(ExecutiveCommand::InjectGoal {
            objective: raw_directive.to_string(),
            parent_id: None,
        })
        .await;

    tokio::spawn(executive_cortex.run_actor_loop());

    let _ = ctx.tx.send(serde_json::json!({"type": "LOG_EVENT", "payload": format!("[EXECUTIVE CORTEX] Registered goal {}. Decomposing DAG...", goal_id)}).to_string());

    let mut graph =
        match PlannerAgent::generate_plan(&ctx.client, &ctx.api_key, raw_directive).await {
            Ok(g) => g,
            Err(e) => return format!("FRACTURE: Planner failed to decompose objective: {}", e),
        };

    if let Some(expected_path) = crate::completion_engine::SubgoalCompletionEngine::infer_contract("ad_hoc", raw_directive)
        .required_artifact_paths
        .first()
    {
        let has_write_node = graph.nodes.values()
            .any(|n| n.objective.contains(&expected_path.path));
        if !has_write_node {
            return format!(
                "FRACTURE: Directive requires writing '{}' but Planner produced no node for it. Refusing to run an incomplete DAG.",
                expected_path.path
            );
        }
    }

    graph.compute_critical_path_priorities();
    let order = match graph.resolve_execution_order() {
        Ok(o) => o,
        Err(e) => return format!("FRACTURE: Graph validation failed: {}", e),
    };

    let plan_id = uuid::Uuid::new_v4().to_string();
    let _ = ctx.tx.send(serde_json::json!({"type": "LOG_EVENT", "payload": format!("[EXECUTIVE CORTEX] Plan {} persisted.", plan_id)}).to_string());

    let mut task_outputs: HashMap<String, String> = HashMap::new();
    let mut mission_had_failures = false;
    let world_model = Arc::new(WorldModel::new());

    for task_id in order {
        let node = graph.nodes.get(&task_id).unwrap().clone();
        let _ = ctx.tx.send(serde_json::json!({"type": "LOG_EVENT", "payload": format!("[SCHEDULER] Dispatching DAG Node: {}\nvia Subagent: {}", node.objective, node.required_agent)}).to_string());

        let mut task_context_history = String::new();
        for dep in &node.deps {
            if let Some(out) = task_outputs.get(dep) {
                let bounded_out = if out.len() > 1500 {
                    format!("{}... [TRUNCATED FOR CONTEXT BOUNDS]", &out[..1500])
                } else {
                    out.clone()
                };
                task_context_history.push_str(&format!(
                    "---\n[DEPENDENCY FOCUS FROM {}]:\n{}\n---\n",
                    dep, bounded_out
                ));
            }
        }

        let combined_mission = format!(
            "OBJECTIVE: {}\nASSIGNED ROLE: {}\n\nLINEAGE CONTEXT:\n{}",
            node.objective, node.required_agent, task_context_history
        );

        let mut loop_cortex = CorticalLoop::new(
            ctx.api_key.clone(),
            world_model.clone(),
            ctx.memory.clone(),
            ctx.vmo_state.clone(),
            ctx.dist.clone(),
            ctx.tx.clone(),
        );

        let _ = ctx.tx.send(serde_json::json!({"type": "LOG_EVENT", "payload": format!("[CORTEX] Igniting Epistemic Session for Node {}", task_id)}).to_string());

        let execution_evidence = match loop_cortex.ignite(&combined_mission).await {
            Ok(CortexOutcome::CompletionProposed { belief, evidence }) => {
                format!(
                    "[DAG NODE COMPLETE] Objective: {}\nBelief: {:.2}\nEvidence:\n{}",
                    node.objective, belief, evidence
                )
            }
            Ok(CortexOutcome::TurnsExhausted { best_belief, evidence }) => {
                mission_had_failures = true;
                let outcome = MissionOutcome {
                    mission_id: uuid::Uuid::parse_str(&task_id)
                        .unwrap_or_else(|_| uuid::Uuid::new_v4()),
                    success: false,
                    evidence_summary: "Turns exhausted without contract satisfaction.".to_string(),
                    failures: vec![evidence.clone()],
                    successful_strategies: vec![],
                    raw_telemetry: String::new(),
                    failed_strategy_id: None,
                };
                if let Ok(report) = ctx.reflection_engine.reflect(&outcome).await {
                    for lesson in report.lessons {
                        let _ = ctx.tx.send(serde_json::json!({"type": "LOG_EVENT", "payload": format!("[REFLECTION ENGINE] {}", lesson)}).to_string());
                    }
                }
                let _ = ctx.tx.send(serde_json::json!({"type": "LOG_EVENT", "payload": format!("[CORTEX INCOMPLETE] Node {} exhausted turns (best belief {:.2}).", task_id, best_belief)}).to_string());
                format!("[DAG NODE INCOMPLETE] Objective: {}\nBest belief: {:.2}\nPartial evidence:\n{}", node.objective, best_belief, evidence)
            }
            Err(e) => {
                mission_had_failures = true;
                let _ = ctx.tx.send(serde_json::json!({"type": "LOG_EVENT", "payload": format!("[CORTEX FRACTURE] Node {} loop failed: {}", task_id, e)}).to_string());
                format!("[DAG NODE FAILED] Ancestor node '{}' failed: {}", node.objective, e)
            }
        };

        task_outputs.insert(task_id.clone(), execution_evidence.clone());
        world_model.record_node_evidence(&task_id, &node.objective, &execution_evidence);

        // ⚡ FIX: Immediate Hard Abort if a dependency fails or runs out of turns
        if mission_had_failures
            || execution_evidence.contains("[CORTEX INCOMPLETE]")
            || execution_evidence.contains("[DAG NODE FAILED]")
            || execution_evidence.contains("[DAG NODE INCOMPLETE]")
        {
            let _ = ctx.tx.send(serde_json::json!({"type": "LOG_EVENT", "payload": format!("[DAG HALT] Aborting downstream execution. Dependency '{}' failed to complete.", task_id)}).to_string());
            break;
        }
    }

    if mission_had_failures {
        format!("DAG/GoT Execution FINISHED WITH INCOMPLETE NODES. Plan ID: {}", plan_id)
    } else {
        format!("DAG/GoT Hybrid Execution Complete. Plan ID: {}", plan_id)
    }
}
