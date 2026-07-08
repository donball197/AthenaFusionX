use anyhow::Result;
use reqwest::Client;
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};
use tokio::time::{sleep, Duration};

use crate::agent_protocol::AgentAction;
use crate::cognition_graph::{CognitiveGraph, EdgeRelation, NodeKind};
use crate::distributed_scheduler::DistributedScheduler;
use crate::epistemic_engine::EpistemicEngine;
use crate::executor::RoboticExecutor;
use crate::shared_state::VmoState;
use crate::world_model::WorldModel;
use nexus_substrate::memory::MemoryPalace;

#[derive(Debug)]
pub enum CortexOutcome {
    Proven { belief: f32 },
    TurnsExhausted { best_belief: f32 },
}

pub struct CorticalLoop {
    client: Client,
    api_key: String,
    graph: CognitiveGraph,
    world_model: Arc<WorldModel>,
    memory: Arc<Mutex<MemoryPalace>>,
    vmo_state: Arc<Mutex<VmoState>>,
    dist: Arc<DistributedScheduler>,
    tx: broadcast::Sender<String>,
}

impl CorticalLoop {
    pub fn new(
        api_key: String,
        world_model: Arc<WorldModel>,
        memory: Arc<Mutex<MemoryPalace>>,
        vmo_state: Arc<Mutex<VmoState>>,
        dist: Arc<DistributedScheduler>,
        tx: broadcast::Sender<String>,
    ) -> Self {
        Self {
            client: Client::new(),
            api_key,
            graph: CognitiveGraph::new(),
            world_model,
            memory,
            vmo_state,
            dist,
            tx,
        }
    }

    pub async fn ignite(&mut self, mission: &str) -> Result<CortexOutcome> {
        println!("\x1b[1;36m>> [CORTEX] Igniting Unified Epistemic Engine\x1b[0m");

        let context_chunks = (*self.world_model).semantic_query(mission).await;
        let world_context = context_chunks.join("\n");

        println!("\x1b[36m>> [CORTEX] Generating competing strategies...\x1b[0m");
        let hypotheses = EpistemicEngine::generate_hypotheses(
            &self.client,
            &self.api_key,
            mission,
            &world_context,
        )
        .await?;

        for h in hypotheses {
            self.graph.add_node(NodeKind::Hypothesis, &h, 0.5, 100);
            println!("\x1b[90m  -> Strategy: {}\x1b[0m", h);
        }

        let mut max_observed_belief = 0.0_f32;

        for turn in 1..=15 {
            println!("\n\x1b[1;33m>> [CORTEX] Cognitive Turn {} / 15\x1b[0m", turn);

            if let Some(best_node) = self.graph.select_best_hypothesis().cloned() {
                let h_id = best_node.id;
                let h_content = best_node.content.clone();
                let current_belief = best_node.belief_score;
                if current_belief > max_observed_belief {
                    max_observed_belief = current_belief;
                }
                println!(
                    "\x1b[32m>> [CORTEX] Exploring Top Strategy (Belief: {:.2}):\x1b[0m {}",
                    current_belief, h_content
                );

                let mut evidence_history = String::new();
                for edge in &self.graph.edges {
                    if edge.target == h_id
                        && (edge.relation == EdgeRelation::Supports
                            || edge.relation == EdgeRelation::Contradicts)
                    {
                        if let Some(obs) = self.graph.nodes.get(&edge.source) {
                            evidence_history
                                .push_str(&format!("- [{:?}] {}\n", edge.relation, obs.content));
                        }
                    }
                }
                if evidence_history.is_empty() {
                    evidence_history = "None yet.".to_string();
                }

                println!("\x1b[36m>> [STRATEGIST] Planning next maneuver...\x1b[0m");
                let proposed_action = match EpistemicEngine::propose_action(
                    &self.client,
                    &self.api_key,
                    mission,
                    &h_content,
                    &evidence_history,
                )
                .await
                {
                    Ok(action) => action,
                    Err(e) => {
                        println!("\x1b[31m>> [STRATEGIST FAULT]\x1b[0m {}", e);
                        continue;
                    }
                };
                println!("\x1b[35m>> [STRATEGIST PROPOSAL]:\x1b[0m {}", proposed_action);

                println!("\x1b[31m>> [RED TEAM] Criticizing proposal...\x1b[0m");
                if let Ok(critic) = EpistemicEngine::criticize_proposal(
                    &self.client,
                    &self.api_key,
                    &h_content,
                    &proposed_action,
                    &world_context,
                )
                .await
                {
                    println!(
                        "\x1b[31m>> [CRITIC] Verdict:\x1b[0m {} (Modifier: {:.2})",
                        critic.critique, critic.confidence_modifier
                    );

                    if critic.confidence_modifier > -0.2 {
                        println!(
                            "\x1b[36m>> [CORTEX] Translating intent to physical action...\x1b[0m"
                        );

                        if let Ok(raw_json) = EpistemicEngine::translate_to_action(
                            &self.client,
                            &self.api_key,
                            &h_content,
                            &proposed_action,
                        )
                        .await
                        {
                            if let Ok(actions) = crate::json_parser::extract_actions(&raw_json) {
                                let mut executor =
                                    RoboticExecutor::new(self.api_key.clone(), self.tx.clone());

                                for action in actions {
                                    // LAZARUS SAFEGUARD FOR MUTATIONS
                                    match &action {
                                        AgentAction::PatchFunction { path, .. }
                                        | AgentAction::DiffPatch { path, .. }
                                        | AgentAction::WriteFile { path, .. } => {
                                            println!("\x1b[35m>> [LAZARUS] Taking atomic snapshot of {}...\x1b[0m", path);
                                            let _ = crate::lazarus_vault::take_snapshot(
                                                path,
                                                "Pre-fix snapshot",
                                            )
                                            .await;
                                        }
                                        _ => {}
                                    }

                                    println!(
                                        "\x1b[35m>> [EXECUTOR] Engaging physical tooling...\x1b[0m"
                                    );
                                    let evidence = match executor
                                        .execute_single_action(
                                            action,
                                            self.memory.clone(),
                                            self.vmo_state.clone(),
                                            self.dist.clone(),
                                        )
                                        .await
                                    {
                                        Ok(ev) => ev,
                                        Err(e) => {
                                            format!("[ACTION FAILED] OS rejected action: {}", e)
                                        }
                                    };

                                    let preview =
                                        evidence.lines().take(5).collect::<Vec<_>>().join("\n");
                                    println!("\x1b[32m>> [EVIDENCE ACQUIRED]\x1b[0m\n{}", preview);

                                    println!("\x1b[33m>> [CORTEX] Evaluating physical evidence against strategy...\x1b[0m");
                                    if let Ok(eval) = EpistemicEngine::evaluate_evidence(
                                        &self.client,
                                        &self.api_key,
                                        &h_content,
                                        &evidence,
                                    )
                                    .await
                                    {
                                        println!(
                                            "\x1b[33m>> [EVALUATION]\x1b[0m {} (Modifier: {:.2})",
                                            eval.critique, eval.confidence_modifier
                                        );

                                        let obs_id = self.graph.add_node(
                                            NodeKind::Observation,
                                            &evidence,
                                            1.0,
                                            50,
                                        );
                                        let relation = if eval.confidence_modifier > 0.0 {
                                            EdgeRelation::Supports
                                        } else {
                                            EdgeRelation::Contradicts
                                        };
                                        self.graph.add_edge(
                                            obs_id,
                                            h_id,
                                            relation,
                                            eval.confidence_modifier.abs(),
                                        );

                                        self.graph.propagate_beliefs();

                                        if let Some(updated_node) = self.graph.nodes.get(&h_id) {
                                            println!("\x1b[1;32m>> [CORTEX] Strategy Belief mathematically updated to {:.2}\x1b[0m", updated_node.belief_score);
                                            if updated_node.belief_score > max_observed_belief {
                                                max_observed_belief = updated_node.belief_score;
                                            }

                                            // ⚡ EXIT CONDITION: If belief is high AND it proved compilation/write success!
                                            if updated_node.belief_score >= 0.85
                                                && (evidence.contains("[WRITE_OK]")
                                                    || evidence.contains("[PATCH_FUNCTION_OK]")
                                                    || evidence.contains("[CMD_OK]"))
                                            {
                                                println!("\x1b[1;36m>> [CORTEX] STRATEGY PROVEN AND FIX APPLIED! Mission Accomplished.\x1b[0m");
                                                return Ok(CortexOutcome::Proven {
                                                    belief: updated_node.belief_score,
                                                });
                                            }

                                            if updated_node.belief_score < 0.2 {
                                                println!("\x1b[90m>> [CORTEX] Strategy completely abandoned. Pivoting graph...\x1b[0m");
                                            }
                                        }
                                    }
                                }
                            } else {
                                println!("\x1b[31m>> [CORTEX FAULT] Failed to parse AgentAction JSON from Translator.\x1b[0m");
                            }
                        }
                    } else {
                        println!("\x1b[31m>> [RED TEAM] VETO. Proposal rejected. Applying penalty to strategy.\x1b[0m");
                        self.graph.add_edge(
                            h_id,
                            h_id,
                            EdgeRelation::Contradicts,
                            critic.confidence_modifier.abs(),
                        );
                        self.graph.propagate_beliefs();
                    }
                }
            } else {
                println!(
                    "\x1b[1;35m>> [CORTEX] All strategies exhausted. Re-orienting graph...\x1b[0m"
                );
                let new_hypotheses = EpistemicEngine::generate_hypotheses(
                    &self.client,
                    &self.api_key,
                    mission,
                    &world_context,
                )
                .await;
                for h in new_hypotheses.unwrap_or_default() {
                    self.graph.add_node(NodeKind::Hypothesis, &h, 0.5, 100);
                    println!("\x1b[90m  -> New Strategy: {}\x1b[0m", h);
                }
            }
            sleep(Duration::from_secs(2)).await;
        }
        println!("\x1b[1;32m>> [CORTEX] Epistemic Execution Cycle Complete.\x1b[0m");
        Ok(CortexOutcome::TurnsExhausted { best_belief: max_observed_belief })
    }
}
