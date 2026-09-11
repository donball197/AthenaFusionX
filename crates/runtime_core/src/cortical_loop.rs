#![allow(clippy::too_many_arguments)]
#![allow(unreachable_code)]
use anyhow::Result;
use reqwest::Client;
use std::fs;
use std::io::Write;
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};

use crate::action_gate::ActionGate;
use crate::agent_protocol::AgentAction;
use crate::anti_loop::{score_search_relevance, should_raise_belief};
use crate::cognition::epistemic_session::EpistemicSession;
use crate::cognition_graph::{EdgeRelation, NodeKind};
use crate::contract_status::{ContractEvaluator, ContractStatus};
use crate::distributed_scheduler::DistributedScheduler;
use crate::epistemic_engine::EpistemicEngine;
use crate::executor::RoboticExecutor;
use crate::intent_contract::IntentContract;
use crate::memory_cortex::MemoryCortex;
use crate::motor_cortex::MotorCortex;
use crate::policy::Capability;
use crate::shared_state::VmoState;
use crate::world_model::WorldModel;

#[derive(Debug)]
pub enum CortexOutcome {
    CompletionProposed { belief: f32, evidence: String },
    TurnsExhausted { best_belief: f32, evidence: String },
}

enum CortexState {
    Strategize,
    Critique {
        actions: Vec<AgentAction>,
        proposal: String,
        belief: f32,
    },
    Execute {
        actions: Vec<AgentAction>,
        proposal: String,
        belief: f32,
    },
    Evaluate {
        evidence: String,
        proposal: String,
        executed_action: String,
        belief: f32,
        is_halt_action: bool,
        mutation_path: Option<String>,
    },
    Complete {
        belief: f32,
        evidence: String,
    },
    Fail {
        belief: f32,
        evidence: String,
    },
}

pub struct CorticalLoop {
    client: Client,
    api_key: String,
    world_model: Arc<WorldModel>,
    memory: Arc<MemoryCortex>,
    vmo_state: Arc<Mutex<VmoState>>,
    dist: Arc<DistributedScheduler>,
    tx: broadcast::Sender<String>,
    action_gate: ActionGate,
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
            world_model,
            memory,
            vmo_state,
            dist,
            tx,
            action_gate: ActionGate::with_capabilities(vec![
                Capability::ReadSubstrate,
                Capability::WriteSubstrate,
                Capability::HaltCognition,
                Capability::ExecuteSystemCall,
            ]),
        }
    }

    fn log(&self, msg: String) {
        let _ = self.tx.send(
            serde_json::json!({"type": "NEURAL_STREAM", "payload": format!("{}\n", msg)})
                .to_string(),
        );
    }

    fn dump_debug(&self, turn: u32, phase: &str, payload: &str) {
        let dir = ".nexus_debug";
        let _ = fs::create_dir_all(dir);
        let filepath = format!("{}/turn_{}_{}.txt", dir, turn, phase);
        if let Ok(mut file) = fs::File::create(&filepath) {
            let _ = file.write_all(payload.as_bytes());
        }
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
                        format!("WORKSPACE FACTS: workspace_root={workspace_root} | target_dir={target_dir}.")
                    }
                    Err(e) => format!("WORKSPACE FACTS: cargo metadata parse failed ({e})."),
                }
            }
            Ok(output) => {
                format!("WORKSPACE FACTS: cargo metadata exited non-zero ({}).", output.status)
            }
            Err(e) => format!("WORKSPACE FACTS: cargo metadata failed to run ({e})."),
        }
    }

    fn compress_context(ctx: &str, max_chars: usize) -> String {
        if ctx.len() <= max_chars {
            return ctx.to_string();
        }
        let chunk_size = max_chars / 2;
        let start = &ctx[..chunk_size];
        let end = &ctx[ctx.len() - chunk_size..];
        format!(
            "{}...\n\n[... {} CHARACTERS COMPRESSED ...]\n\n{}",
            start,
            ctx.len() - max_chars,
            end
        )
    }

    pub async fn ignite(&mut self, mission: &str) -> Result<CortexOutcome> {
        self.ignite_with_evidence(mission, None).await
    }

    pub async fn ignite_with_evidence(
        &mut self,
        mission: &str,
        ancestor_evidence: Option<&str>,
    ) -> Result<CortexOutcome> {
        self.log("\x1b[1;36m>> [CORTEX] Igniting Epistemic Reasoning Session\x1b[0m".to_string());

        let stage_start = std::time::Instant::now();
        let mut context_chunks = self.world_model.semantic_query(mission).await;
        self.log(format!(
            "[TIMING] WORLD_MODEL semantic_query={}ms",
            stage_start.elapsed().as_millis()
        ));

        let stage_start = std::time::Instant::now();
        context_chunks.push(Self::resolve_workspace_paths().await);
        self.log(format!(
            "[TIMING] WORKSPACE_PATHS cargo_metadata={}ms",
            stage_start.elapsed().as_millis()
        ));

        // ============================================================
        // V5 FUSION: AUTOMATIC EXPERIENCE RECALL
        // ============================================================
        //
        // Prior verified experience is supplied to reasoning as ADVISORY
        // context. It cannot authorize completion or bypass verification.
        let stage_start = std::time::Instant::now();
        self.log("[TIMING] MEMORY_RECALL start".to_string());

        match nexus_substrate::vector_bridge::get_embedding(&self.api_key, mission).await {
            Ok(query_embedding) => {
                match self.memory.recall_semantic_context(&query_embedding, mission, 6).await {
                    Ok(recall) if !recall.memories.is_empty() => {
                        let mut memory_context =
                            String::from("\n[RECALLED EXPERIENCE — ADVISORY ONLY]\n");

                        for memory in recall.memories {
                            memory_context
                                .push_str(&format!("- [{}] {}\n", memory.source, memory.content));
                        }

                        memory_context.push_str("[END RECALLED EXPERIENCE]\n");

                        context_chunks.push(memory_context);

                        self.log(format!(
                            "[FUSION] Recalled prior experience; relevance confidence {:.2}",
                            recall.confidence
                        ));
                    }

                    Ok(_) => {
                        self.log(
                            "[FUSION] No prior experience relevant to this mission.".to_string(),
                        );
                    }

                    Err(error) => {
                        self.log(format!(
                            "[TIMING] MEMORY_RECALL failed after {}ms",
                            stage_start.elapsed().as_millis()
                        ));

                        // Memory is never a hard dependency for execution.
                        self.log(format!(
                            "[FUSION] Memory recall unavailable; continuing without recalled experience: {}",
                            error
                        ));
                    }
                }

                self.log(format!(
                    "[TIMING] MEMORY_RECALL completed in {}ms",
                    stage_start.elapsed().as_millis()
                ));
            }

            Err(error) => {
                self.log(format!(
                    "[FUSION] Memory query embedding unavailable; continuing without recalled experience: {}",
                    error
                ));
            }
        }

        if let Some(evidence) = ancestor_evidence {
            if !evidence.trim().is_empty() {
                context_chunks.push(format!("\n[ANCESTOR NODE EVIDENCE]\n{}", evidence));
            }
        }
        let world_context = Self::compress_context(&context_chunks.join("\n"), 2500);

        let mut session = EpistemicSession::new(mission);

        if let Some(evidence) = ancestor_evidence {
            if !evidence.trim().is_empty() {
                session.local_evidence.push_str("\n[DAG LINEAGE INHERITED EVIDENCE]\n");
                session.local_evidence.push_str(evidence.trim());
                session.local_evidence.push_str("\n[DAG LINEAGE COMPLETE]\n");
            }
        }

        self.log("\x1b[36m>> [CORTEX] Generating competing strategies...\x1b[0m".to_string());
        session.budget.record_call();
        let hypotheses = EpistemicEngine::generate_hypotheses(
            &self.client,
            &self.api_key,
            mission,
            &world_context,
        )
        .await?;

        for h in hypotheses {
            let hid = session.graph.add_node(NodeKind::Hypothesis, &h, 0.5, 100);
            if session.best_hypothesis_id.is_none() {
                session.best_hypothesis_id = Some(hid);
            }
            self.log(format!("\x1b[90m  -> Strategy: {}\x1b[0m", h));
        }

        let is_mutation_task = session.contract.is_mutation_task
            || mission.to_lowercase().contains("synthesize_report")
            || mission.to_lowercase().contains(".md");
        let is_write_task = is_mutation_task
            || session.contract.objective.to_lowercase().contains("write")
            || session.contract.objective.to_lowercase().contains("export")
            || session.contract.objective.to_lowercase().contains(".md")
            || session.contract.objective.to_lowercase().contains(".json")
            || mission.to_lowercase().contains("synthesize_report");

        let mut state = CortexState::Strategize;
        let mut turn = 0;
        let mut shift_rx = self.dist.file_shift_tx.subscribe();

        loop {
            turn += 1;
            self.log(format!(
                "\n\x1b[1;33m>> [CORTEX] Cognitive Turn {} / {}\x1b[0m",
                turn, session.budget.max_turns
            ));

            // Active code-shift listener
            while let Ok(event) = shift_rx.try_recv() {
                self.log(format!(
                    "\x1b[1;33m>> [CODE-SHIFT] Concurrent mutation detected on `{}` via {}. Invalidating session cache.\x1b[0m",
                    event.path, event.modifier
                ));
                let shift_obs = format!(
                    "[CODE_SHIFT_ALERT] File `{}` was just modified externally. You MUST re-read this file before proposing edits.",
                    event.path
                );
                session.local_evidence.push_str(&format!("\n{}\n", shift_obs));
                if let Some(h_id) = session.best_hypothesis_id {
                    let obs_id = session.graph.add_node(NodeKind::Observation, &shift_obs, 1.0, 85);
                    session.graph.add_edge(obs_id, h_id, EdgeRelation::Contradicts, 0.40);
                    session.graph.propagate_beliefs();
                }
            }

            if let Some(best_node) = session.graph.select_best_hypothesis().cloned() {
                let current_belief = best_node.belief_score;

                // Deterministic physical evidence is authoritative.
                // Check the completion contract before allowing the cognitive
                // budget guard to terminate an already-completed subgoal.
                let deterministic_contract_satisfied =
                    crate::completion_engine::SubgoalCompletionEngine::evaluate(
                        &session.contract,
                        &session.local_evidence,
                        current_belief,
                    );

                if deterministic_contract_satisfied {
                    self.log(
                        "\x1b[1;36m>> [CORTEX TERMINAL] Physical success contract already satisfied. No additional cognitive turn required.\x1b[0m"
                            .to_string(),
                    );

                    return Ok(CortexOutcome::CompletionProposed {
                        belief: current_belief.max(0.95),
                        evidence: session.collect_evidence(best_node.id),
                    });
                }

                if session.budget.is_exhausted(turn) {
                    if current_belief >= 0.80
                        && session.consecutive_mutation_vetoes < 2
                        && turn < 80
                    {
                        self.log(format!("\x1b[1;35m>> [COGNITIVE MOMENTUM] Call limit reached with high belief ({:.2}). Extending budget.\x1b[0m", current_belief));
                        session.budget.max_turns += 10;
                        session.budget.max_model_calls += 30;
                    } else {
                        self.log("\x1b[1;31m>> [SUBGOAL_BUDGET_EXHAUSTED] Cognitive call limit reached. Node finalized.\x1b[0m".to_string());
                        return Ok(CortexOutcome::TurnsExhausted {
                            best_belief: current_belief,
                            evidence: session.collect_evidence(best_node.id),
                        });
                    }
                }
            } else if session.budget.is_exhausted(turn) {
                return Ok(CortexOutcome::TurnsExhausted {
                    best_belief: 0.0,
                    evidence: "No viable hypotheses left.".to_string(),
                });
            }

            state = match state {
                CortexState::Strategize => {
                    self.handle_strategize(
                        turn,
                        &mut session,
                        mission,
                        &world_context,
                        is_mutation_task,
                        is_write_task,
                    )
                    .await?
                }
                CortexState::Critique { actions, proposal, belief } => {
                    self.handle_critic(
                        turn,
                        &mut session,
                        mission,
                        &world_context,
                        actions,
                        proposal,
                        belief,
                        is_mutation_task,
                        is_write_task,
                    )
                    .await?
                }
                CortexState::Execute { actions, proposal, belief } => {
                    self.handle_execute(&mut session, actions, proposal, belief).await?
                }
                CortexState::Evaluate {
                    evidence,
                    proposal,
                    executed_action,
                    belief,
                    is_halt_action,
                    mutation_path,
                } => {
                    self.handle_evaluate(
                        &mut session,
                        mission,
                        evidence,
                        proposal,
                        executed_action,
                        belief,
                        is_mutation_task,
                        is_write_task,
                        is_halt_action,
                        mutation_path,
                    )
                    .await?
                }
                CortexState::Complete { belief, evidence } => {
                    return Ok(CortexOutcome::CompletionProposed { belief, evidence })
                }
                CortexState::Fail { belief, evidence } => {
                    return Ok(CortexOutcome::TurnsExhausted { best_belief: belief, evidence })
                }
            };
        }
    }

    /// Deterministically enforce the Cortex strategy/action boundary.
    ///
    /// The Cortex selects the active strategy. The LLM Strategist may
    /// operationalize that strategy, but may not silently substitute a
    /// different native action.
    fn action_matches_strategy(strategy: &str, action: &AgentAction) -> bool {
        let strategy = strategy.trim();

        if let Some(expected_path) = strategy.strip_prefix("read_file:") {
            return matches!(
                action,
                AgentAction::ReadFile { path, .. }
                    if path.trim() == expected_path.trim()
            );
        }

        if let Some(rest) = strategy.strip_prefix("read_function:") {
            let mut parts = rest.splitn(2, ':');
            let expected_path = parts.next().unwrap_or("").trim();
            let expected_function = parts.next().unwrap_or("").trim();

            return matches!(
                action,
                AgentAction::ReadFunction {
                    path,
                    function_name,
                    ..
                } if path.trim() == expected_path
                    && function_name.trim() == expected_function
            );
        }

        if let Some(expected_path) = strategy.strip_prefix("read_file_chunk:") {
            return matches!(
                action,
                AgentAction::ReadFileChunk { path, .. }
                    if path.trim() == expected_path.trim()
            );
        }

        if let Some(rest) = strategy.strip_prefix("search_code:") {
            let mut parts = rest.splitn(2, ':');
            let expected_path = parts.next().unwrap_or("").trim();
            let expected_query = parts.next().unwrap_or("").trim();

            return matches!(
                action,
                AgentAction::SearchCode { path, query, .. }
                    if path.trim() == expected_path
                    && query.trim() == expected_query
            );
        }

        if strategy.strip_prefix("search_memory:").is_some() {
            // NOTE: search_memory strategies carry the full authoritative
            // directive text (see EpistemicEngine::generate_hypotheses),
            // not a literal query contract like search_code's path/query
            // or patch_function's path/function_name. Requiring exact
            // string equality here is unsatisfiable by construction --
            // no model reproduces the full node prompt verbatim as its
            // query -- and previously caused permanent STRATEGY VIOLATION
            // loops on every legitimate SearchMemory proposal. Only the
            // action *type* is authoritative; the query text is free-form.
            return matches!(action, AgentAction::SearchMemory { .. });
        }

        if let Some(expected_command) = strategy.strip_prefix("execute_command:") {
            return matches!(
                action,
                AgentAction::ExecuteCommand { command, .. }
                    if command.trim() == expected_command.trim()
            );
        }

        if let Some(expected_path) = strategy.strip_prefix("write_file:") {
            return matches!(
                action,
                AgentAction::WriteFile { path, .. }
                    if path.trim() == expected_path.trim()
            );
        }

        if let Some(expected_path) = strategy.strip_prefix("replace_lines:") {
            return matches!(
                action,
                AgentAction::ReplaceLines { path, .. }
                    if path.trim() == expected_path.trim()
            );
        }

        if let Some(expected_path) = strategy.strip_prefix("diff_patch:") {
            return matches!(
                action,
                AgentAction::DiffPatch { path, .. }
                    if path.trim() == expected_path.trim()
            );
        }

        if let Some(rest) = strategy.strip_prefix("patch_function:") {
            let mut parts = rest.splitn(2, ':');
            let expected_path = parts.next().unwrap_or("").trim();
            let expected_function = parts.next().unwrap_or("").trim();

            return matches!(
                action,
                AgentAction::PatchFunction {
                    path,
                    function_name,
                    ..
                } if path.trim() == expected_path
                    && function_name.trim() == expected_function
            );
        }

        if let Some(expected_path) = strategy.strip_prefix("synthesize_report:") {
            return matches!(
                action,
                AgentAction::SynthesizeReport { output_path, .. }
                    if output_path.trim() == expected_path.trim()
            );
        }

        // Strategies that are not explicit native actions are reasoning
        // descriptions. They are not subject to a parameter-level binding.
        true
    }

    async fn handle_strategize(
        &mut self,
        turn: u32,
        session: &mut EpistemicSession,
        mission: &str,
        world_context: &str,
        is_mutation_task: bool,
        is_write_task: bool,
    ) -> Result<CortexState> {
        let best_node = match session.graph.select_best_hypothesis().cloned() {
            Some(node) => node,
            None => {
                return Ok(CortexState::Fail {
                    belief: 0.0,
                    evidence: "No viable hypotheses left.".to_string(),
                })
            }
        };

        let h_id = best_node.id;
        let h_content = best_node.content.clone();
        let current_belief = best_node.belief_score;
        session.best_hypothesis_id = Some(h_id);

        self.log(format!(
            "\x1b[32m>> [CORTEX] Exploring Strategy (Belief: {:.2}):\x1b[0m {}",
            current_belief, h_content
        ));

        // 1. Evaluate the contract, but NEVER turn artifact existence into
        // unconditional node completion.
        //
        // WRITE_OK means the write action succeeded. The enclosing node may
        // still require reads, searches, source evidence, analysis, and/or
        // epistemic verification.
        let status = ContractEvaluator::evaluate(
            is_mutation_task,
            is_write_task,
            &session.local_evidence,
            current_belief,
            mission,
        );

        if status == ContractStatus::Satisfied {
            let command_only_contract = session.contract.conditions.len() == 1
                && matches!(
                    session.contract.conditions.first(),
                    Some(
                        crate::completion_engine::CompletionCondition::CommandExecutionVerified { .. }
                    )
                );

            if command_only_contract {
                self.log(
                    "\x1b[1;36m>> [CORTEX TERMINAL] Pure command contract satisfied by physical CMD_OK evidence. No additional model turn required. Node complete.\x1b[0m"
                        .to_string(),
                );

                return Ok(CortexState::Complete {
                    belief: current_belief.max(0.95),
                    evidence: session.collect_evidence(h_id),
                });
            }

            self.log(
                "\x1b[1;33m>> [CORTEX] Local contract evidence is satisfied. Continuing through normal completion verification.\x1b[0m"
                    .to_string(),
            );
        }

        let evidence_history = Self::compress_context(&session.collect_evidence(h_id), 3000);

        // Phase-aware mandate:
        //
        // A generic recon marker is NOT sufficient to enter the write phase.
        // The deterministic contract must prove every non-output condition
        // first: reads, searches, source evidence, and belief threshold.
        //
        // Only after that gate passes may the strategist be encouraged
        // to produce the required deliverable.

        let mission_contract =
            crate::completion_engine::SubgoalCompletionEngine::infer_contract("ad_hoc", mission);

        let unwritten_targets: Vec<&str> = mission_contract
            .required_artifact_paths
            .iter()
            .map(|a| a.path.as_str())
            .filter(|p| !crate::evidence_markers::has_artifact_write(&session.local_evidence, p))
            .collect();

        let recon_ready =
            crate::completion_engine::SubgoalCompletionEngine::reconnaissance_complete(
                &mission_contract,
                &session.local_evidence,
                current_belief,
            );

        // An outstanding artifact may enter the write phase only after the
        // deterministic reconnaissance contract is actually satisfied.
        // A single successful command is evidence for that command only; it
        // must never bypass the remaining reconnaissance conditions.
        let write_phase_ready = !unwritten_targets.is_empty() && recon_ready;

        // DETERMINISTIC PHASE TRANSITION:
        //
        // Once reconnaissance is complete, an outstanding artifact must be
        // produced through the native write boundary. Do not merely tell the
        // Strategist that write_file is available while continuing to pass
        // the old execute_command hypothesis as the authoritative strategy.
        //
        // This is intentionally upstream of EpistemicEngine::propose_action().
        // That function already force-selects the native tool for explicit
        // strategies, so the Cortex must supply the correct strategy here.
        let active_strategy = if write_phase_ready {
            let target = unwritten_targets[0];
            let strategy = format!("write_file:{}", target);

            self.log(format!(
                "\x1b[1;35m>> [CORTEX PHASE TRANSITION]\x1b[0m Reconnaissance verified; forcing native deliverable strategy `{}`.",
                strategy
            ));

            strategy
        } else {
            h_content.clone()
        };

        let enhanced_mission = if write_phase_ready {
            format!(
                "{}\n\n[PHASE MANDATE: RECONNAISSANCE VERIFIED]\nThe deterministic reconnaissance contract has been satisfied. The required deliverable MUST now be produced at '{}' using the native `write_file` action.",
                mission,
                unwritten_targets[0]
            )
        } else {
            mission.to_string()
        };
        session.budget.record_call();

        let raw_proposal = match EpistemicEngine::propose_action(
            &self.client,
            &self.api_key,
            &enhanced_mission,
            &active_strategy,
            world_context,
            &evidence_history,
        )
        .await
        {
            Ok(p) => p,
            Err(e) => {
                self.log(format!("\x1b[31m>> [STRATEGIST FAULT]\x1b[0m {}", e));
                return Ok(CortexState::Strategize);
            }
        };

        self.dump_debug(turn, "strategist_output", &raw_proposal);
        self.log(format!("\x1b[35m>> [STRATEGIST PROPOSAL]:\x1b[0m {}", raw_proposal));

        // 3. Immediately Extract Typed Actions from Proposal
        let mut actions = crate::json_parser::extract_actions(&raw_proposal).unwrap_or_default();
        if actions.is_empty() {
            if let Some(intercepted) =
                crate::action_intercept::intercept_fallback_action(&raw_proposal)
            {
                actions.push(intercepted);
            }
        }

        if actions.is_empty() {
            let prop_trim = raw_proposal.trim().to_lowercase();
            if prop_trim.contains("halt") {
                actions.push(AgentAction::Halt {
                    reasoning: "Model declared task complete".to_string(),
                });
            }
        }

        if actions.is_empty() {
            self.log(
                "\x1b[31m>> [CORTEX FAULT] Could not extract valid action from proposal.\x1b[0m"
                    .to_string(),
            );
            return Ok(CortexState::Strategize);
        }

        // HARD CORTEX BOUNDARY:
        // The first executable action must operationalize the strategy
        // selected by the Cortex. The Strategist is not allowed to silently
        // replace the selected strategy with an independently chosen action.
        //
        // EXPLICIT SEARCHMEMORY CONTRACT:
        // When the original directive explicitly names SearchMemory and
        // contains a semantic target introduced by "recall prior memory
        // specifically about", that target is user-authoritative. The
        // Strategist may choose the native action and provide reasoning,
        // but it must not compress the requested semantic query into a
        // generic substitute such as "host metrics".
        if h_content.starts_with("search_memory:") {
            if let Some(authoritative_query) = Self::extract_explicit_search_memory_query(mission) {
                if let Some(AgentAction::SearchMemory { query, .. }) = actions.first_mut() {
                    if query.trim() != authoritative_query {
                        self.log(format!(
                            "[CORTEX] Preserving authoritative SearchMemory query: {}",
                            authoritative_query
                        ));
                        *query = authoritative_query;
                    }
                }
            }
        }

        if let Some(first_action) = actions.first() {
            if !Self::action_matches_strategy(&h_content, first_action) {
                self.log(format!(
                    "\x1b[1;31m>> [STRATEGY VIOLATION]\x1b[0m Selected strategy '{}' produced incompatible action '{:?}'. Rejecting proposal and forcing re-strategy.",
                    h_content,
                    first_action
                ));

                // OPERATIONAL FAILURE IS NOT EPISTEMIC CONTRADICTION.
                //
                // The Strategist failed to operationalize the selected
                // strategy. That does NOT prove the underlying hypothesis
                // false, so preserve its epistemic belief.
                let violation_obs = format!(
                    "[STRATEGY_VIOLATION] Selected strategy `{}` \
                     produced an incompatible action. Proposal rejected; \
                     underlying hypothesis remains unchanged.",
                    h_content
                );

                session.local_evidence.push_str(&violation_obs);
                session.local_evidence.push('\n');

                self.log(
                    "[STRATEGY RECOVERY] Operational proposal rejected; \
                     hypothesis belief preserved. Re-entering Strategize."
                        .to_string(),
                );

                return Ok(CortexState::Strategize);
            }
        }

        Ok(CortexState::Critique { actions, proposal: raw_proposal, belief: current_belief })
    }

    fn extract_explicit_search_memory_query(mission: &str) -> Option<String> {
        let lower = mission.to_lowercase();

        let marker = "recall prior memory specifically about";
        let start = lower.find(marker)? + marker.len();

        let remainder = mission.get(start..)?.trim_start();

        let query = remainder
            .split_once('.')
            .map(|(q, _)| q)
            .unwrap_or(remainder)
            .trim()
            .trim_end_matches(':')
            .trim();

        if query.is_empty() {
            None
        } else {
            Some(query.to_string())
        }
    }

    async fn handle_critic(
        &mut self,
        turn: u32,
        session: &mut EpistemicSession,
        mission: &str,
        world_context: &str,
        actions: Vec<AgentAction>,
        proposal: String,
        belief: f32,
        is_mutation_task: bool,
        is_write_task: bool,
    ) -> Result<CortexState> {
        let status = ContractEvaluator::evaluate(
            is_mutation_task,
            is_write_task,
            &session.local_evidence,
            belief,
            mission,
        );

        if status == ContractStatus::Satisfied {
            self.log(
                "\x1b[1;33m>> [SUBGOAL_COMPLETE] Non-mutating subgoal satisfied. Mission remains active.\x1b[0m"
                    .to_string(),
            );
        }

        let h_content = if let Some(h_id) = session.best_hypothesis_id {
            session.graph.nodes.get(&h_id).map(|n| n.content.clone()).unwrap_or_default()
        } else {
            String::new()
        };

        // Check Anti-Loop on actions before calling Critic.
        // Filter duplicates individually -- a single stale action in a
        // multi-action proposal must not discard sibling actions that
        // have never crossed the execution boundary.
        let mut filtered_actions: Vec<AgentAction> = Vec::with_capacity(actions.len());
        let mut vetoed_keys: Vec<String> = Vec::new();

        for action in actions {
            let action_key = match &action {
                AgentAction::ReadFile { path, .. } => format!("read_file:{}", path),
                AgentAction::ReadFunction { path, function_name, .. } => {
                    format!("read_function:{}:{}", path, function_name)
                }
                AgentAction::ReadFileChunk { path, start_line, .. } => {
                    format!("read_file_chunk:{}:{}", path, start_line)
                }
                AgentAction::SearchCode { path, query, .. } => {
                    format!("search_code:{}:{}", path, query)
                }
                AgentAction::SearchMemory { query, .. } => {
                    format!("search_memory:{}", query)
                }
                AgentAction::WriteFile { path, content, .. } => {
                    use std::hash::{Hash, Hasher};
                    let mut hasher = std::collections::hash_map::DefaultHasher::new();
                    content.hash(&mut hasher);
                    format!("write_file:{}:{:016x}", path, hasher.finish())
                }
                AgentAction::DiffPatch { path, .. } => format!("diff_patch:{}", path),
                AgentAction::PatchFunction { path, function_name, .. } => {
                    format!("patch_function:{}:{}", path, function_name)
                }
                AgentAction::SynthesizeReport { output_path, .. } => {
                    format!("synthesize_report:{}", output_path)
                }
                AgentAction::ExecuteCommand { command, .. } => {
                    format!("execute_command:{}", command.trim())
                }
                _ => format!("{:?}", action),
            };

            let attempt_count = session.global_action_attempt_count(&action_key);

            // STRICT ANTI-LOOP:
            // An identical action that has already crossed the execution
            // boundary is not new evidence. Drop it from the batch, but
            // let never-attempted sibling actions in the same proposal
            // proceed -- one stale action must not starve the rest.
            if attempt_count >= 1 {
                self.log(format!(
                    "\x1b[1;31m>> [ANTI-LOOP] Duplicate action [{}] filtered from batch.\x1b[0m",
                    action_key
                ));
                vetoed_keys.push(action_key);
            } else {
                filtered_actions.push(action);
            }

            // IMPORTANT:
            // Do NOT count the action here. This is the critic stage.
            // The counter will be recorded only after the action passes
            // the ActionGate and is actually sent to the executor.
        }

        if filtered_actions.is_empty() {
            // Every action in the batch was a duplicate. Returning to
            // Strategize alone does nothing here: belief is normally only
            // updated after real evidence comes back from execution, so
            // the current hypothesis would just be re-selected as "best"
            // again next turn, unchanged. Apply a real belief penalty so
            // select_best_hypothesis (which requires belief_score >= 0.15)
            // is eventually forced to promote a different hypothesis.
            if let Some(h_id) = session.best_hypothesis_id {
                if let Some(node) = session.graph.nodes.get_mut(&h_id) {
                    node.belief_score = (node.belief_score - 0.20).max(0.0);
                    self.log(format!(
                        "\x1b[1;31m>> [ANTI-LOOP] Repeated stale proposal -- decaying hypothesis belief to {:.2}\x1b[0m",
                        node.belief_score
                    ));
                }
            }

            let loop_obs = format!(
                "[LOOP_VETO] All proposed actions already attempted: {}. \
                 Must choose different action(s).",
                vetoed_keys.join(", ")
            );
            session.local_evidence.push_str(&loop_obs);
            session.local_evidence.push('\n');

            self.log(
                "[ANTI-LOOP RECOVERY] All actions in batch were duplicates; \
                 hypothesis belief decayed to force pivot. Re-entering Strategize."
                    .to_string(),
            );

            return Ok(CortexState::Strategize);
        }

        if !vetoed_keys.is_empty() {
            let loop_obs = format!(
                "[LOOP_VETO] Dropped already-attempted action(s) from batch: {}. \
                 Proceeding with remaining valid action(s).",
                vetoed_keys.join(", ")
            );
            session.local_evidence.push_str(&loop_obs);
            session.local_evidence.push('\n');
        }

        let actions = filtered_actions;

        self.log("\x1b[31m>> [RED TEAM] Criticizing proposal...\x1b[0m".to_string());
        session.budget.record_call();

        match EpistemicEngine::criticize_proposal(
            &self.client,
            &self.api_key,
            mission,
            &h_content,
            &proposal,
            world_context,
            &session.local_evidence,
        )
        .await
        {
            Err(e) => {
                self.log(format!("\x1b[33m>> [CRITIC BYPASS] Parser error ({}). Proceeding with execution.\x1b[0m", e));
                Ok(CortexState::Execute { actions, proposal, belief })
            }
            Ok(critic) => {
                self.dump_debug(turn, "critic_output", &format!("{:?}", critic));
                self.log(format!(
                    "\x1b[31m>> [CRITIC] Verdict:\x1b[0m {} (Modifier: {:.2})",
                    critic.critique, critic.confidence_modifier
                ));

                if critic.confidence_modifier > -0.25 {
                    Ok(CortexState::Execute { actions, proposal, belief })
                } else {
                    self.log("\x1b[33m>> [DIALECTIC FEEDBACK] Veto registered in graph. Re-strategizing...\x1b[0m".to_string());
                    if let Some(h_id) = session.best_hypothesis_id {
                        let obs_id = session.graph.add_node(
                            NodeKind::Observation,
                            &format!("[CRITIC_VETO] {}", critic.critique),
                            1.0,
                            75,
                        );
                        session.graph.add_edge(obs_id, h_id, EdgeRelation::Contradicts, 0.40);
                        session.graph.propagate_beliefs();
                    }
                    Ok(CortexState::Strategize)
                }
            }
        }
    }

    async fn handle_execute(
        &mut self,
        session: &mut EpistemicSession,
        actions: Vec<AgentAction>,
        proposal: String,
        belief: f32,
    ) -> Result<CortexState> {
        let mut executor = RoboticExecutor::new(self.api_key.clone(), self.tx.clone());
        let mut latest_evidence = String::new();
        let mut executed_actions: Vec<String> = Vec::new();
        let mut mutation_paths: Vec<String> = Vec::new();
        let mut last_is_halt = false;
        let mut rust_files_mutated = false;
        let evidence_checkpoint = session.local_evidence.len();

        for action in actions {
            let intent = IntentContract::new(&proposal, belief);
            let capability = match &action {
                AgentAction::ExecuteCommand { .. } => Capability::ExecuteSystemCall,
                AgentAction::WriteFile { .. }
                | AgentAction::DiffPatch { .. }
                | AgentAction::PatchFunction { .. }
                | AgentAction::SynthesizeReport { .. } => Capability::WriteSubstrate,
                AgentAction::ScrapeUrl { .. } | AgentAction::ReadFunction { .. } => {
                    Capability::ReadSubstrate
                }
                AgentAction::Halt { .. } => Capability::HaltCognition,
                _ => Capability::ReadSubstrate,
            };

            if let AgentAction::AtomicPatch { patches, .. } = &action {
                for p in patches {
                    mutation_paths.push(p.path.clone());
                    if p.path.ends_with(".rs") {
                        rust_files_mutated = true;
                    }
                }
            }

            if let AgentAction::ExecuteCommand { command, .. } = &action {
                // Defensive: an arbitrary shell command (e.g. invoking an external
                // tool like nexus_ast_tool) can mutate a .rs file without going
                // through any recognized AgentAction write variant. If the command
                // string references a .rs path, treat it as a possible Rust
                // mutation so the compiler-critic gate below still runs -- silent
                // corruption from an external tool must not skip verification
                // just because it didn't arrive as a WriteFile/DiffPatch/etc.
                for token in command.split_whitespace() {
                    let clean = token.trim_matches(|c: char| {
                        !c.is_alphanumeric() && c != '.' && c != '/' && c != '_' && c != '-'
                    });
                    if clean.ends_with(".rs") {
                        mutation_paths.push(clean.to_string());
                        rust_files_mutated = true;
                    }
                }
            }
            let mutation_path = match &action {
                AgentAction::WriteFile { path, .. }
                | AgentAction::DiffPatch { path, .. }
                | AgentAction::PatchFunction { path, .. } => Some(path.clone()),
                AgentAction::SynthesizeReport { output_path, .. } => Some(output_path.clone()),
                _ => None,
            };

            if let Some(path) = mutation_path {
                mutation_paths.push(path.clone());
                if path.ends_with(".rs") {
                    rust_files_mutated = true;
                }
            }

            if matches!(action, AgentAction::Halt { .. }) {
                last_is_halt = true;
            }

            // Serialize the exact AgentAction before MotorCortex::translate()
            // consumes it. This is the authoritative action record that will
            // cross the execution/evaluation boundary.
            let executed_action_json = serde_json::to_string(&action)
                .map_err(|e| anyhow::anyhow!("Failed to serialize executed AgentAction: {}", e))?;

            // Build the canonical action key before MotorCortex::translate()
            // consumes the AgentAction.
            let action_key = match &action {
                AgentAction::ReadFile { path, .. } => format!("read_file:{}", path),
                AgentAction::ReadFunction { path, function_name, .. } => {
                    format!("read_function:{}:{}", path, function_name)
                }
                AgentAction::ReadFileChunk { path, start_line, .. } => {
                    format!("read_file_chunk:{}:{}", path, start_line)
                }
                AgentAction::SearchCode { path, query, .. } => {
                    format!("search_code:{}:{}", path, query)
                }
                AgentAction::WriteFile { path, content, .. } => {
                    use std::hash::{Hash, Hasher};
                    let mut hasher = std::collections::hash_map::DefaultHasher::new();
                    content.hash(&mut hasher);
                    format!("write_file:{}:{:016x}", path, hasher.finish())
                }
                AgentAction::DiffPatch { path, .. } => format!("diff_patch:{}", path),
                AgentAction::PatchFunction { path, function_name, .. } => {
                    format!("patch_function:{}:{}", path, function_name)
                }
                AgentAction::SynthesizeReport { output_path, .. } => {
                    format!("synthesize_report:{}", output_path)
                }
                AgentAction::ExecuteCommand { command, .. } => {
                    format!("execute_command:{}", command.trim())
                }
                _ => format!("{:?}", action),
            };

            let motor_cmd =
                MotorCortex::translate(&intent, None, action, capability, proposal.clone());

            if let Err(gate_err) = self.action_gate.approve(&motor_cmd) {
                self.log(format!("\x1b[1;31m>> [ACTION GATE REJECTED]: {}\x1b[0m", gate_err));
                if let Some(h_id) = session.best_hypothesis_id {
                    let obs_id = session.graph.add_node(
                        NodeKind::Observation,
                        &format!("[ACTION_GATE_BLOCKED] {}", gate_err),
                        1.0,
                        50,
                    );
                    session.graph.add_edge(obs_id, h_id, EdgeRelation::Contradicts, 0.50);
                    session.graph.propagate_beliefs();
                }
                continue;
            }

            // The ActionGate has approved this exact action.
            // Record both:
            //   1. the canonical anti-loop attempt
            //   2. the exact serialized action crossing the physical boundary
            //
            // IMPORTANT: this happens AFTER the gate and immediately before
            // execution. A blocked action is therefore never presented to the
            // evaluator as executed.
            session.record_global_action_attempt(&action_key);
            executed_actions.push(executed_action_json);

            self.log("\x1b[35m>> [MOTOR CORTEX] Emitting authorized MotorCommand to RoboticExecutor...\x1b[0m".to_string());
            let evidence = match executor
                .execute_motor_command(
                    motor_cmd,
                    self.memory.clone(),
                    self.vmo_state.clone(),
                    self.dist.clone(),
                )
                .await
            {
                Ok(ev) => ev,
                Err(e) => format!("[ACTION FAILED] OS rejected action: {}", e),
            };

            latest_evidence.push_str(&evidence);
            latest_evidence.push('\n');
            session.local_evidence.push_str(&evidence);
            session.local_evidence.push('\n');

            let preview = evidence.lines().take(5).collect::<Vec<_>>().join("\n");
            self.log(format!("\x1b[32m>> [EVIDENCE ACQUIRED]\x1b[0m\n{}", preview));
        }

        // If an action failed with PATH_NOT_FOUND, prevent re-attempting that path.
        //
        // GUARD: this must not fire on evidence that IS a successful recon
        // result (e.g. SearchMemory/SearchCode/Read _OK] markers). Retrieved
        // document content can legitimately contain the literal string
        // "PATH_NOT_FOUND" as prose (e.g. an audit doc describing this
        // codebase's own error markers) without the action itself having
        // failed. Confirmed root cause of a false -0.90 Contradicts edge
        // permanently poisoning belief on a genuinely successful
        // SearchMemory hit (see BELIEF TRACE: edges=3->4, delta -0.85).
        let starts_with_success_marker = {
            let trimmed = latest_evidence.trim_start();
            trimmed.starts_with('[')
                && trimmed
                    .split(']')
                    .next()
                    .map(|m| m.trim_start_matches('[').ends_with("_OK"))
                    .unwrap_or(false)
        };

        if latest_evidence.contains("PATH_NOT_FOUND") && !starts_with_success_marker {
            if let Some(h_id) = session.best_hypothesis_id {
                let obs_id = session.graph.add_node(
                    NodeKind::Observation,
                    "[PATH_DOES_NOT_EXIST] The requested path does not exist on disk. Do NOT attempt to read it again. Proceed directly to synthesis with the data you already have.",
                    1.0,
                    95,
                );
                session.graph.add_edge(obs_id, h_id, EdgeRelation::Contradicts, 0.90);
                session.graph.propagate_beliefs();
            }
        }

        if latest_evidence.trim().is_empty() {
            return Ok(CortexState::Strategize);
        }

        if rust_files_mutated {
            self.log("\x1b[36m>> [COMPILER CRITIC] Running cargo check to validate Rust mutation...\x1b[0m".to_string());

            let compile_output = tokio::process::Command::new("cargo").arg("check").output().await;

            match compile_output {
                Ok(output) if output.status.success() => {
                    latest_evidence.push_str("\n[COMPILE_OK] Workspace compiles successfully.");
                    self.log("\x1b[32m>> [COMPILER CRITIC] Compile OK. Proceeding to LLM Evaluator.\x1b[0m".to_string());
                }
                Ok(output) => {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    let err_preview = stderr.lines().take(20).collect::<Vec<_>>().join("\n");

                    self.log("\x1b[1;31m>> [COMPILER CRITIC] Compile FAILED. Reverting mutations.\x1b[0m".to_string());
                    let compile_fail_evidence =
                        format!("[COMPILE_FAIL] Mutation failed compiler check:\n{}", err_preview);

                    for path in &mutation_paths {
                        let _ = crate::podman::PodmanShield::execute(&format!(
                            "git checkout -- {}",
                            path
                        ));
                    }

                    session.local_evidence.truncate(evidence_checkpoint);
                    session.local_evidence.push_str(&compile_fail_evidence);
                    session.local_evidence.push('\n');

                    if let Some(h_id) = session.best_hypothesis_id {
                        let obs_id = session.graph.add_node(
                            NodeKind::Observation,
                            "[COMPILE_FAIL] Patch failed cargo check. Reverted.",
                            1.0,
                            90,
                        );
                        session.graph.add_edge(obs_id, h_id, EdgeRelation::Contradicts, 0.99);
                        session.graph.propagate_beliefs();
                    }

                    return Ok(CortexState::Strategize);
                }
                Err(e) => {
                    self.log(format!("\x1b[33m>> [COMPILER CRITIC] Failed to run cargo check: {}. Proceeding.\x1b[0m", e));
                }
            }
        }

        Ok(CortexState::Evaluate {
            evidence: latest_evidence,
            proposal: proposal.clone(),
            executed_action: executed_actions.join("\n"),
            belief,
            is_halt_action: last_is_halt,
            mutation_path: mutation_paths.last().cloned(),
        })
    }

    async fn handle_evaluate(
        &mut self,
        session: &mut EpistemicSession,
        mission: &str,
        evidence: String,
        _proposal: String,
        executed_action: String,
        belief: f32,
        _is_mutation_task: bool,
        _is_write_task: bool,
        is_halt_action: bool,
        mutation_path: Option<String>,
    ) -> Result<CortexState> {
        self.log(
            "\x1b[33m>> [CORTEX] Evaluating physical evidence against strategy...\x1b[0m"
                .to_string(),
        );
        session.budget.record_call();

        let h_id = session.best_hypothesis_id.unwrap_or_default();
        let h_content =
            session.graph.nodes.get(&h_id).map(|n| n.content.clone()).unwrap_or_default();
        // HARD EPISTEMIC BOUNDARY:
        // The evaluator receives ONLY:
        //   - the exact action(s) that physically crossed the ActionGate
        //   - the physical evidence produced by those action(s)
        //
        // Historical evidence remains available to the Cortex/session, but
        // must NOT be mixed into the evaluator's current-turn evidence.
        // Keep the authoritative full action in the execution/audit record,
        // but bound the representation sent to the LLM evaluator. Large
        // write_file payloads must not consume the light-model context window.
        let evaluator_action: String = executed_action.chars().take(2000).collect();

        // The action is supplied separately as ACTION TAKEN. Do not duplicate
        // it inside the physical-evidence context.
        let eval_evidence_context =
            format!("PHYSICAL EVIDENCE PRODUCED BY THOSE ACTION(S):\\n{}", evidence);

        match EpistemicEngine::evaluate_evidence(
            &self.client,
            &self.api_key,
            &h_content,
            &evaluator_action,
            &eval_evidence_context,
        )
        .await
        {
            Err(e) => {
                self.log(format!("\x1b[31m>> [CORTEX FATAL] Evaluator panicked: {}\x1b[0m", e));
                Ok(CortexState::Strategize)
            }
            Ok(eval) => {
                // Search relevance must inspect the action that actually
                // executed, never the strategist's natural-language proposal.
                let executed_action_lower = executed_action.to_ascii_lowercase();
                let is_search_like = executed_action_lower.contains("search_code")
                    || executed_action_lower.contains("search_memory");
                if is_search_like {
                    let query_hint = executed_action.chars().take(200).collect::<String>();
                    let rel = score_search_relevance(&query_hint, &evidence);
                    if !should_raise_belief(rel, 0.45) {
                        let mut eval = eval;
                        if eval.confidence_modifier > 0.0 {
                            eval.confidence_modifier = 0.0;
                        }
                        session.record_evidence(&evidence, &eval, h_id);
                        return Ok(CortexState::Strategize);
                    }
                }
                self.log(format!(
                    "\x1b[33m>> [EVALUATION]\x1b[0m {} (Modifier: {:.2})",
                    eval.critique, eval.confidence_modifier
                ));
                session.record_evidence(&evidence, &eval, h_id);

                let has_unwritten_artifacts =
                    session.contract.required_artifact_paths.iter().any(|a| {
                        !crate::evidence_markers::has_artifact_write(
                            &session.local_evidence,
                            &a.path,
                        )
                    });

                if is_halt_action && has_unwritten_artifacts {
                    self.log(format!("\x1b[1;33m>> [CORTEX] Terminal HALT rejected: Required deliverable(s) {:?} not yet written.\x1b[0m",
                        session.contract.required_artifact_paths.iter().map(|a| &a.path).collect::<Vec<_>>()));
                    session.local_evidence.push_str("\n[HALT_REJECTED] You cannot halt. The required output deliverable has not been written yet. You MUST write the final file now.\n");
                    return Ok(CortexState::Strategize);
                }

                let deterministic_contract_satisfied =
                    crate::completion_engine::SubgoalCompletionEngine::evaluate(
                        &session.contract,
                        &session.local_evidence,
                        belief,
                    );

                let command_only_contract = session.contract.conditions.len() == 1
                    && matches!(
                        session.contract.conditions.first(),
                        Some(crate::completion_engine::CompletionCondition::CommandExecutionVerified { .. })
                    );

                if is_halt_action && deterministic_contract_satisfied && command_only_contract {
                    self.log(
                        "\x1b[1;36m>> [CORTEX TERMINAL] Deterministic command contract satisfied by physical CMD_OK evidence. No additional model verification required. Node complete.\x1b[0m"
                            .to_string(),
                    );

                    return Ok(CortexState::Complete {
                        belief: belief.max(0.95),
                        evidence: session.collect_evidence(h_id),
                    });
                }

                if is_halt_action
                    && deterministic_contract_satisfied
                    && eval.confidence_modifier >= 0.0
                    && belief > 0.70
                {
                    let strategy_for_verification = format!(
                        "MISSION OBJECTIVE:\n{}\n\nCURRENT STRATEGY:\n{}",
                        mission, h_content
                    );

                    self.log(
                        "\x1b[1;35m>> [CORTEX] HALT proposed. Running node-level epistemic verification...\x1b[0m"
                            .to_string(),
                    );

                    let verification_evidence = format!(
                        "CURRENT-TURN PHYSICAL EVIDENCE:\n{}\n\n                         HISTORICAL TASK EVIDENCE (CONTEXT ONLY):\n{}",
                        evidence, session.local_evidence
                    );

                    match EpistemicEngine::verify_outcome(
                        &self.client,
                        &self.api_key,
                        &strategy_for_verification,
                        &executed_action,
                        &verification_evidence,
                    )
                    .await
                    {
                        Ok(report)
                            if report.goal_satisfied
                                && report.independent_check_passed
                                && report.evidence_confidence >= 0.75 =>
                        {
                            self.log(format!(
                                "\x1b[1;36m>> [CORTEX TERMINAL] DAG node independently verified. Confidence: {:.2}. Node complete.\x1b[0m",
                                report.evidence_confidence
                            ));

                            return Ok(CortexState::Complete {
                                belief: belief.max(report.evidence_confidence).max(0.95),
                                evidence: session.collect_evidence(h_id),
                            });
                        }

                        Ok(report) => {
                            self.log(format!(
                                "\x1b[1;33m>> [CORTEX] HALT rejected at mission level. goal_satisfied={} independent_check_passed={} confidence={:.2}. Continuing.\x1b[0m",
                                report.goal_satisfied,
                                report.independent_check_passed,
                                report.evidence_confidence
                            ));

                            session.local_evidence.push_str(&format!(
                                "\n[HALT_REJECTED] Mission verification failed: {}\n",
                                report.rationale
                            ));

                            return Ok(CortexState::Strategize);
                        }

                        Err(e) => {
                            self.log(format!(
                                "\x1b[1;33m>> [CORTEX] HALT verification unavailable: {}. Continuing mission instead of terminating.\x1b[0m",
                                e
                            ));

                            return Ok(CortexState::Strategize);
                        }
                    }
                }

                if is_halt_action && !deterministic_contract_satisfied {
                    self.log(
                        "\x1b[1;33m>> [CORTEX] HALT rejected: deterministic completion contract is not satisfied. Continuing mission.\x1b[0m"
                            .to_string(),
                    );
                    session.local_evidence.push_str(
                        "\n[HALT_REJECTED] Deterministic completion contract not satisfied.\n",
                    );
                    return Ok(CortexState::Strategize);
                }

                if eval.confidence_modifier < 0.0 {
                    if let Some(path) = &mutation_path {
                        let _ = crate::podman::PodmanShield::execute(&format!(
                            "git checkout -- {}",
                            path
                        ));
                        session.local_evidence = session.local_evidence.replace(&evidence, "");
                    }
                }

                if let Some(updated_node) = session.graph.nodes.get(&h_id) {
                    self.log(format!(
                        "\x1b[1;32m>> [CORTEX] Strategy Belief updated to {:.2}\x1b[0m",
                        updated_node.belief_score
                    ));
                }
                Ok(CortexState::Strategize)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::CorticalLoop;
    use crate::agent_protocol::AgentAction;

    #[test]
    fn search_memory_strategy_requires_matching_search_memory_action() {
        let query =
            "host observations involving uptime, filesystem, available memory, and load average";
        let strategy = format!("search_memory:{}", query);

        let matching = AgentAction::SearchMemory {
            query: query.to_string(),
            reasoning: "regression test".to_string(),
        };

        let wrong_action = AgentAction::ReadFile {
            path: query.to_string(),
            reasoning: "regression test".to_string(),
        };

        assert!(CorticalLoop::action_matches_strategy(&strategy, &matching));
        assert!(!CorticalLoop::action_matches_strategy(&strategy, &wrong_action));
    }
}
