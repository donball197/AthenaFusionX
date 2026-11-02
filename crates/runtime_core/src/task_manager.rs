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
use nexus_substrate::episode::EpisodeContext;
use nexus_substrate::taint::TaintLabel;
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

    // Allocate the plan identity before persistence; the Executive Cortex
    // is deliberately not started until the initial plan row is durable.
    let plan_id = uuid::Uuid::new_v4().to_string();

    let mut graph =
        match PlannerAgent::generate_plan(&ctx.client, &ctx.api_key, raw_directive).await {
            Ok(g) => g,
            Err(e) => return format!("FRACTURE: Planner failed to decompose objective: {}", e),
        };

    let expected_paths: Vec<String> =
        crate::completion_engine::SubgoalCompletionEngine::infer_contract("ad_hoc", raw_directive)
            .required_artifact_paths
            .into_iter()
            .map(|a| a.path)
            .collect();
    for expected_path in &expected_paths {
        let has_write_node =
            graph.nodes.values().any(|n| n.objective.contains(expected_path.as_str()));
        if !has_write_node {
            return format!(
                "FRACTURE: Directive requires writing '{}' but Planner produced no node for it. Refusing to run an incomplete DAG.",
                expected_path
            );
        }
    }

    graph.compute_critical_path_priorities();
    let order = match graph.resolve_execution_order() {
        Ok(o) => o,
        Err(e) => return format!("FRACTURE: Graph validation failed: {}", e),
    };

    // Initial persistence is a hard execution boundary: no DAG node may
    // execute unless its plan record has been durably created.
    let initial_graph_json = match serde_json::to_string(&graph) {
        Ok(json) => json,
        Err(error) => {
            return format!(
                "PERSISTENCE FAILURE: Could not serialize initial plan graph: {}",
                error
            );
        }
    };
    let initial_now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;

    if let Err(error) = ctx
        .memory
        .upsert_plan_run(
            &plan_id,
            raw_directive,
            &initial_graph_json,
            "Active",
            initial_now,
        )
        .await
    {
        return format!(
            "PERSISTENCE FAILURE: Plan {} was not stored; execution was not started: {}",
            plan_id, error
        );
    }

    let _ = ctx.tx.send(
        serde_json::json!({
            "type": "LOG_EVENT",
            "payload": format!("[EXECUTIVE CORTEX] Plan {} persisted.", plan_id)
        })
        .to_string(),
    );

    // Start the Executive Cortex only after the initial plan record exists.
    let (executive_cortex, command_tx) = ExecutiveCortex::new(ctx.memory.clone());
    let goal_id = uuid::Uuid::new_v4();

    if let Err(error) = command_tx
        .send(ExecutiveCommand::InjectGoal {
            plan_id: plan_id.clone(),
            objective: raw_directive.to_string(),
            parent_id: None,
        })
        .await
    {
        return format!(
            "PERSISTENCE FAILURE: Plan {} was stored, but Executive Cortex goal registration failed: {}",
            plan_id, error
        );
    }

    tokio::spawn(executive_cortex.run_actor_loop());

    let _ = ctx.tx.send(
        serde_json::json!({
            "type": "LOG_EVENT",
            "payload": format!(
                "[EXECUTIVE CORTEX] Registered goal {}. Decomposing DAG...",
                goal_id
            )
        })
        .to_string(),
    );

    let planned_node_count = order.len();
    let mut completed_node_count = 0usize;
    let mut task_outputs: HashMap<String, String> = HashMap::new();
    let mut mission_had_failures = false;
    let world_model = Arc::new(WorldModel::new());

    for task_id in order {
        let node = graph.nodes.get(&task_id).unwrap().clone();
        let _ = ctx.tx.send(serde_json::json!({"type": "LOG_EVENT", "payload": format!("[SCHEDULER] Dispatching DAG Node: {}\nvia Subagent: {}", node.objective, node.required_agent)}).to_string());

        // Structured DAG telemetry: this is the actual production execution boundary.
        let _ = ctx.tx.send(
            serde_json::json!({
                "type": "DAG_STATE",
                "id": task_id,
                "objective": node.objective,
                "status": "RUNNING"
            })
            .to_string(),
        );

        let mut task_context_history = String::new();

        if node.deps.is_empty() {
            task_context_history.push_str("[ROOT NODE - NO PREDECESSOR LINEAGE]\n");
        } else {
            for dep in &node.deps {
                if let Some(out) = task_outputs.get(dep) {
                    let bounded_out = if out.len() > 1500 {
                        format!("{}... [TRUNCATED FOR CONTEXT BOUNDS]", &out[..1500])
                    } else {
                        out.clone()
                    };
                    task_context_history.push_str(&format!(
                        "---\n[DEPENDENCY LINEAGE FROM {}]:\n{}\n---\n",
                        dep, bounded_out
                    ));
                } else {
                    task_context_history.push_str(&format!(
                        "---\n[DEPENDENCY LINEAGE FROM {}]:\n[NO COMPLETED OUTPUT AVAILABLE]\n---\n",
                        dep
                    ));
                }
            }
        }

        let lineage_inherit_only = !node.deps.is_empty()
            && crate::completion_engine::is_lineage_inherit_only_objective(&node.objective);

        let effective_success_contract = if lineage_inherit_only {
            vec!["DAG_LINEAGE_OK".to_string()]
        } else {
            node.success_criteria.clone()
        };

        let success_contract = if effective_success_contract.is_empty() {
            "[NO EXPLICIT SUCCESS CRITERIA — NODE MUST NOT BE AUTHORIZED AUTOMATICALLY]".to_string()
        } else {
            effective_success_contract
                .iter()
                .map(|criterion| format!("- {}", criterion))
                .collect::<Vec<_>>()
                .join("\n")
        };

        let lineage_mode = if lineage_inherit_only { "LINEAGE MODE: INHERIT_ONLY\n" } else { "" };

        let combined_mission = format!(
            "{}OBJECTIVE: {}\nASSIGNED ROLE: {}\n\nMANDATORY NODE SUCCESS CRITERIA:\n{}\n\nNODE COMPLETION RULE: Every mandatory criterion must be physically evidenced before this node can be considered complete. Cortex belief alone is never sufficient.\n\nROOT DIRECTIVE CONTEXT:\n{}\n\nLINEAGE CONTEXT:\n{}",
            lineage_mode,
            node.objective,
            node.required_agent,
            success_contract,
            raw_directive,
            task_context_history
        );

        let mut loop_cortex = CorticalLoop::new(
            ctx.api_key.clone(),
            world_model.clone(),
            ctx.memory.clone(),
            ctx.vmo_state.clone(),
            ctx.dist.clone(),
            ctx.tx.clone(),
        );

        // Episode context for this DAG node. plan_id and task_id
        // are known here; session_id is minted inside
        // ignite_with_evidence.
        loop_cortex.set_episode(nexus_substrate::episode::EpisodeContext {
            plan_id: Some(plan_id.clone()),
            task_id: Some(task_id.clone()),
            session_id: None,
            mission_id: None,
        });

        let _ = ctx.tx.send(serde_json::json!({"type": "LOG_EVENT", "payload": format!("[CORTEX] Igniting Epistemic Session for Node {}", task_id)}).to_string());

        let ancestor_evidence = if !node.deps.is_empty() {
            node.deps
                .iter()
                .filter_map(|dep| task_outputs.get(dep))
                .map(String::as_str)
                .collect::<Vec<_>>()
                .join("\n")
        } else {
            String::new()
        };

        // Completion authorization is a two-stage boundary:
        //
        //   1. Cortex must explicitly propose completion.
        //   2. The deterministic verifier must independently prove every
        //      success criterion from physical evidence.
        //
        // Cortex exhaustion/failure is never eligible for stage 2.
        let cortex_proposed_completion: bool;

        let execution_evidence = match loop_cortex
            .ignite_with_evidence(
                &combined_mission,
                if ancestor_evidence.is_empty() { None } else { Some(ancestor_evidence.as_str()) },
            )
            .await
        {
            Ok(CortexOutcome::CompletionProposed { belief, evidence }) => {
                cortex_proposed_completion = true;
                // The Cortex/OutcomeVerifier has already authorized this node
                // completion. Reflection learns from the VERIFIED result;
                // reflection itself does not authorize completion.
                let success_outcome = MissionOutcome {
                    mission_id: uuid::Uuid::parse_str(&task_id)
                        .unwrap_or_else(|_| uuid::Uuid::new_v4()),
                    success: true,
                    evidence_summary: evidence.clone(),
                    failures: vec![],
                    successful_strategies: vec![node.objective.clone()],
                    raw_telemetry: String::new(),
                    failed_strategy_id: None,
                };

                if let Ok(report) = ctx.reflection_engine.reflect(&success_outcome).await {
                    for lesson in report.lessons {
                        let _ = ctx.tx.send(
                            serde_json::json!({
                                "type": "LOG_EVENT",
                                "payload": format!(
                                    "[REFLECTION SUCCESS] {}",
                                    lesson
                                )
                            })
                            .to_string(),
                        );

                        let lesson_text: String = format!(
                            "[VERIFIED_SUCCESS_LESSON] Objective: {} | {}",
                            node.objective, lesson
                        )
                        .chars()
                        .take(4000)
                        .collect();

                        let emb = nexus_substrate::vector_bridge::get_embedding(
                            &ctx.api_key,
                            &lesson_text,
                        )
                        .await
                        .ok();

                        match ctx
                            .memory
                            .store_memory_with_episode(
                                "ReflectionEngine",
                                &node.objective,
                                &lesson_text,
                                TaintLabel::INTERNAL,
                                emb,
                                &EpisodeContext {
                                    plan_id: Some(plan_id.clone()),
                                    task_id: Some(task_id.clone()),
                                    session_id: None,
                                    mission_id: None,
                                },
                            )
                            .await
                        {
                            Ok(()) => {
                                tracing::info!(
                                    "[FUSION] Stored verified successful experience for future recall."
                                );
                            }
                            Err(error) => {
                                tracing::warn!(
                                    "[FUSION] Failed to store successful lesson: {}",
                                    error
                                );
                            }
                        }
                    }
                }

                format!(
                    "[DAG NODE COMPLETE] Objective: {}\nBelief: {:.2}\nEvidence:\n{}",
                    node.objective, belief, evidence
                )
            }
            Ok(CortexOutcome::TurnsExhausted { best_belief, evidence }) => {
                // Cortex exhaustion is a hard completion boundary.
                // Partial action evidence must never promote an exhausted
                // Cortex session into a completed DAG node.
                cortex_proposed_completion = false;

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
                        // ⚡ Commit lesson to VectorVault so future missions automatically inherit it
                        let lesson_text = format!(
                            "[REFLECTION_LESSON] Objective: {} | Insight: {}",
                            node.objective, lesson
                        );
                        let emb = nexus_substrate::vector_bridge::get_embedding(
                            &ctx.api_key,
                            &lesson_text,
                        )
                        .await
                        .ok();
                        let _ = ctx
                            .memory
                            .store_memory_with_episode(
                                "ReflectionEngine",
                                &node.objective,
                                &lesson_text,
                                TaintLabel::INTERNAL,
                                emb,
                                &EpisodeContext {
                                    plan_id: Some(plan_id.clone()),
                                    task_id: Some(task_id.clone()),
                                    session_id: None,
                                    mission_id: None,
                                },
                            )
                            .await;
                    }
                }
                let _ = ctx.tx.send(serde_json::json!({"type": "LOG_EVENT", "payload": format!("[CORTEX INCOMPLETE] Node {} exhausted turns (best belief {:.2}).", task_id, best_belief)}).to_string());
                format!("[DAG NODE INCOMPLETE] Objective: {}\nBest belief: {:.2}\nPartial evidence:\n{}", node.objective, best_belief, evidence)
            }
            Err(e) => {
                // A Cortex failure cannot authorize DAG completion.
                cortex_proposed_completion = false;
                mission_had_failures = true;
                let _ = ctx.tx.send(serde_json::json!({"type": "LOG_EVENT", "payload": format!("[CORTEX FRACTURE] Node {} loop failed: {}", task_id, e)}).to_string());
                format!("[DAG NODE FAILED] Ancestor node '{}' failed: {}", node.objective, e)
            }
        };

        task_outputs.insert(task_id.clone(), execution_evidence.clone());
        world_model.record_node_evidence(&task_id, &node.objective, &execution_evidence);

        // Cortex CompletionProposed is a REQUIRED precondition, not final
        // authority. The deterministic verifier remains the final authority.
        //
        // This prevents TurnsExhausted/INCOMPLETE evidence from being
        // promoted to Completed merely because partial exploration emitted
        // individually valid-looking evidence markers.
        let verification_criteria = if lineage_inherit_only {
            vec!["DAG_LINEAGE_OK".to_string()]
        } else {
            node.success_criteria.clone()
        };

        let node_verification = if !cortex_proposed_completion {
            crate::orchestrator_v2::verifier::VerificationResult::Failure(
                "Cortex did not propose completion. Node completion is not authorized.".to_string(),
            )
        } else if verification_criteria.is_empty() {
            crate::orchestrator_v2::verifier::VerificationResult::Failure(
                "Node has no explicit success criteria. Independent completion proof is unavailable."
                    .to_string(),
            )
        } else {
            crate::orchestrator_v2::verifier::Verifier::verify_contract(
                &task_id,
                &execution_evidence,
                &verification_criteria,
            )
        };

        let node_verification_succeeded = matches!(
            node_verification,
            crate::orchestrator_v2::verifier::VerificationResult::Success
        );

        match node_verification {
            crate::orchestrator_v2::verifier::VerificationResult::Success => {
                completed_node_count += 1;

                if let Some(graph_node) = graph.nodes.get_mut(&task_id) {
                    graph_node.status = crate::orchestrator_v2::task_graph::TaskStatus::Completed;
                    graph_node.output_evidence = Some(execution_evidence.clone());
                }

                // Structured DAG telemetry mirrors the authoritative production mutation.
                let _ = ctx.tx.send(
                    serde_json::json!({
                        "type": "DAG_STATE",
                        "id": task_id,
                        "objective": node.objective,
                        "status": "COMPLETED"
                    })
                    .to_string(),
                );

                let _ = ctx.tx.send(
                    serde_json::json!({
                        "type": "LOG_EVENT",
                        "payload": format!(
                            "[DAG NODE VERIFIED] {}/{} nodes completed: {}",
                            completed_node_count,
                            planned_node_count,
                            node.objective
                        )
                    })
                    .to_string(),
                );
            }

            other => {
                mission_had_failures = true;

                let reason = format!("{:?}", other);

                if let Some(graph_node) = graph.nodes.get_mut(&task_id) {
                    graph_node.status = crate::orchestrator_v2::task_graph::TaskStatus::Failed;
                    graph_node.output_evidence = Some(format!(
                        "{}\n[NODE VERIFICATION FAILED] {}",
                        execution_evidence, reason
                    ));
                }

                // Structured DAG telemetry mirrors the authoritative production mutation.
                let _ = ctx.tx.send(
                    serde_json::json!({
                        "type": "DAG_STATE",
                        "id": task_id,
                        "objective": node.objective,
                        "status": "FAILED"
                    })
                    .to_string(),
                );

                let _ = ctx.tx.send(
                    serde_json::json!({
                        "type": "LOG_EVENT",
                        "payload": format!(
                            "[NODE VERIFICATION FAILED] {} :: {}",
                            node.objective, reason
                        )
                    })
                    .to_string(),
                );
            }
        }

        // Persist the verified node outcome before another node can be
        // dispatched. A failed checkpoint halts the DAG immediately.
        let checkpoint_graph_json = match serde_json::to_string(&graph) {
            Ok(json) => json,
            Err(error) => {
                return format!(
                    "PERSISTENCE FAILURE: Could not serialize checkpoint for plan {}: {}",
                    plan_id, error
                );
            }
        };
        let checkpoint_now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        match ctx
            .memory
            .update_plan_run_checkpoint(
                &plan_id,
                &checkpoint_graph_json,
                "Active",
                checkpoint_now,
            )
            .await
        {
            Ok(true) => {}
            Ok(false) => {
                return format!(
                    "PERSISTENCE FAILURE: Plan {} was no longer Active; downstream execution halted.",
                    plan_id
                );
            }
            Err(error) => {
                return format!(
                    "PERSISTENCE FAILURE: Checkpoint for plan {} failed; downstream execution halted: {}",
                    plan_id, error
                );
            }
        }

        // Completion requires BOTH authorization stages:
        //   Cortex CompletionProposed + deterministic contract verification.
        //
        // Any Cortex exhaustion/failure therefore reaches this halt path.
        if !node_verification_succeeded {
            let _ = ctx.tx.send(
                serde_json::json!({
                    "type": "LOG_EVENT",
                    "payload": format!(
                        "[DAG HALT] Aborting downstream execution. Dependency '{}' failed deterministic verification.",
                        task_id
                    )
                })
                .to_string()
            );
            break;
        }
    }

    // WHOLE-DAG COMPLETION GATE:
    // A successful Cortex node is only a subgoal completion.
    // The overall DAG is complete only when every planned node completed.
    let final_graph_json = match serde_json::to_string(&graph) {
        Ok(json) => json,
        Err(error) => {
            return format!(
                "PERSISTENCE FAILURE: Could not serialize final graph for plan {}: {}",
                plan_id, error
            );
        }
    };
    let final_status = if mission_had_failures
        || planned_node_count == 0
        || completed_node_count != planned_node_count
    {
        "Aborted"
    } else {
        "Completed"
    };
    let final_now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;

    match ctx
        .memory
        .update_plan_run_checkpoint(
            &plan_id,
            &final_graph_json,
            final_status,
            final_now,
        )
        .await
    {
        Ok(true) => {}
        Ok(false) => {
            return format!(
                "PERSISTENCE FAILURE: Plan {} could not transition from Active to {}.",
                plan_id, final_status
            );
        }
        Err(error) => {
            return format!(
                "PERSISTENCE FAILURE: Final {} transition for plan {} failed: {}",
                final_status, plan_id, error
            );
        }
    }

    if mission_had_failures || planned_node_count == 0 || completed_node_count != planned_node_count
    {
        let _ = ctx.tx.send(
            serde_json::json!({
                "type": "LOG_EVENT",
                "payload": format!(
                    "[DAG INCOMPLETE] Plan {} completed {}/{} nodes.                      Whole mission completion is NOT authorized.",
                    plan_id,
                    completed_node_count,
                    planned_node_count
                )
            })
            .to_string()
        );

        format!(
            "DAG/GoT Execution FINISHED WITH INCOMPLETE NODES.              Completed {}/{} nodes. Plan ID: {}",
            completed_node_count,
            planned_node_count,
            plan_id
        )
    } else {
        let _ = ctx.tx.send(
            serde_json::json!({
                "type": "LOG_EVENT",
                "payload": format!(
                    "[DAG COMPLETE] All {}/{} planned nodes completed successfully.                      Whole-DAG completion authorized. Plan ID: {}",
                    completed_node_count,
                    planned_node_count,
                    plan_id
                )
            })
            .to_string()
        );

        format!(
            "DAG/GoT Hybrid Execution Complete.              All {}/{} nodes completed. Plan ID: {}",
            completed_node_count,
            planned_node_count,
            plan_id
        )
    }
}
