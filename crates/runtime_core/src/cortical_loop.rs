#![allow(unreachable_code)]
use anyhow::Result;
use reqwest::Client;
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};
use tokio::time::{sleep, Duration};

use crate::action_gate::ActionGate;
use crate::agent_protocol::AgentAction;
use crate::cognition::epistemic_session::EpistemicSession;
use crate::cognition_graph::{EdgeRelation, NodeKind};
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

#[derive(Debug)]
struct CompletionState {
    pub is_write_task: bool,
    pub is_mutation_task: bool,
    pub has_prior_evidence: bool,
    pub compound_satisfied: bool,
}

fn evaluate_completion(
    session: &EpistemicSession,
    mission: &str,
    current_belief: f32,
    evidence_history: &str,
) -> CompletionState {
    // Task classification used to be re-derived here from raw mission/objective text
        // via naive `.contains(...)` checks with no negation-awareness -- separately from
        // session.contract, which is already built by SubgoalCompletionEngine::infer_contract()
        // using negation-aware classification (any_word_unnegated). That duplication let a
        // read-only synthesis directive get misclassified as a mutation task purely because
        // the mission text said "synthesis"/"report"/"document" as a phase label, not as an
        // instruction to write. Derive from session.contract instead -- the single source of
        // truth for task classification everywhere else in the completion pipeline.
        let is_mutation_task = session.contract.is_mutation_task;
        let is_write_task = is_mutation_task
            && session.contract.conditions.iter().any(|c| {
                matches!(c, crate::completion_engine::CompletionCondition::WriteMarkerAcquired)
            });

    let has_prior_evidence = if is_write_task {
        session.local_evidence.lines().any(|l| l.starts_with("[WRITE_OK]") || l.starts_with("[CMD_OK]"))
    } else if is_mutation_task {
        session.local_evidence.lines().any(|l| {
            l.starts_with("[DIFF_PATCH_OK]") || l.starts_with("[REPLACE_LINES_OK]") || l.starts_with("[PATCH_FUNCTION_OK]") || l.starts_with("[WRITE_OK]")
        })
    } else {
        session.local_evidence.lines().any(|l| {
            l.starts_with("[READ_CHUNK_OK]") || l.starts_with("[SEARCH_CODE_OK]") || l.starts_with("[READ_OK]") || l.starts_with("[WRITE_OK]") || l.starts_with("[CMD_OK]")
        })
    };

    let compound_satisfied = if is_mutation_task {
        session.is_subgoal_satisfied(current_belief)
    } else if is_write_task {
        true
    } else {
        let mission_lower = mission.to_lowercase();
        let objective_part = mission_lower.lines().find(|line| line.starts_with("objective:")).unwrap_or(mission_lower.as_str());
        let evidence_lower = format!("{} {}", evidence_history.to_lowercase(), session.local_evidence.to_lowercase());
        let and_segments: Vec<&str> = objective_part.split(" and ").map(|s| s.trim()).filter(|s| !s.is_empty()).collect();
        
        and_segments.iter().all(|segment| {
            let or_parts: Vec<&str> = segment.split(" or ").map(|s| s.trim()).filter(|s| !s.is_empty()).collect();
            if or_parts.len() <= 1 {
                segment.split_whitespace().filter(|w| w.len() > 3).any(|w| evidence_lower.contains(w))
            } else {
                or_parts.iter().any(|part| part.split_whitespace().filter(|w| w.len() > 3).any(|w| evidence_lower.contains(w)))
            }
        })
    };

    CompletionState {
        is_write_task,
        is_mutation_task,
        has_prior_evidence,
        compound_satisfied,
    }
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
            action_gate: ActionGate::new(),
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
                            "WORKSPACE FACTS (ground truth, do not guess): workspace_root={workspace_root} | target_dir={target_dir}."
                        )
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
        format!("{}...\n\n[... {} CHARACTERS COMPRESSED TO PREVENT CONTEXT OVERFLOW ...]\n\n{}", start, ctx.len() - max_chars, end)
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

        let mut context_chunks = (*self.world_model).semantic_query(mission).await;
        context_chunks.push(Self::resolve_workspace_paths().await);
        if let Some(evidence) = ancestor_evidence {
            if !evidence.trim().is_empty() {
                context_chunks.push(format!("\n[ANCESTOR NODE EVIDENCE]\n{}", evidence));
            }
        }
        let world_context = Self::compress_context(&context_chunks.join("\n"), 12000);

        let mut session = EpistemicSession::new(mission);

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
            session.graph.add_node(NodeKind::Hypothesis, &h, 0.5, 100);
            self.log(format!("\x1b[90m  -> Strategy: {}\x1b[0m", h));
        }

        // Unified completion state computed inside the turn loop

        let mut turn = 0;
        loop {
            turn += 1;
            self.log(format!(
                "\n\x1b[1;33m>> [CORTEX] Cognitive Turn {} / {}\x1b[0m",
                turn, session.budget.max_turns
            ));

            if let Some(best_node) = session.graph.select_best_hypothesis().cloned() {
                let h_id = best_node.id;
                let h_content = best_node.content.clone();
                let current_belief = best_node.belief_score;
                if current_belief > session.max_observed_belief {
                    session.max_observed_belief = current_belief;
                    session.best_hypothesis_id = Some(h_id);
                }
                self.log(format!(
                    "\x1b[32m>> [CORTEX] Exploring Top Strategy (Belief: {:.2}):\x1b[0m {}",
                    current_belief, h_content
                ));

                let evidence_history = Self::compress_context(&session.collect_evidence(h_id), 64000);

                // ⚡ COGNITIVE MOMENTUM GUARD (PERMANENT SCALING)
                if session.budget.is_exhausted(turn) {
                    if current_belief >= 0.55 && session.consecutive_mutation_vetoes < 3 && turn < 300 {
                        self.log(format!("\x1b[1;35m>> [COGNITIVE MOMENTUM] Call limit reached, but Belief is high ({:.2}). Dynamically extending budget by +20 turns / +80 API calls.\x1b[0m", current_belief));
                        session.budget.max_turns += 20;
                        session.budget.max_model_calls += 80;
                    } else {
                        self.log("\x1b[1;31m>> [SUBGOAL_BUDGET_EXHAUSTED] Cognitive call limit reached with low belief or stagnation. Node failed.\x1b[0m".to_string());
                        return Ok(CortexOutcome::TurnsExhausted {
                            best_belief: current_belief,
                            evidence: session.collect_evidence(h_id),
                        });
                    }
                }

                
                // ⚡ AUTONOMOUS COMPLETION GUARD (PERMANENT ARCHITECTURAL FIX)
                let comp_state = evaluate_completion(&session, mission, current_belief, &evidence_history);

                // If the state is mathematically satisfied and belief is extremely high, force the node to halt.
                if comp_state.has_prior_evidence && comp_state.compound_satisfied && current_belief >= 0.95 {
                    self.log("\x1b[1;36m>> [CORTEX OMNI-GUARD] Contract mathematically satisfied. Stripping LLM authority and forcing deterministic node completion.\x1b[0m".to_string());
                    return Ok(CortexOutcome::CompletionProposed {
                        belief: current_belief,
                        evidence: session.collect_evidence(h_id),
                    });
                }

                self.log("\x1b[36m>> [STRATEGIST] Planning next maneuver...\x1b[0m".to_string());
                session.budget.record_call();
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

                // ⚡ DETERMINISTIC HALT SHORT-CIRCUIT
                let prop_lower = proposed_action.trim().to_lowercase();
                if prop_lower == "halt" || prop_lower.starts_with("halt ") {
                    if comp_state.has_prior_evidence && comp_state.compound_satisfied {
                        self.log("\x1b[1;36m>> [CORTEX] DETERMINISTIC GUARD: Halt proposed following successful local evidence. Subtask complete.\x1b[0m".to_string());
                        return Ok(CortexOutcome::CompletionProposed {
                            belief: 1.0,
                            evidence: session.collect_evidence(h_id),
                        });
                    } else if comp_state.has_prior_evidence && !comp_state.compound_satisfied {
                        self.log("\x1b[1;33m>> [CORTEX] DETERMINISTIC GUARD SUPPRESSED: Halt proposed but evidence covers only part of a compound objective; continuing to explore.\x1b[0m".to_string());
                    } else if (comp_state.is_mutation_task || comp_state.is_write_task) && !comp_state.has_prior_evidence {
                        if current_belief > 0.90 {
                            self.log("\x1b[1;33m>> [CORTEX GUARD OVERRIDE] High belief (>.90) halt proposed without mutation. Allowing Evaluator to assess validity.\x1b[0m".to_string());
                        } else {
                        self.log("\x1b[1;31m>> [CORTEX FAULT] Halt rejected: Objective requires patching/writing, but no mutation success marker found.\x1b[0m".to_string());
                        let obs_id = session.graph.add_node(NodeKind::Observation, "[HALT_REJECTED] The directive requires modifying files. You proposed halt without successfully patching anything. You MUST use diff_patch, patch_function, or write_file.", 1.0, 50);
                        session.graph.add_edge(obs_id, h_id, EdgeRelation::Contradicts, 0.6);
                        session.graph.propagate_beliefs();
                        continue;
                    }
                    }
                }

                // --- ANTI-LOOP KINETIC GUARD (PATCHED) ---
                let action_key = proposed_action.trim().to_string();
                if session.is_action_attempted(&action_key) {
                    // ⚡ FIX: Removed the || belief >= 0.60 escape hatch. Must satisfy contract.
                    if comp_state.compound_satisfied {
                        self.log("\x1b[1;36m>> [SUBGOAL_COMPLETE] Loop detected but contract is mathematically satisfied. Halting.\x1b[0m".to_string());
                        return Ok(CortexOutcome::CompletionProposed {
                            belief: current_belief,
                            evidence: session.collect_evidence(h_id),
                        });
                    } else {
                        self.log(format!("\x1b[1;31m>> [ENTERPRISE CORTEX] Anti-Loop Guard Triggered: Action [{}] already attempted.\x1b[0m", action_key));
                        let obs_id = session.graph.add_node(
                            NodeKind::Observation,
                            "[CRITICAL VETO] You are repeating a failed action! YOU MUST MUTATE YOUR STRATEGY, USE A DIFFERENT TOOL, OR HALT.",
                            1.0, 95
                        );
                        session.graph.add_edge(obs_id, h_id, EdgeRelation::Contradicts, 0.99);
                        session.graph.propagate_beliefs();
                        continue;
                    }
                }
                session.record_action_attempt(&action_key);

                self.log("\x1b[31m>> [RED TEAM] Criticizing proposal...\x1b[0m".to_string());
                session.budget.record_call();
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
                        self.log(format!(
                            "\x1b[31m>> [CORTEX FATAL] Red Team critic parser panicked: {}\x1b[0m",
                            e
                        ));
                        let evidence = format!(
                            "[CRITIC_PARSE_FAILURE] Critic response could not be parsed: {}",
                            e
                        );
                        let obs_id =
                            session.graph.add_node(NodeKind::Observation, &evidence, 1.0, 50);
                        session.graph.add_edge(obs_id, h_id, EdgeRelation::Contradicts, 0.15);
                        session.graph.propagate_beliefs();
                    }
                    Ok(mut critic) => {
                        self.log(format!(
                            "\x1b[31m>> [CRITIC] Verdict:\x1b[0m {} (Modifier: {:.2})",
                            critic.critique, critic.confidence_modifier
                        ));

                        // VETO CIRCUIT BREAKER: the Critic can get stuck repeatedly
                        // rejecting the exact mutation action that would satisfy an
                        // unmet write/patch contract, reasoning e.g. "the file hasn't
                        // been written yet" against the very write_file proposal that
                        // would write it. A prompt fix reduces how often this occurs
                        // but can't guarantee it never recurs under a different model
                        // or phrasing. This is the deterministic backstop: after enough
                        // consecutive vetoes of a mutation-shaped proposal while the
                        // contract's mutation marker is still missing, force the
                        // proposal through instead of trusting the Critic indefinitely.
                        let is_mutation_shaped_proposal = prop_lower.contains("write_file")
                            || prop_lower.contains("patch_function")
                            || prop_lower.contains("diff_patch")
                            || prop_lower.contains("replace_lines");
                        let contract_still_unmet = (comp_state.is_mutation_task || comp_state.is_write_task) && !comp_state.has_prior_evidence;

                        if critic.confidence_modifier <= -0.2
                            && is_mutation_shaped_proposal
                            && contract_still_unmet
                        {
                            session.consecutive_mutation_vetoes += 1;
                            if session.consecutive_mutation_vetoes >= 3 {
                                self.log(format!(
                                    "\x1b[1;35m>> [CIRCUIT_BREAKER_OVERRIDE] Critic vetoed {} consecutive mutation proposals while the contract's mutation marker is still missing. Overriding veto and executing.\x1b[0m",
                                    session.consecutive_mutation_vetoes
                                ));
                                critic.confidence_modifier = 0.1;
                                session.consecutive_mutation_vetoes = 0;
                            }
                        } else {
                            session.consecutive_mutation_vetoes = 0;
                        }

                        if critic.confidence_modifier > -0.2 {
                            self.log("\x1b[36m>> [CORTEX] Translating intent to physical action...\x1b[0m".to_string());
                            session.budget.record_call();

                            let parsed_actions = if let Ok(direct_actions) =
                                crate::json_parser::extract_actions(&proposed_action)
                            {
                                if !direct_actions.is_empty() {
                                    self.log("\x1b[32m>> [CORTEX INTERCEPT] Direct JSON Proposal Intercept: Bypassing Translator LLM.\x1b[0m".to_string());
                                    Ok(direct_actions)
                                } else {
                                    if let Ok(raw_json) = EpistemicEngine::translate_to_action(
                                        &self.client,
                                        &self.api_key,
                                        &h_content,
                                        &proposed_action,
                                    )
                                    .await
                                    {
                                        crate::json_parser::extract_actions(&raw_json)
                                    } else {
                                        Ok(vec![])
                                    }
                                }
                            } else {
                                if let Ok(raw_json) = EpistemicEngine::translate_to_action(
                                    &self.client,
                                    &self.api_key,
                                    &h_content,
                                    &proposed_action,
                                )
                                .await
                                {
                                    crate::json_parser::extract_actions(&raw_json)
                                } else {
                                    Ok(vec![])
                                }
                            };

                            if let Ok(mut actions) = parsed_actions {
                                if actions.is_empty() && !proposed_action.trim().is_empty() {
                                    let action_str = proposed_action.trim();
                                    if let Some(intercepted) =
                                        crate::action_intercept::intercept_fallback_action(
                                            action_str,
                                        )
                                    {
                                        self.log("\x1b[32m>> [CORTEX INTERCEPT] Native Tool Intercept: Handling action directly.\x1b[0m".to_string());
                                        actions.push(intercepted);
                                    } else {
                                        // Deliberately NOT falling back to raw
                                        // ExecuteCommand here. This branch used to
                                        // hand the Strategist's unparsed proposal
                                        // text (which is frequently a raw JSON blob,
                                        // not shell syntax) straight to bash -c --
                                        // same corruption class as the write_file
                                        // fallback removed earlier this session,
                                        // just luckier so far because malformed
                                        // JSON happens to fail as bash syntax. A
                                        // proposal whose text is coincidentally
                                        // valid (and destructive) shell would not
                                        // be so lucky. Treat unrecognized text as a
                                        // genuine translation failure instead.
                                        self.log("\x1b[31m>> [CORTEX FAULT] Translator produced empty JSON and proposal text matched no known tool. Refusing raw fallback execution.\x1b[0m".to_string());
                                        let obs_id = session.graph.add_node(
                                            NodeKind::Observation,
                                            "[TRANSLATION_UNRECOGNIZED] Proposal text did not match any known action type and was not executed as a raw command.",
                                            1.0,
                                            50,
                                        );
                                        session.graph.add_edge(obs_id, h_id, EdgeRelation::Contradicts, 0.3);
                                        session.graph.propagate_beliefs();
                                    }
                                }

                                if actions.is_empty() {
                                    self.log("\x1b[31m>> [CORTEX FAULT] Translator produced an empty action list.\x1b[0m".to_string());
                                    let obs_id = session.graph.add_node(
                                        NodeKind::Observation,
                                        "[TRANSLATION_EMPTY]",
                                        1.0,
                                        50,
                                    );
                                    session.graph.add_edge(
                                        obs_id,
                                        h_id,
                                        EdgeRelation::Contradicts,
                                        0.15,
                                    );
                                    session.graph.propagate_beliefs();
                                } else {
                                    let mut executor =
                                        RoboticExecutor::new(self.api_key.clone(), self.tx.clone());

                                    for action in actions {
                                        // ⚡ SYNTHESIS LOCK -- REMOVED. This blocked ReadFile/ReadFileChunk/
                                        // SearchCode/SearchMemory whenever the mission text (checked as a
                                        // raw uppercase substring match) contained "SYNTHESIS", "FINAL
                                        // REPORT", or "DOCUMENT" -- but "Document exact signatures" in a
                                        // legitimate read-then-extract objective matched "DOCUMENT" as a
                                        // verb, not the synthesis-phase noun it was meant to catch, and
                                        // blocked the task's own required file read for its full 60-turn
                                        // budget. It was also trivially bypassed via execute_command "cat
                                        // <path>" (not in the is_recon match arm), which returns the same
                                        // content but skips ReadFile's real side effect of embedding +
                                        // storing the read into memory via vector_bridge::get_embedding /
                                        // store_memory -- so the one path around the lock was also silently
                                        // incomplete. Confirmed live: a real synthesis-labeled task that
                                        // needed to read one specific file first was blocked from doing so
                                        // by every proper tool, exhausted its budget, and failed outright.
                                        // OutcomeVerifier's per-condition checks and the Anti-Loop Guard
                                        // already prevent unbounded recon looping without this heuristic's
                                        // false-positive and bypass problems, so it is removed rather than
                                        // re-tuned.
                                        if false {
                                            let obs_id = session.graph.add_node(
                                                NodeKind::Observation,
                                                "[SYNTHESIS_LOCK_ACTIVE] Reconnaissance is structurally forbidden in this phase. You already have all evidence in context. You MUST use write_file to output the report.",
                                                1.0, 99
                                            );
                                            session.graph.add_edge(obs_id, h_id, EdgeRelation::Contradicts, 0.99);
                                            session.graph.propagate_beliefs();
                                            continue;
                                        }

                                        let intent =
                                            IntentContract::new(&h_content, current_belief);
                                        let capability = match &action {
                                            AgentAction::ExecuteCommand { .. } => {
                                                Capability::ExecuteSystemCall
                                            }
                                            AgentAction::WriteFile { .. }
                                            | AgentAction::DiffPatch { .. }
                                            | AgentAction::PatchFunction { .. } => {
                                                Capability::WriteSubstrate
                                            }
                                            AgentAction::Halt { .. } => Capability::HaltCognition,
                                            _ => Capability::ReadSubstrate,
                                        };

                                        // Capture the mutation target path now -- `action` is moved
                                        // into MotorCortex::translate() below and won't be readable
                                        // after this point, but we need the path later to auto-revert
                                        // if the Evaluator scores this mutation negative.
                                        let mutation_path: Option<String> = match &action {
                                            AgentAction::WriteFile { path, .. }
                                            | AgentAction::DiffPatch { path, .. }
                                            | AgentAction::PatchFunction { path, .. } => {
                                                Some(path.clone())
                                            }
                                            _ => None,
                                        };

                                        let is_halt_action_captured = matches!(action, AgentAction::Halt { .. }) || proposed_action.trim().to_lowercase().starts_with("halt");
                                        let motor_cmd = MotorCortex::translate(
                                            &intent,
                                            None,
                                            action,
                                            capability,
                                            h_content.clone(),
                                        );

                                        if let Err(gate_err) = self.action_gate.approve(&motor_cmd)
                                        {
                                            self.log(format!(
                                                "\x1b[1;31m>> [ACTION GATE REJECTED]: {}\x1b[0m",
                                                gate_err
                                            ));
                                            let obs_id = session.graph.add_node(
                                                NodeKind::Observation,
                                                &format!("[ACTION_GATE_BLOCKED] {}", gate_err),
                                                1.0,
                                                50,
                                            );
                                            session.graph.add_edge(
                                                obs_id,
                                                h_id,
                                                EdgeRelation::Contradicts,
                                                0.50,
                                            );
                                            session.graph.propagate_beliefs();
                                            continue;
                                        }

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
                                            Err(e) => {
                                                format!("[ACTION FAILED] OS rejected action: {}", e)
                                            }
                                        };

                                        session.local_evidence.push_str(&evidence);
                                        session.local_evidence.push('\n');

                                        let preview =
                                            evidence.lines().take(5).collect::<Vec<_>>().join("\n");
                                        self.log(format!(
                                            "\x1b[32m>> [EVIDENCE ACQUIRED]\x1b[0m\n{}",
                                            preview
                                        ));

                                        self.log("\x1b[33m>> [CORTEX] Evaluating physical evidence against strategy...\x1b[0m".to_string());
                                        let eval_evidence_context = format!("Current Action Output:\n{}\n\nPrior Evidence in Task:\n{}", evidence, evidence_history);
                                        session.budget.record_call();
                                        match EpistemicEngine::evaluate_evidence(
                                            &self.client,
                                            &self.api_key,
                                            &h_content,
                                            &proposed_action,
                                            &eval_evidence_context,
                                        )
                                        .await
                                        {
                                            Err(e) => {
                                                self.log(format!("\x1b[31m>> [CORTEX FATAL] Evaluator panicked: {}\x1b[0m", e));
                                            }
                                            Ok(eval) => {
                                                self.log(format!(
                                                    "\x1b[33m>> [EVALUATION]\x1b[0m {} (Modifier: {:.2})",
                                                    eval.critique, eval.confidence_modifier
                                                ));

                                                session.record_evidence(&evidence, &eval, h_id);

                                                // TERMINAL HALT INTERCEPT
                                                // If the action was 'halt' and the Evaluator didn't penalize it,
                                                // force-exit the node to prevent infinite halt loops.
                                                if is_halt_action_captured && eval.confidence_modifier >= 0.0 && current_belief > 0.85 {
                                                    self.log("\x1b[1;36m>> [CORTEX TERMINAL] Valid halt executed and verified. Submitting evidence to ContractVerifier.\x1b[0m".to_string());
                                                    return Ok(CortexOutcome::CompletionProposed {
                                                        belief: current_belief.max(0.95),
                                                        evidence: session.collect_evidence(h_id),
                                                    });
                                                }

                                                // AUTO-REVERT: a mutation the Evaluator scored negative must
                                                // not be allowed to stand as satisfied evidence. Revert the
                                                // file on disk and strip the raw marker from local_evidence so
                                                // completion contracts correctly see this as unresolved instead
                                                // of "patch succeeded technically" being treated as "objective
                                                // accomplished."
                                                if eval.confidence_modifier < 0.0 {
                                                    if let Some(path) = &mutation_path {
                                                        self.log(format!("\x1b[1;31m>> [CORTEX SAFETY] Mutation to '{}' scored negative ({:.2}) -- reverting via git checkout.\x1b[0m", path, eval.confidence_modifier));
                                                        let _ =
                                                            crate::podman::PodmanShield::execute(
                                                                &format!(
                                                                    "git checkout -- {}",
                                                                    path
                                                                ),
                                                            );
                                                        session.local_evidence = session
                                                            .local_evidence
                                                            .replace(&evidence, "");
                                                        let reverted_tag = format!(
                                                            "[MUTATION_REVERTED] {} -- Evaluator rejected this mutation ({:.2}): {}\n",
                                                            path, eval.confidence_modifier, eval.critique
                                                        );
                                                        session
                                                            .local_evidence
                                                            .push_str(&reverted_tag);
                                                    }
                                                }

                                                if let Some(updated_node) =
                                                    session.graph.nodes.get(&h_id)
                                                {
                                                    self.log(format!("\x1b[1;32m>> [CORTEX] Strategy Belief updated to {:.2}\x1b[0m", updated_node.belief_score));

                                                    // Deterministic early exits disabled. 
                                                    // The LLM must explicitly issue a HALT action.
                                                    if comp_state.compound_satisfied {
                                                        self.log("\x1b[1;32m>> [CORTEX] Contract criteria met. Awaiting explicit Agent HALT.\x1b[0m".to_string());
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        } else {
                            self.log(
                                "\x1b[31m>> [RED TEAM] VETO. Proposal rejected.\x1b[0m".to_string(),
                            );
                            let veto_msg = format!(
                                "[RED TEAM VETO] Action blocked. Critic feedback: {}",
                                critic.critique
                            );
                            let veto_obs_id =
                                session.graph.add_node(NodeKind::Observation, &veto_msg, 1.0, 50);
                            session.graph.add_edge(
                                veto_obs_id,
                                h_id,
                                EdgeRelation::Contradicts,
                                critic.confidence_modifier.abs(),
                            );
                            session.graph.propagate_beliefs();
                        }
                    }
                }
            } else {
                self.log(
                    "\x1b[1;35m>> [CORTEX] All strategies exhausted. Re-orienting graph...\x1b[0m"
                        .to_string(),
                );
                let mut reorient_context = world_context.clone();
                if !session.executed_actions.is_empty() {
                    reorient_context.push_str("\n[ALREADY ATTEMPTED ACTIONS -- DO NOT REPEAT]\n");
                    for a in &session.executed_actions {
                        reorient_context.push_str(&format!("- {}\n", a));
                    }
                }
                session.budget.record_call();
                let new_hypotheses = EpistemicEngine::generate_hypotheses(
                    &self.client,
                    &self.api_key,
                    mission,
                    &reorient_context,
                )
                .await;
                for h in new_hypotheses.unwrap_or_default() {
                    session.graph.add_node(NodeKind::Hypothesis, &h, 0.5, 100);
                    self.log(format!("\x1b[90m  -> Strategy: {}\x1b[0m", h));
                }
            }
            sleep(Duration::from_secs(2)).await;
        }

    }
}
