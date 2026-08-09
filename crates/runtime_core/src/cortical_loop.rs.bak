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
use crate::memory_cortex::MemoryCortex;
use crate::shared_state::VmoState;
use crate::world_model::WorldModel;

#[derive(Debug)]
pub enum CortexOutcome {
    Proven { belief: f32, evidence: String },
    TurnsExhausted { best_belief: f32, evidence: String },
}

pub struct CorticalLoop {
    client: Client,
    api_key: String,
    graph: CognitiveGraph,
    world_model: Arc<WorldModel>,
    memory: Arc<MemoryCortex>,
    vmo_state: Arc<Mutex<VmoState>>,
    dist: Arc<DistributedScheduler>,
    tx: broadcast::Sender<String>,
}

impl CorticalLoop {
    pub fn new(
        api_key: String,
        world_model: Arc<WorldModel>,
        memory: Arc<MemoryCortex>,
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

    fn collect_evidence(&self, h_id: uuid::Uuid) -> String {
        let mut history = String::new();
        for edge in &self.graph.edges {
            if edge.target == h_id
                && (edge.relation == EdgeRelation::Supports || edge.relation == EdgeRelation::Contradicts)
            {
                if let Some(obs) = self.graph.nodes.get(&edge.source) {
                    history.push_str(&format!("- [{:?}] {}\n", edge.relation, obs.content));
                }
            }
        }
        if history.is_empty() {
            "No evidence recorded.".to_string()
        } else {
            history
        }
    }

    fn log(&self, msg: String) {
        let _ = self.tx.send(
            serde_json::json!({"type": "NEURAL_STREAM", "payload": format!("{}\n", msg)})
                .to_string(),
        );
    }

    async fn resolve_workspace_paths() -> String {
        match tokio::process::Command::new("cargo")
            .args(["metadata", "--no-deps", "--format-version=1"])
            .output()
            .await
        {
            Ok(output) if output.status.success() => {
                match serde_json::from_slice::<serde_json::Value>(&output.stdout) {
                    Ok(meta) => {
                        let workspace_root = meta
                            .get("workspace_root")
                            .and_then(|v| v.as_str())
                            .unwrap_or("<unresolved>");
                        let target_dir = meta
                            .get("target_directory")
                            .and_then(|v| v.as_str())
                            .unwrap_or("<unresolved>");
                        format!(
                            "WORKSPACE FACTS (ground truth, do not guess): workspace_root={workspace_root} | target_dir={target_dir}. This is a Cargo WORKSPACE. ALL compiled binaries land at {target_dir}/debug/<bin_name> or {target_dir}/release/<bin_name> REGARDLESS of which member crate directory (e.g. apps/<name>) you build from. NEVER assume a binary exists at <member_crate>/target/release/<bin_name> or ./target/release/<bin_name> when cwd is a member crate -- that path does not exist in a workspace. Prefer `cargo run --manifest-path <member_crate>/Cargo.toml --release -- <args>` or the absolute path {target_dir}/release/<bin_name>."
                        )
                    }
                    Err(e) => format!(
                        "WORKSPACE FACTS: cargo metadata parse failed ({e}); do not guess target-dir paths, verify with `cargo metadata` or `find` first."
                    ),
                }
            }
            Ok(output) => format!(
                "WORKSPACE FACTS: cargo metadata exited non-zero ({}); do not guess target-dir paths.",
                output.status
            ),
            Err(e) => format!(
                "WORKSPACE FACTS: cargo metadata failed to run ({e}); do not guess target-dir paths."
            ),
        }
    }

    pub async fn ignite(&mut self, mission: &str) -> Result<CortexOutcome> {
        self.ignite_with_evidence(mission, None).await
    }

    pub async fn ignite_with_evidence(&mut self, mission: &str, ancestor_evidence: Option<&str>) -> Result<CortexOutcome> {
        self.log("\x1b[1;36m>> [CORTEX] Igniting Unified Epistemic Engine\x1b[0m".to_string());

        let mut context_chunks = (*self.world_model).semantic_query(mission).await;
        context_chunks.push(Self::resolve_workspace_paths().await);
        context_chunks.push("CRITICAL RULE FOR WRITING FILES: Do not use shell commands to write files. To write or overwrite a file, output EXACTLY this XML block in your proposal:\n<write_file path=\"exact/path.rs\">\n[RAW FILE CONTENT HERE]\n</write_file>".to_string());
        if let Some(evidence) = ancestor_evidence {
            if !evidence.trim().is_empty() {
                let ancestor_succeeded = evidence.contains("[CMD_OK]")
                    || evidence.contains("[READ_OK]")
                    || evidence.contains("[WRITE_OK]")
                    || evidence.contains("[DIFF_PATCH_OK]")
                    || evidence.contains("[PATCH_FUNCTION_OK]");
                let label = if ancestor_succeeded {
                    "[ANCESTOR NODE EVIDENCE & RESOLVED PATHS]"
                } else {
                    "[ANCESTOR NODE FAILURE -- DO NOT TREAT PATHS BELOW AS RESOLVED]"
                };
                context_chunks.push(format!("\n{}\n{}", label, evidence));
            }
        }
        let world_context = context_chunks.join("\n");

        self.log("\x1b[36m>> [CORTEX] Generating competing strategies...\x1b[0m".to_string());
        let hypotheses = EpistemicEngine::generate_hypotheses(
            &self.client,
            &self.api_key,
            mission,
            &world_context,
        )
        .await?;

        for h in hypotheses {
            self.graph.add_node(NodeKind::Hypothesis, &h, 0.5, 100);
            self.log(format!("\x1b[90m  -> Strategy: {}\x1b[0m", h));
        }

        let mut max_observed_belief = 0.0_f32;
        let mut best_h_id: Option<uuid::Uuid> = None;

        for turn in 1..=15 {
            self.log(format!("\n\x1b[1;33m>> [CORTEX] Cognitive Turn {} / 15\x1b[0m", turn));

            if let Some(best_node) = self.graph.select_best_hypothesis().cloned() {
                let h_id = best_node.id;
                let h_content = best_node.content.clone();
                let current_belief = best_node.belief_score;
                if current_belief > max_observed_belief {
                    max_observed_belief = current_belief;
                    best_h_id = Some(h_id);
                }
                self.log(format!(
                    "\x1b[32m>> [CORTEX] Exploring Top Strategy (Belief: {:.2}):\x1b[0m {}",
                    current_belief, h_content
                ));

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

                // --- DETERMINISTIC BACKSTOP ---
                let lower_m = mission.to_lowercase();

                let is_pure_read = lower_m.contains("read")
                    && !lower_m.contains("overhead")
                    && !lower_m.contains("evaluate")
                    && !lower_m.contains("benchmark");

                if is_pure_read && evidence_history.contains("[READ_OK]")
                {
                    self.log("\x1b[1;36m>> [CORTEX] DETERMINISTIC GUARD: Source code read objective satisfied. Halting early.\x1b[0m".to_string());
                    // return Ok(CortexOutcome::Proven { belief: 1.0, evidence: evidence_history.clone() });
                }
                let is_path_task = lower_m.contains("resolve absolute path") || lower_m.contains("query symbol_graph");
                let is_report_task = lower_m.contains("report") || lower_m.contains("synthesize") || lower_m.contains("verdict");

                if is_path_task
                    && !is_report_task
                    && evidence_history.contains("[CMD_OK]")
                    && evidence_history.contains(".rs")
                {
                    self.log("\x1b[1;36m>> [CORTEX] DETERMINISTIC GUARD: Path resolution objective satisfied. Halting early.\x1b[0m".to_string());
                    return Ok(CortexOutcome::Proven { belief: 1.0, evidence: evidence_history.clone() });
                }
                if evidence_history.contains("[HALT]") {
                    self.log("\x1b[1;36m>> [CORTEX] DETERMINISTIC GUARD: HALT detected in evidence. Mission Accomplished.\x1b[0m".to_string());
                    return Ok(CortexOutcome::Proven { belief: 1.0, evidence: evidence_history.clone() });
                }
                // ------------------------------

                self.log("\x1b[36m>> [STRATEGIST] Planning next maneuver...\x1b[0m".to_string());
                let proposed_action = match EpistemicEngine::propose_action(
                    &self.client,
                    &self.api_key,
                    mission,
                    &h_content,
                    &world_context,
                    &evidence_history,
                )
                .await
                {
                    Ok(action) => action,
                    Err(e) => {
                        self.log(format!("\x1b[31m>> [STRATEGIST FAULT]\x1b[0m {}", e));
                        continue;
                    }
                };
                self.log(format!("\x1b[35m>> [STRATEGIST PROPOSAL]:\x1b[0m {}", proposed_action));

                self.log("\x1b[31m>> [RED TEAM] Criticizing proposal...\x1b[0m".to_string());
                match EpistemicEngine::criticize_proposal(
                    &self.client,
                    &self.api_key,
                    mission,
                    &h_content,
                    &proposed_action,
                    &world_context,
                    &evidence_history,
                )
                .await
                {
                    Err(e) => {
                        self.log(format!("\x1b[31m>> [CORTEX FATAL] Red Team critic parser panicked. Raw Error: {}\x1b[0m", e));
                        let evidence = format!("[CRITIC_PARSE_FAILURE] Adversarial Critic response could not be parsed as valid JSON ({}). No safety verdict was reached for this proposal -- treat this turn as unresolved, not silent progress.", e);
                        let obs_id = self.graph.add_node(NodeKind::Observation, &evidence, 1.0, 50);
                        self.graph.add_edge(obs_id, h_id, EdgeRelation::Contradicts, 0.15);
                        self.graph.propagate_beliefs();
                    }
                    Ok(critic) => {
                        self.log(format!(
                            "\x1b[31m>> [CRITIC] Verdict:\x1b[0m {} (Modifier: {:.2})",
                            critic.critique, critic.confidence_modifier
                        ));

                        if critic.confidence_modifier > -0.2 {
                            self.log(
                                "\x1b[36m>> [CORTEX] Translating intent to physical action...\x1b[0m"
                                    .to_string(),
                            );

                            if let Ok(raw_json) = EpistemicEngine::translate_to_action(
                                &self.client,
                                &self.api_key,
                                &h_content,
                                &proposed_action,
                            )
                            .await
                            {
                                if let Ok(mut actions) = crate::json_parser::extract_actions(&raw_json)
                                {
                                    if actions.is_empty() && !proposed_action.trim().is_empty() {
                                        let action_str = proposed_action.trim();
                                        if let (Some(start_tag), Some(end_quotes)) = (action_str.find("<write_file path=\""), action_str.find("\">")) {
                                            let path = action_str[start_tag + 18..end_quotes].to_string();
                                            if let Some(close_tag) = action_str.find("</write_file>") {
                                                let content_start = end_quotes + 2;
                                                let content = action_str[content_start..close_tag].trim_start_matches('\n').trim_end_matches('\n').to_string();
                                                self.log("\x1b[32m>> [CORTEX INTERCEPT] Native XML Tool Intercept: Handling <write_file> directly.\x1b[0m".to_string());
                                                actions.push(AgentAction::WriteFile {
                                                    path,
                                                    content,
                                                    reasoning: "Intercepted XML write_file block".to_string(),
                                                    confidence: 0.9,
                                                });
                                            } else {
                                                self.log("\x1b[31m>> [CORTEX FAULT] Found <write_file> but missing </write_file>.\x1b[0m".to_string());
                                            }
                                        } else {
                                            self.log("\x1b[33m>> [CORTEX FALLBACK] Translator produced empty JSON. Executing proposed action directly...\x1b[0m".to_string());
                                            actions.push(AgentAction::ExecuteCommand {
                                                command: proposed_action.clone(),
                                                reasoning: "Fallback raw command execution".to_string(),
                                                confidence: 0.8,
                                            });
                                        }
                                    }

                                    if actions.is_empty() {
                                        self.log("\x1b[31m>> [CORTEX FAULT] Translator produced an empty action list.\x1b[0m".to_string());
                                        let evidence = "[TRANSLATION_EMPTY] Translator produced an empty action list. This turn produced no executable action.".to_string();
                                        let obs_id = self.graph.add_node(NodeKind::Observation, &evidence, 1.0, 50);
                                        self.graph.add_edge(obs_id, h_id, EdgeRelation::Contradicts, 0.15);
                                        self.graph.propagate_beliefs();
                                    } else {
                                        let mut executor =
                                            RoboticExecutor::new(self.api_key.clone(), self.tx.clone());

                                        for action in actions {
                                        match &action {
                                            AgentAction::PatchFunction { path, .. }
                                            | AgentAction::DiffPatch { path, .. }
                                            | AgentAction::WriteFile { path, .. } => {
                                                self.log(format!("\x1b[35m>> [LAZARUS] Taking atomic snapshot of {}...\x1b[0m", path));
                                                let _ = crate::lazarus_vault::take_snapshot(
                                                    path,
                                                    "Pre-fix snapshot",
                                                )
                                                .await;
                                            }
                                            _ => {}
                                        }

                                        self.log(
                                            "\x1b[35m>> [EXECUTOR] Engaging physical tooling...\x1b[0m"
                                                .to_string(),
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
                                        self.log(format!(
                                            "\x1b[32m>> [EVIDENCE ACQUIRED]\x1b[0m\n{}",
                                            preview
                                        ));

                                        self.log("\x1b[33m>> [CORTEX] Evaluating physical evidence against strategy...\x1b[0m".to_string());
                                        match EpistemicEngine::evaluate_evidence(
                                            &self.client,
                                            &self.api_key,
                                            &h_content,
                                            &proposed_action,
                                            &evidence,
                                        )
                                        .await
                                        {
                                            Err(e) => {
                                                self.log(format!("\x1b[31m>> [CORTEX FATAL] Evidence evaluation parser panicked. Raw Error: {}\x1b[0m", e));
                                            }
                                            Ok(eval) => {
                                                self.log(format!(
                                                    "\x1b[33m>> [EVALUATION]\x1b[0m {} (Modifier: {:.2})",
                                                    eval.critique, eval.confidence_modifier
                                                ));

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

                                                if let Some(updated_node) =
                                                    self.graph.nodes.get(&h_id)
                                                {
                                                    self.log(format!("\x1b[1;32m>> [CORTEX] Strategy Belief mathematically updated to {:.2}\x1b[0m", updated_node.belief_score));
                                                    if updated_node.belief_score
                                                        > max_observed_belief
                                                    {
                                                        max_observed_belief =
                                                            updated_node.belief_score;
                                                    }

                                                    let _is_mutation = evidence
                                                        .contains("[WRITE_OK]")
                                                        || evidence.contains("[PATCH_FUNCTION_OK]")
                                                        || evidence.contains("[DIFF_PATCH_OK]");
                                                    let _is_terminal_cmd = evidence
                                                        .contains("[CMD_OK]")
                                                        && !proposed_action.contains("grep")
                                                        && !proposed_action.contains("ls")
                                                        && !proposed_action.contains("cat")
                                                        && !proposed_action.contains("find");
                                                    let is_halt = evidence.contains("[HALT]");

                                                    if updated_node.belief_score >= 0.85 && is_halt
                                                    {
                                                        self.log("\x1b[1;36m>> [CORTEX] STRATEGY PROVEN AND FIX APPLIED! Mission Accomplished.\x1b[0m".to_string());
                                                        return Ok(CortexOutcome::Proven {
                                                            belief: updated_node.belief_score,
                                                            evidence: self.collect_evidence(h_id),
                                                        });
                                                    }

                                                    if updated_node.belief_score < 0.2 {
                                                        self.log("\x1b[90m>> [CORTEX] Strategy completely abandoned. Pivoting graph...\x1b[0m".to_string());
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    }
                                } else {
                                    self.log("\x1b[31m>> [CORTEX FAULT] Failed to parse AgentAction JSON from Translator.\x1b[0m".to_string());
                                    let evidence = "[TRANSLATION_FAILURE] Translator produced JSON that could not be parsed into a valid AgentAction. This turn produced no executable action.".to_string();
                                    let obs_id = self.graph.add_node(
                                        NodeKind::Observation,
                                        &evidence,
                                        1.0,
                                        50,
                                    );
                                    self.graph.add_edge(
                                        obs_id,
                                        h_id,
                                        EdgeRelation::Contradicts,
                                        0.15,
                                    );
                                    self.graph.propagate_beliefs();
                                }
                            } else {
                                self.log("\x1b[31m>> [TRANSLATOR FAULT] Translator API call failed; no action was generated this turn.\x1b[0m".to_string());
                                let evidence = "[TRANSLATOR_API_FAILURE] The Action Translator's API call failed (rate limit or transient error). This turn produced no executable action.".to_string();
                                let obs_id =
                                    self.graph.add_node(NodeKind::Observation, &evidence, 1.0, 50);
                                self.graph.add_edge(obs_id, h_id, EdgeRelation::Contradicts, 0.10);
                                self.graph.propagate_beliefs();
                            }
                        } else {
                            self.log("\x1b[31m>> [RED TEAM] VETO. Proposal rejected. Injecting Critic rationale into Memory Graph.\x1b[0m".to_string());
                            let veto_msg = format!("[RED TEAM VETO] Action \"{}\" was blocked. Critic feedback: {}", proposed_action.trim(), critic.critique);
                            let veto_obs_id = self.graph.add_node(NodeKind::Observation, &veto_msg, 1.0, 50);
                            self.graph.add_edge(
                                veto_obs_id,
                                h_id,
                                EdgeRelation::Contradicts,
                                critic.confidence_modifier.abs(),
                            );
                            self.graph.propagate_beliefs();
                        }
                    }
                }
            } else {
                self.log(
                    "\x1b[1;35m>> [CORTEX] All strategies exhausted. Re-orienting graph...\x1b[0m"
                        .to_string(),
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
                    self.log(format!("\x1b[90m  -> New Strategy: {}\x1b[0m", h));
                }
            }
            sleep(Duration::from_secs(2)).await;
        }
        self.log("\x1b[1;32m>> [CORTEX] Epistemic Execution Cycle Complete.\x1b[0m".to_string());
        let final_evidence = best_h_id
            .map(|id| self.collect_evidence(id))
            .unwrap_or_else(|| "No evidence gathered across any turn.".to_string());
        Ok(CortexOutcome::TurnsExhausted { best_belief: max_observed_belief, evidence: final_evidence })
    }
}


// v1.9: Prefer BeliefState::may_prove / BeliefVector::may_prove before
// treating any node as proven. Example:
//   if belief.may_prove(1, 0.75) { /* complete node */ }
