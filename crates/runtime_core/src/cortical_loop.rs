#![allow(clippy::too_many_arguments)]
#![allow(unreachable_code)]
use anyhow::Result;
use nexus_substrate::episode::{EpisodeContext, RecallScope};
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
use crate::compile_remediation::{
    format_compile_failure_evidence, CompileDecision, COMPILE_RETRY_BUDGET,
};
use crate::contract_status::{ContractEvaluator, ContractStatus};
use crate::cortex::compiler_gate::{CompilerGate, CompilerGateResult, RustVerificationTarget};
use crate::distributed_scheduler::DistributedScheduler;
use crate::epistemic_engine::EpistemicEngine;
use crate::executor::RoboticExecutor;
use crate::intent_contract::IntentContract;
use crate::memory_cortex::MemoryCortex;
use crate::motor_cortex::MotorCortex;
use crate::policy::Capability;
use crate::shared_state::VmoState;
use crate::world_model::WorldModel;
use crate::cortical::capability::capability_for;

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
        /// Diagnostic-only. The active propagation path returns
        /// `session.local_evidence` instead of this field (see the
        /// `CortexState::Fail` arm in `ignite_with_evidence`), so it is
        /// retained for constructors but never read.
        #[allow(dead_code)]
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
    /// Episode context for memory provenance and automatic recall
    /// scoping. Defaults to all-None ("episode unknown"), which
    /// makes automatic recall return zero results. Set by the
    /// caller (task_manager.rs) before ignite.
    episode: EpisodeContext,
}

impl CorticalLoop {
    /// Revert the active mutation set without losing remediation state when
    /// any individual checkout fails. A failed revert means the source may
    /// still be broken, so the compile-remediation episode must remain active.
    fn rollback_active_mutations(session: &mut EpistemicSession) -> bool {
        let mut all_succeeded = true;

        for path in &session.active_mutation_paths {
            let quoted = format!("'{}'", path.replace('\'', "'\\''"));
            if crate::podman::PodmanShield::execute(&format!("git checkout -- {}", quoted)).is_err()
            {
                all_succeeded = false;
            }
        }

        if all_succeeded {
            session.compile_remediation.reset();
            session.active_mutation_paths.clear();
        } else {
            session.local_evidence.push_str(
                "[COMPILE_REVERT_FAILED] One or more active-mutation reverts failed. \
                 Source may still be broken; compile remediation remains active.\n",
            );
        }

        all_succeeded
    }

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
            episode: EpisodeContext::default(),
        }
    }

    /// Set the episode context for this loop. Automatic recall is
    /// scoped by plan_id via `RecallScope::CurrentMission`; the
    /// session_id is minted inside ignite_with_evidence and
    /// recorded on every memory this loop writes. When unset
    /// (all-None), automatic recall returns zero results
    /// (fail-closed).
    pub fn set_episode(&mut self, episode: EpisodeContext) {
        self.episode = episode;
    }

    pub(crate) fn log(&self, msg: String) {
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

    pub async fn ignite(&mut self, mission: &str) -> Result<CortexOutcome> {
        self.ignite_with_evidence(mission, None).await
    }

    pub async fn ignite_with_evidence(
        &mut self,
        mission: &str,
        ancestor_evidence: Option<&str>,
    ) -> Result<CortexOutcome> {
        self.log("\x1b[1;36m>> [CORTEX] Igniting Epistemic Reasoning Session\x1b[0m".to_string());

        // Mint the epistemic session first so its session_id is
        // available to the episode-scoped recall gate below. The
        // session is not moved or borrowed until after the recall
        // completes.
        let mut session = EpistemicSession::new(mission);
        let session_episode = EpisodeContext {
            session_id: Some(session.session_id.to_string()),
            plan_id: self.episode.plan_id.clone(),
            task_id: self.episode.task_id.clone(),
            mission_id: self.episode.mission_id.clone(),
        };

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
                match self
                    .memory
                    .recall_semantic_context_scoped(
                        &query_embedding,
                        mission,
                        6,
                        RecallScope::CurrentMission,
                        &session_episode,
                        nexus_substrate::taint::FlowPolicy::INTERNAL_SINK,
                    )
                    .await
                {
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
        let world_context = crate::cortical::util::compress_context(&context_chunks.join("\n"), 6000);

        if let Some(evidence) = ancestor_evidence {
            if !evidence.trim().is_empty() {
                use sha2::{Digest, Sha256};
                let trimmed = evidence.trim();
                let digest = format!("{:x}", Sha256::digest(trimmed.as_bytes()));
                session.local_evidence.push_str(&format!(
                    "\n[DAG LINEAGE INHERITED EVIDENCE sha256:{}]\n{}\n[DAG LINEAGE COMPLETE sha256:{}]\n",
                    digest, trimmed, digest
                ));
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

        // HYPOTHESIS SELECTION DETERMINISM:
        //
        // generate_hypotheses returns its hypotheses in a deliberate
        // order (the compound READ_OK: + SEARCH_CODE_OK: block emits
        // search first, then read). Adding them all with equal belief
        // 0.5 lets select_best_hypothesis tie-break arbitrarily over
        // a UUID-keyed graph, so the turn-1 strategy can be any of the
        // equal-belief hypotheses. Give the first one a marginally
        // higher initial belief so the intended order is authoritative.
        //
        // Confirmed root cause of the task_failed_source_retention
        // abort at 8/12: search_code:rollback was generated first but
        // read_file was selected for turn 1, and every subsequent
        // search_code proposal was rejected as a STRATEGY VIOLATION.
        // The node exhausted its turns without ever executing the
        // action required by its mandatory SEARCH_CODE_OK criterion.
        //
        // Belief is only the initial tiebreak; subsequent evidence
        // updates move the hypotheses freely.
        for (index, h) in hypotheses.into_iter().enumerate() {
            let initial_belief = if index == 0 { 0.51 } else { 0.5 };
            let hid = session.graph.add_node(NodeKind::Hypothesis, &h, initial_belief, 100);
            if session.best_hypothesis_id.is_none() {
                session.best_hypothesis_id = Some(hid);
            }
            self.log(format!("\x1b[90m  -> Strategy: {}\x1b[0m", h));
        }

        self.log(format!(
            "\x1b[90m  -> Contract: read_only={} mutation={} artifacts={:?}\n  -> Conditions: {:?}\x1b[0m",
            session.contract.read_only,
            session.contract.is_mutation_task,
            session.contract
                .required_artifact_paths
                .iter()
                .map(|a| &a.path)
                .collect::<Vec<_>>(),
            session.contract.conditions,
        ));

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
                // Ignore code shifts caused by actions performed within this session
                if session.local_evidence.contains(&event.path)
                    || session.executed_actions.keys().any(|k| k.contains(&event.path))
                {
                    continue;
                }
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
                    session.graph.observe_contradiction(&shift_obs, h_id, 85, 0.40);
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

                // An active compile-remediation episode means the source on
                // disk is not proven to compile. Even if the static contract
                // conditions are satisfied by historical evidence (for example
                // by a [PATCH_FUNCTION_OK] marker that a subsequent compiler
                // check invalidated), do not authorize completion while
                // remediation is in flight. The state machine is reset only
                // when a subsequent compile succeeds or a confirmed revert
                // resolves the episode.
                let compile_remediation_resolved = !session.compile_remediation.is_active();

                if deterministic_contract_satisfied && !compile_remediation_resolved {
                    self.log(
                        "\x1b[1;33m>> [CORTEX] Contract satisfied by static evidence, but an unresolved compile failure is active. Continuing remediation.\x1b[0m"
                            .to_string(),
                    );
                }

                let requires_semantic_verification =
                    crate::completion_engine::requires_semantic_verification(
                        &session.contract.objective,
                    );

                if deterministic_contract_satisfied
                    && compile_remediation_resolved
                    && !requires_semantic_verification
                {
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
                        if session.compile_remediation.is_active() {
                            Self::rollback_active_mutations(&mut session);
                        }

                        return Ok(CortexOutcome::TurnsExhausted {
                            best_belief: current_belief,
                            evidence: session.collect_evidence(best_node.id),
                        });
                    }
                }
            } else if session.budget.is_exhausted(turn) {
                // No hypothesis survived belief decay (e.g. every candidate
                // strategy was falsified and dropped below the 0.15
                // selection floor), but accumulated evidence in
                // session.local_evidence is independent of hypothesis
                // survival. Check the deterministic contract directly
                // before declaring failure -- otherwise a mission whose
                // evidence already satisfies its contract (e.g. a
                // confirmed-absent inspection target) can never complete
                // once its sole hypothesis decays to death, even though
                // physical proof of completion already exists. Confirmed
                // root cause of the reflection_test.rs mission's task_1
                // stall: ReadPathAcquired was satisfied by evidence at
                // turn 3, but the hypothesis decayed below 0.15 by turn 6
                // and the node was declared failed with belief 0.0 and no
                // contract check. Belief is passed as 1.0, matching
                // condition_is_met's probing convention, so a
                // MinBeliefScore condition -- the very thing that just
                // decayed to death -- cannot circularly block this check.
                let contract_satisfied_by_evidence =
                    crate::completion_engine::SubgoalCompletionEngine::evaluate(
                        &session.contract,
                        &session.local_evidence,
                        1.0,
                    );

                let requires_semantic_verification =
                    crate::completion_engine::requires_semantic_verification(
                        &session.contract.objective,
                    );

                if contract_satisfied_by_evidence && !requires_semantic_verification {
                    self.log(
                        "\x1b[1;36m>> [CORTEX TERMINAL] No hypothesis survived, but accumulated physical evidence already satisfies the contract. Node complete.\x1b[0m"
                            .to_string(),
                    );
                    return Ok(CortexOutcome::CompletionProposed {
                        belief: 0.95,
                        evidence: session.local_evidence.clone(),
                    });
                }

                if session.compile_remediation.is_active() {
                    Self::rollback_active_mutations(&mut session);
                }

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
                        is_halt_action,
                        mutation_path,
                    )
                    .await?
                }
                CortexState::Complete { belief, evidence } => {
                    return Ok(CortexOutcome::CompletionProposed { belief, evidence })
                }
                CortexState::Fail { belief, .. } => {
                    if session.compile_remediation.is_active() {
                        Self::rollback_active_mutations(&mut session);
                    }

                    // handle_strategize's own no-hypothesis branch routes
                    // here directly with an unconditional return, bypassing
                    // the equivalent contract check at the top of this loop
                    // (which only fires when the budget is already
                    // exhausted before dispatch). Fail is reached the turn
                    // immediately AFTER the sole hypothesis decays below
                    // the 0.15 selection floor, regardless of remaining
                    // budget, so the same evidence-based check must be
                    // repeated here. Confirmed root cause of the
                    // reflection_test.rs mission's task_1 stall persisting
                    // even after the top-of-loop fix: Strategize -> Fail ->
                    // TurnsExhausted happens entirely inside this match
                    // block and never revisits the top-of-loop guard.
                    let contract_satisfied_by_evidence =
                        crate::completion_engine::SubgoalCompletionEngine::evaluate(
                            &session.contract,
                            &session.local_evidence,
                            1.0,
                        );

                    let requires_semantic_verification =
                        crate::completion_engine::requires_semantic_verification(
                            &session.contract.objective,
                        );

                    if contract_satisfied_by_evidence && !requires_semantic_verification {
                        self.log(
                            "\x1b[1;36m>> [CORTEX TERMINAL] No hypothesis survived, but accumulated physical evidence already satisfies the contract. Node complete.\x1b[0m"
                                .to_string(),
                        );
                        return Ok(CortexOutcome::CompletionProposed {
                            belief: belief.max(0.95),
                            evidence: session.local_evidence.clone(),
                        });
                    }

                    // Always return the full accumulated evidence. A bare
                    // diagnostic string (e.g. "No viable hypotheses left.")
                    // is not evidence for any completion condition, and
                    // passing it up to the DAG verifier causes every
                    // criterion to fail even when the physical work was
                    // actually completed successfully.
                    return Ok(CortexOutcome::TurnsExhausted {
                        best_belief: belief,
                        evidence: session.local_evidence.clone(),
                    });
                }
            };
        }
    }

    /// The write phase may be entered only when a declared artifact is
    /// outstanding, reconnaissance is complete, and the mission is not
    /// read-only. The `!contract.read_only` term is currently redundant
    /// with artifact suppression in `infer_contract`, but keeping it
    /// explicit makes the invariant self-documenting: a read-only
    /// mission never enters the write phase, regardless of how artifact
    /// extraction behaves in the future.
    fn write_phase_ready(
        contract: &crate::completion_engine::SubgoalContract,
        unwritten_targets: &[String],
        recon_ready: bool,
    ) -> bool {
        !unwritten_targets.is_empty() && recon_ready && !contract.read_only
    }

    async fn handle_strategize(
        &mut self,
        turn: u32,
        session: &mut EpistemicSession,
        mission: &str,
        world_context: &str,
    ) -> Result<CortexState> {
        let best_node = match session.graph.select_best_hypothesis().cloned() {
            Some(node) => node,
            None => {
                // D (no-selectable-hypothesis recovery): a node must not die
                // merely because every hypothesis decayed below the 0.15
                // selection floor. Log the condition (it was previously a
                // silent turn), then attempt ONE regeneration pass. Content
                // identical to any existing hypothesis -- including the dead
                // one that just fell below the floor -- is skipped, so this
                // can never resurrect a poisoned strategy.
                self.log(
                    "\x1b[1;31m>> [CORTEX] No selectable hypothesis (threshold 0.15). Attempting one regeneration.\x1b[0m"
                        .to_string(),
                );
                session.budget.record_call();
                if let Ok(hyps) = EpistemicEngine::generate_hypotheses(
                    &self.client,
                    &self.api_key,
                    mission,
                    world_context,
                )
                .await
                {
                    for h in hyps {
                        let h_norm = normalize_strategy(&h);
                        let already = session.graph.find_by_content(&h).is_some()
                            || session
                                .graph
                                .nodes
                                .values()
                                .any(|n| normalize_strategy(&n.content) == h_norm);
                        if already {
                            continue;
                        }
                        let hid = session.graph.add_node(NodeKind::Hypothesis, &h, 0.51, 100);
                        session.best_hypothesis_id = Some(hid);
                        self.log(format!(
                            "\x1b[1;36m>> [CORTEX HYPOTHESIS REGENERATION]\x1b[0m Injected `{}`.",
                            h
                        ));
                        return Ok(CortexState::Strategize);
                    }
                }
                self.log(
                    "\x1b[1;31m>> [CORTEX] Regeneration produced no new hypotheses; failing node.\x1b[0m"
                        .to_string(),
                );
                let best_belief = session.graph.best_hypothesis_belief();
                return Ok(CortexState::Fail {
                    belief: best_belief,
                    evidence: "No viable hypotheses left.".to_string(),
                });
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
            &session.contract,
            &session.local_evidence,
            current_belief,
        );

        if status == ContractStatus::Satisfied {
            let command_only_contract = session.contract.conditions.len() == 1
                && matches!(
                    session.contract.conditions.first(),
                    Some(
                        crate::completion_engine::CompletionCondition::CommandExecutionVerified { .. }
                    )
                );

            let requires_semantic_verification =
                crate::completion_engine::requires_semantic_verification(
                    &session.contract.objective,
                );

            if command_only_contract && !requires_semantic_verification {
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

        let evidence_history = crate::cortical::util::compress_context(&session.collect_evidence(h_id), 150000);

        // Phase-aware mandate:
        //
        // A generic recon marker is NOT sufficient to enter the write phase.
        // The deterministic contract must prove every non-output condition
        // first: reads, searches, source evidence, and belief threshold.
        //
        // Only after that gate passes may the strategist be encouraged
        // to produce the required deliverable.

        // Owned strings: this must not borrow `session`, which is passed as
        // `&mut` to the recovery hooks below.
        let unwritten_targets: Vec<String> = session
            .contract
            .required_artifact_paths
            .iter()
            .map(|a| a.path.clone())
            .filter(|p| !crate::evidence_markers::has_artifact_write(&session.local_evidence, p))
            .collect();

        let recon_ready =
            crate::completion_engine::SubgoalCompletionEngine::reconnaissance_complete(
                &session.contract,
                &session.local_evidence,
                current_belief,
            );

        // An outstanding artifact may enter the write phase only after the
        // deterministic reconnaissance contract is actually satisfied.
        // A single successful command is evidence for that command only; it
        // must never bypass the remaining reconnaissance conditions.
        let write_phase_ready =
            Self::write_phase_ready(&session.contract, &unwritten_targets, recon_ready);

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
        let mut active_strategy = if write_phase_ready {
            let target = &unwritten_targets[0];
            let strategy = format!("write_file:{}", target);

            self.log(format!(
                "\x1b[1;35m>> [CORTEX PHASE TRANSITION]\x1b[0m Reconnaissance verified; forcing native deliverable strategy `{}`.",
                strategy
            ));

            strategy
        } else if session.coverage.coverage_pct() >= 99.9
            && session.contract.read_only
            && session.coverage.largest_interior_gap().or_else(|| session.coverage.largest_gap()).is_none()
        {
            let strategy = "halt".to_string();
            self.log(format!(
                "\x1b[1;35m>> [CORTEX PHASE TRANSITION]\x1b[0m Reconnaissance complete; forcing `halt` strategy for synthesis."
            ));
            strategy
        } else {
            h_content.clone()
        };

        let patch_function_strategy = active_strategy.starts_with("patch_function:");

        self.log(format!(
            "[CORTEX RECOVERY CHECK] strategy=`{}` patch_function={} write_phase_ready={} evidence_bytes={} coverage={:.1}% gap={:?}",
            active_strategy,
            patch_function_strategy,
            write_phase_ready,
            session.local_evidence.len(),
            session.coverage.coverage_pct(),
            session.coverage.largest_interior_gap().or_else(|| session.coverage.largest_gap())
        ));

        self.apply_ast_unaddressable_recovery(session, &mut active_strategy);

        self.apply_post_read_mutation_pivot(session, &mut active_strategy, mission, write_phase_ready);

        self.apply_halt_veto_recovery(session);

        self.apply_post_search_expansion(session, &mut active_strategy, current_belief, write_phase_ready);

        // [COVERAGE MANDATE] precedence: any remaining unread range outranks
        // the completion/halt mandate -- the strategist must read the gap
        // before summarizing or halting (the deliverable verifier gates the
        // halt's line claims on this same ledger). Write-phase and
        // critic-veto recovery mandates keep precedence so an in-flight
        // deliverable is never interrupted by a read directive.
        let coverage_gap = session
            .coverage
            .largest_interior_gap()
            .or_else(|| session.coverage.largest_gap());

        // A pinned `read_file:<path>` strategy narrows the strategist's native tool set to
        // read_file only, so a COVERAGE MANDATE that demands read_file_chunk cannot be
        // followed. While a gap is outstanding, pin the chunked read instead.
        if coverage_gap.is_some() && active_strategy.starts_with("read_file:") {
            active_strategy = active_strategy.replacen("read_file:", "read_file_chunk:", 1);
        }

        let enhanced_mission = if write_phase_ready {
            format!(
                "{}\n\n[PHASE MANDATE: RECONNAISSANCE VERIFIED]\nThe deterministic reconnaissance contract has been satisfied. The required deliverable MUST now be produced at '{}' using the native `write_file` action.",
                mission,
                unwritten_targets[0]
            )
        } else if let Some((gap_s, gap_e)) = coverage_gap {
            format!(
                "{}\n\n[COVERAGE MANDATE] Unread evidence range {}-{} of {}. Read this range before proposing any summary or halt. Use read_file_chunk with start_line={} and max_lines={}.",
                mission,
                gap_s,
                gap_e,
                session.coverage.total_lines().unwrap_or(0),
                gap_s,
                (gap_e as i64 - gap_s as i64 + 1)
            )
        } else if session.coverage.coverage_pct() >= 99.9 && session.contract.read_only {
            format!("{}\n\n[PHASE MANDATE: RECONNAISSANCE COMPLETE]\nYou have successfully read 100% of the target files. Do NOT propose any further read actions. You MUST now synthesize your findings and propose the `halt` action.", mission)
        } else if status == ContractStatus::Satisfied && unwritten_targets.is_empty() {
            format!("{}\n\n[PHASE MANDATE: COMPLETION VERIFIED]\nThe deterministic contract has been satisfied. You MUST now propose the `halt` action. Do not propose any further reconnaissance.", mission)
        } else {
            mission.to_string()
        };
        // Deterministic outline for audit reports: read evidence carries no line
        // numbers, so models estimate them (and disagree run to run).
        let enhanced_mission = if session.contract.read_only
            && session.coverage.largest_interior_gap().or_else(|| session.coverage.largest_gap()).is_none()
        {
            let outline = crate::cortical::halt_claims::function_outline(&session.local_evidence, 20);
            if outline.is_empty() { enhanced_mission } else { format!("{}\n\n{}", enhanced_mission, outline) }
        } else {
            enhanced_mission
        };

        let recent_vetoes: Vec<_> = session.local_evidence.lines()
            .rev()
            .take(30)
            .filter(|l| l.contains("[LOOP_VETO]") || l.contains("[PARSE_FAULT]") || l.contains("[CRITIC_VETO") || l.contains("[CRITIC_REPEAT_VETO]") || l.contains("[STRATEGY_VIOLATION]") || l.contains("[HALT_REJECTED") || l.contains("[WRITE_CONFORMANCE_FAILED]"))
            .collect();
        
        let enhanced_mission = if !recent_vetoes.is_empty() {
            let mut rev_vetoes = recent_vetoes.clone();
            rev_vetoes.reverse();
            format!("{}\n\n[SYSTEM OVERRIDES - CRITICAL]\n{}", enhanced_mission, rev_vetoes.join("\n"))
        } else {
            enhanced_mission
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
                {
                    let msg = e.to_string();
                    if msg.contains("429") || msg.contains("Overloaded") {
                        tokio::time::sleep(std::time::Duration::from_secs(20)).await;
                    }
                }
                return Ok(CortexState::Strategize);
            }
        };

        self.dump_debug(turn, "strategist_output", &raw_proposal);
        self.log(format!("\x1b[35m>> [STRATEGIST PROPOSAL]:\x1b[0m {}", raw_proposal));

        // 3. Immediately Extract Typed Actions from Proposal
        let mut actions = match crate::json_parser::extract_actions(&raw_proposal) {
            Ok(parsed) => parsed,
            Err(parse_err) => {
                self.log(format!(
                    "\x1b[31m>> [CORTEX PARSE] extract_actions rejected proposal: {}\x1b[0m",
                    parse_err
                ));
                Vec::new()
            }
        };
        // Reject interceptor results that are structurally incomplete.
        // A SearchCode with an empty query would fail at the executor
        // anyway; rejecting here forces the parse-fault recovery path
        // below to run and feed the schema hint back to the strategist.
        if actions.is_empty() {
            if let Some(intercepted) =
                crate::action_intercept::intercept_fallback_action(&raw_proposal)
            {
                let reject = matches!(
                    &intercepted,
                    AgentAction::SearchCode { _thought_process: None, query, .. } if query.trim().is_empty()
                );
                if !reject {
                    actions.push(intercepted);
                }
            }
        }

        if actions.is_empty() {
            // Synthesize a halt ONLY from prose-only output. A JSON proposal
            // that failed schema validation must fall through to the
            // parse-fault recovery path instead of manufacturing a halt.
            if crate::cortical::util::should_synthesize_halt(&raw_proposal) {
                actions.push(AgentAction::Halt { _thought_process: None, reasoning: "Model declared task complete".to_string(),
                });
            }
        }

        if status == ContractStatus::Satisfied
            && unwritten_targets.is_empty()
            && !actions.iter().any(|a| matches!(a, AgentAction::Halt { _thought_process: None, .. }))
            && !session.local_evidence.contains("[HALT] Deterministic contract is satisfied.")
            // Never auto-halt while the read ledger still has an unread range:
            // a truncated first read satisfies ReadPathAcquired long before the
            // file has actually been read.
            && session
                .coverage
                .largest_interior_gap()
                .or_else(|| session.coverage.largest_gap())
                .is_none()
        {
            self.log("\x1b[1;36m>> [CORTEX] Contract satisfied; automatically injecting Halt action.\x1b[0m".to_string());
            actions = vec![AgentAction::Halt { _thought_process: None, reasoning: "Deterministic contract is satisfied.".to_string(),
            }];
        }

        if actions.is_empty() {
            self.log(
                "\x1b[31m>> [CORTEX FAULT] Could not extract valid action from proposal.\x1b[0m"
                    .to_string(),
            );

            // REGRESSION FIX (parse-fault recovery, 2026-09-13):
            // Without this, a malformed proposal has no recovery path. The
            // strategist sees no feedback in local_evidence (the marker never
            // reaches it), the action attempt counter is never incremented
            // (the action was never constructed, so no action_key exists),
            // and the belief on the active hypothesis is never decayed. The
            // result is that the same malformed proposal is re-issued turn
            // after turn until TurnsExhausted -- see the 2026-09-13 audit
            // mission for the observed 51-turn spin on an end_line/max_lines
            // schema mismatch in a read_file_chunk payload.
            //
            // Mirror the anti-loop recovery path: push a diagnosable marker
            // into session evidence so the strategist can see its own failure
            // on the next turn, and decay the current hypothesis belief so
            // select_best_hypothesis (which requires belief_score >= 0.15) is
            // eventually forced to promote a different competing strategy.
            // Identify the action `type` the model attempted, if any, so the
            // corrective message can name the REAL field names for that exact
            // action instead of a generic "try again" -- reused verbatim from
            // crate::tools::schema::all_tool_declarations(), the same schema
            // supplied to the model's tool-calling interface, so this can
            // never drift out of sync with the AgentAction enum.
            //
            // Confirmed root cause of a full node loss (60 turns, both
            // competing hypotheses decayed to 0.00): a model invented
            // nonexistent fields (`offset`/`limit`) for `read_file_chunk`
            // instead of the real schema (`start_line`/`max_lines`), and the
            // prior parse-fault message never told it what the correct field
            // names were -- it repeated the identical wrong shape every turn
            // until the node died with zero successful reads.
            let attempted_type = raw_proposal
                .find("\"type\"")
                .and_then(|pos| raw_proposal[pos..].find(':').map(|c| pos + c + 1))
                .and_then(|start| {
                    raw_proposal[start..].trim_start().strip_prefix('"').map(str::to_string)
                })
                .and_then(|rest| rest.find('"').map(|end| rest[..end].to_string()));

            let attempted_type = attempted_type.or_else(|| fallback_action_type(&raw_proposal));
            let schema_hint = attempted_type.as_deref().and_then(|t| {
                crate::tools::schema::all_tool_declarations()
                    .into_iter()
                    .find(|decl| decl.name == t)
                    .and_then(|decl| decl.parameters.get("properties").cloned())
                    .and_then(|props| props.as_object().cloned())
                    .map(|obj| {
                        let fields = obj.keys().cloned().collect::<Vec<_>>().join(", ");
                        format!(
                            "\n[PARSE_FAULT_SCHEMA] The `{}` action's ONLY valid fields are: {}. Do not invent field names not in this list.",
                            t, fields
                        )
                    })
            }).unwrap_or_default();

            let parse_fault_obs = format!(
                "[PARSE_FAULT] Proposal could not be deserialized as a valid AgentAction. \
Raw proposal (truncated to 500 chars): {}\n\
[PARSE_FAULT_ACTION] Re-propose using a valid AgentAction type with its required \
fields. Do NOT repeat the rejected proposal verbatim. Use the key \"type\" for the action name and put ALL fields at the top level of the object (never nested under action_input or args).{}",
                raw_proposal.chars().take(500).collect::<String>(),
                schema_hint
            );
            session.local_evidence.push_str(&parse_fault_obs);
            session.local_evidence.push('\n');

            if let Some(h_id) = session.best_hypothesis_id {
                if let Some(new_belief) = session.graph.decay(h_id, 0.05) {
                    self.log(format!(
                        "\x1b[1;31m>> [PARSE_FAULT] Hypothesis belief decayed to {:.2} to force pivot\x1b[0m",
                        new_belief
                    ));
                }
            }

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
            if let Some(authoritative_query) = crate::cortical::util::extract_explicit_search_memory_query(mission) {
                if let Some(AgentAction::SearchMemory { _thought_process: None, query, .. }) = actions.first_mut() {
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

        // Compatibility is checked against the strategy the model was
        // actually asked to execute. When write_phase_ready forced a
        // write_file strategy, active_strategy differs from h_content;
        // comparing against h_content rejected correct write_file
        // proposals as STRATEGY VIOLATIONS. When write_phase_ready is
        // false, active_strategy == h_content and behavior is unchanged.
        if let Some(first_action) = actions.first() {
            let strategy_ok = crate::cortical::strategy_match::action_matches_strategy(
                &active_strategy,
                first_action,
            );
            let advances_unmet =
                crate::cortical::contract_advance::action_advances_unmet_contract_condition(first_action, session, mission);
            // A critic veto may demand a different RECON action than the pinned
            // strategy (e.g. "run one grep instead of chunked reads"). Without
            // this, the veto and the strategy gate deadlock and the node dies
            // with belief 0.00. Only non-mutating actions are admitted; every
            // mutation still goes through the strict strategy match.
            let veto_recon_ok = session.local_evidence.contains("[CRITIC_VETO]")
                && match first_action {
                    AgentAction::ReadFile { _thought_process: None, .. }
                    | AgentAction::ReadFileChunk { _thought_process: None, .. }
                    | AgentAction::ReadFunction { _thought_process: None, .. }
                    | AgentAction::SearchCode { _thought_process: None, .. }
                    | AgentAction::SearchMemory { _thought_process: None, .. } => true,
                    AgentAction::ExecuteCommand { _thought_process: None, command, .. } => {
                        crate::cortical::read_only_gate::is_read_only_safe_command(command)
                    }
                    _ => false,
                };
            if !strategy_ok && !advances_unmet && !veto_recon_ok {
                self.log(format!(
                    "\x1b[1;31m>> [STRATEGY VIOLATION]\x1b[0m Selected strategy '{}' produced incompatible action '{:?}'. Rejecting proposal and forcing re-strategy.",
                    active_strategy,
                    first_action
                ));

                if let Some(h_id) = session.best_hypothesis_id {
                    Self::decay_hypothesis_belief(session, h_id, 0.20);
                    if let Some(node) = session.graph.nodes.get(&h_id) {
                        self.log(format!(
                            "\x1b[1;31m>> [STRATEGY_VIOLATION] Hypothesis belief decayed to {:.2} to force pivot\x1b[0m",
                            node.belief_score
                        ));
                    }
                }

                let violation_obs = format!(
                    "[STRATEGY_VIOLATION] Selected strategy `{}` \
                     produced an incompatible action. Proposal rejected; \
                     underlying hypothesis decayed to allow strategy pivot.",
                    h_content
                );

                session.local_evidence.push_str(&violation_obs);
                session.local_evidence.push('\n');

                return Ok(CortexState::Strategize);
            }
        }

        Ok(CortexState::Critique { actions, proposal: raw_proposal, belief: current_belief })
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
    ) -> Result<CortexState> {
        let status = ContractEvaluator::evaluate(
            &session.contract,
            &session.local_evidence,
            belief,
        );

        if status == ContractStatus::Satisfied {
            self.log(
                "\x1b[1;33m>> [SUBGOAL_COMPLETE] Non-mutating subgoal satisfied. Mission remains active.\x1b[0m"
                    .to_string(),
            );
        }

        let h_content = if let Some(h_id) = session.best_hypothesis_id {
            session.graph.content_of(h_id)
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
            let action_key = crate::cortical::action_identity::action_key(&action);

            let attempt_count = session.global_action_attempt_count(&action_key);

            // STRICT ANTI-LOOP:
            // An identical action that has already crossed the execution
            // boundary is not new evidence. Drop it from the batch, but
            // let never-attempted sibling actions in the same proposal
            // proceed -- one stale action must not starve the rest.
            //
            // EXCEPTION — active compile remediation:
            // While a retention episode is open, the on-disk source is
            // known to have changed since the previously-recorded
            // attempt (the compiler failure left the source in a state
            // that the same patch would now correct). A repeated
            // PatchFunction is therefore a materially different action
            // against a different pre-image, even though its new_body
            // hashes identically. Permit it through the anti-loop filter
            // so the corrective patch the retention path instructs the
            // strategist to produce can actually execute.
            let bypass_antiloop_for_retention = session.compile_remediation.is_active()
                && matches!(
                    action,
                    AgentAction::PatchFunction { _thought_process: None, .. }
                        | AgentAction::ReadFunction { _thought_process: None, .. }
                        | AgentAction::ReadFileChunk { _thought_process: None, .. }
                        | AgentAction::ReadFile { _thought_process: None, .. }
                );

            if attempt_count >= 1 && !bypass_antiloop_for_retention {
                match crate::anti_loop::classify_duplicate(
                    &action,
                    &session.contract,
                    &session.local_evidence,
                ) {
                    crate::anti_loop::FilterOutcome::ConditionSatisfied => {
                        self.log(
                            "\x1b[1;32m>> [ANTI-LOOP RESOLVED] Required command already executed; condition satisfied.\x1b[0m"
                                .to_string(),
                        );
                        filtered_actions.push(action);
                    }
                    crate::anti_loop::FilterOutcome::AllowRetry => {
                        self.log(
                            "\x1b[1;33m>> [ANTI-LOOP] Duplicate required by contract with no marker; permitting retry.\x1b[0m"
                                .to_string(),
                        );
                        filtered_actions.push(action);
                    }
                    crate::anti_loop::FilterOutcome::FilterAndDecay => {
                        self.log(format!(
                            "\x1b[1;31m>> [ANTI-LOOP] Duplicate action [{}] filtered from batch.\x1b[0m",
                            action_key
                        ));
                        vetoed_keys.push(action_key);
                    }
                }
            } else {
                if attempt_count >= 1 {
                    self.log(format!(
                        "\x1b[1;33m>> [ANTI-LOOP] Permitting repeated action [{}] during active compile remediation.\x1b[0m",
                        action_key
                    ));
                }
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
                Self::decay_hypothesis_belief(session, h_id, 0.20);
                if let Some(node) = session.graph.nodes.get(&h_id) {
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

            // STUCK-NODE BUDGET: repeated all-duplicate batches mean the strategist is
            // re-proposing already-attempted actions with no new evidence. Fail the node
            // honestly instead of spinning to the turn limit.
            const LOOP_VETO_BUDGET: usize = 6;
            let stuck_count = session
                .local_evidence
                .lines()
                .filter(|l| l.starts_with("[LOOP_VETO] All proposed actions already attempted"))
                .count();
            if stuck_count >= LOOP_VETO_BUDGET {
                self.log(format!(
                    "\x1b[1;31m>> [STUCK NODE] {} all-duplicate proposal batches with no new evidence. Failing node early.\x1b[0m",
                    stuck_count
                ));
                session.local_evidence.push_str(
                    "[STUCK_NODE] Repeated duplicate proposals with no new evidence; the objective may be unachievable in this environment (missing dependency or unsupported platform).\n",
                );
                return Ok(CortexState::Fail {
                    belief: session.graph.best_hypothesis_belief(),
                    evidence: "Stuck: repeated duplicate proposals with no new evidence.".to_string(),
                });
            }

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

        // NEXUS_FIX_LEDGER_BYPASS: next unread chunk is deterministic pagination; skip the critic
        if session.contract.read_only && actions.len() == 1 {
            if let Some((gap_start, _)) = session
                .coverage
                .largest_interior_gap()
                .or_else(|| session.coverage.largest_gap())
            {
                let is_next_chunk = matches!(
                    &actions[0],
                    AgentAction::ReadFileChunk { _thought_process: None, start_line, .. }
                        if (*start_line as u64) == (gap_start as u64)
                );
                if is_next_chunk {
                    self.log(
                        "\x1b[1;36m>> [CRITIC BYPASS] Ledger-authoritative pagination; critic skipped.\x1b[0m"
                            .to_string(),
                    );
                    session.last_critic_rejected_actions = None;
                    return Ok(CortexState::Execute { actions, proposal, belief });
                }
            }
        }

        // HALT PRE-GATE (read-only audits): the LLM critic is not authoritative on
        // halt reports (it has approved garbage and vetoed good reports from stale
        // evidence text). Symbols, substance and cited line ranges are checked
        // against the physical read ledger instead.
        let halt_check = match actions.first() {
            Some(AgentAction::Halt { reasoning, .. }) => Some(
                crate::cortical::halt_claims::halt_defects(reasoning, &session.local_evidence),
            ),
            _ => None,
        };
        if session.contract.read_only && actions.len() == 1 {
            if let Some(mut defects) = halt_check {
                // Truncation guard: a report cut off mid-sentence (output token cap)
                // must not pass on length alone.
                if let Some(AgentAction::Halt { reasoning, .. }) = actions.first() {
                    let tail = reasoning.trim_end();
                    if !tail.ends_with(|c: char| matches!(c, '.' | ')' | '!' | '`' | '"')) {
                        defects.push(format!(
                            "report appears truncated (ends with {:?}); finish the final sentence",
                            tail.chars().rev().take(12).collect::<Vec<_>>().into_iter().rev().collect::<String>()
                        ));
                    }
                }
                // COVERAGE GATE: a halt report is not acceptable while most of the file is unread.
                // The strategist's own claim of completeness is not evidence; the ledger is. This
                // mirrors the prompt's >=90% rule and only applies once a file read has
                // established a total line count.
                if let Some(total) = session.coverage.total_lines() {
                    let pct = session.coverage.coverage_pct();
                    if pct < 90.0 {
                        let gap = session
                            .coverage
                            .largest_interior_gap()
                            .or_else(|| session.coverage.largest_gap());
                        defects.push(format!(
                            "only {:.1}% of the {} lines have been read{}; read the remaining range with read_file_chunk before halting",
                            pct,
                            total,
                            gap.map(|(s, e)| format!(" (unread {}-{})", s, e)).unwrap_or_default()
                        ));
                    }
                }

                if defects.is_empty() {
                    self.log("\x1b[1;36m>> [CRITIC BYPASS] Halt report passed deterministic symbol, substance and line-range checks; LLM critic skipped.\x1b[0m".to_string());
                    session.last_critic_rejected_actions = None;
                    return Ok(CortexState::Execute { actions, proposal, belief });
                }
                self.log(format!("\x1b[1;31m>> [HALT PRE-GATE] Rejecting halt: {:?}\x1b[0m", defects));
                session.local_evidence.push_str(&format!(
                    "\n[HALT_REJECTED_REPORT] The halt `reasoning` field IS the deliverable. Fix these defects and re-propose the full audit: {}\n",
                    defects.join("; ")
                ));
                return Ok(CortexState::Strategize);
            }
        }

        // HARD CRITIC BOUNDARY:
        // A critic veto applies to the exact structured action batch that
        // the critic actually evaluated. Historical [CRITIC_VETO] text in
        // local_evidence is feedback, not a timeless control latch.
        //
        // Compare only after anti-loop filtering because `actions` here is
        // the exact batch that would otherwise cross the critic boundary.
        // action_key() is the canonical action identity, including the
        // PatchFunction new_body where applicable.
        let current_action_keys = actions
            .iter()
            .map(crate::cortical::action_identity::action_key)
            .collect::<Vec<_>>();

        if session.last_critic_rejected_actions.as_ref() == Some(&current_action_keys) {
            self.log(
                "\x1b[1;31m>> [CRITIC REPEAT VETO] Exact previously rejected action batch proposed again; rejecting before critic re-evaluation.\x1b[0m"
                    .to_string(),
            );

            session.local_evidence.push_str(
                "[CRITIC_REPEAT_VETO] Exact action batch previously rejected by the Adversarial Critic was proposed again. Select a materially different action batch.\n",
            );

            if let Some(h_id) = session.best_hypothesis_id {
                Self::decay_hypothesis_belief(session, h_id, 0.20);
                if let Some(node) = session.graph.nodes.get(&h_id) {
                    self.log(format!(
                        "\x1b[1;31m>> [CRITIC_REPEAT_VETO] Hypothesis belief decayed to {:.2} to force pivot\x1b[0m",
                        node.belief_score
                    ));
                }
            }

            return Ok(CortexState::Strategize);
        }

        // Payload guard: patch_function replaces a whole function item, so
        // new_body must contain that function's signature. Comment-only or
        // body-only payloads are rejected before critic/executor.
        for a in &actions {
            if let AgentAction::PatchFunction { _thought_process: None, function_name, new_body, .. } = a {
                let needle = format!("fn {}", function_name.trim());
                if !new_body.contains(&needle) {
                    self.log(format!(
                        "\x1b[1;31m>> [PATCH PAYLOAD REJECTED]\x1b[0m new_body for `{}` has no `{}` item.",
                        function_name, needle
                    ));
                    session.local_evidence.push_str(&format!(
                        "[PATCH_PAYLOAD_REJECTED] new_body must be the COMPLETE function item including its signature (`{}`), with any doc comment above it. Comment-only or body-only payloads are invalid.\n",
                        needle
                    ));
                    if let Some(h_id) = session.best_hypothesis_id {
                        Self::decay_hypothesis_belief(session, h_id, 0.10);
                    }
                    return Ok(CortexState::Strategize);
                }
            }
        }

        // Deterministic PATCH_FUNCTION mutation fact check.
        //
        // The Adversarial Critic must remain responsible for safety and
        // mission-compliance judgments, but it must not be allowed to
        // misclassify a physically real AST mutation as a no-op.
        //
        // Use the exact same ReplaceFunctionBody strategy that the executor
        // will use. This is validation only: nothing is written here.
        let mut patch_mutation_verified = false;

        for action in &actions {
            match classify_patch_action(action).await {
                Ok(PatchClass::Mutation { path, function_name }) => {
                    patch_mutation_verified = true;
                    self.log(format!(
                        "[PATCH_MUTATION_VERIFIED] {}:{} produces a real source mutation.",
                        path, function_name
                    ));
                    session.local_evidence.push_str(&format!(
                        "[PATCH_MUTATION_VERIFIED] {}:{}\n",
                        path, function_name
                    ));
                }
                Ok(PatchClass::Idempotent { path, function_name }) => {
                    self.log(format!(
                        "[PATCH_IDEMPOTENT_OK] {}:{} requested function body is already satisfied.",
                        path, function_name
                    ));
                    // REGRESSION FIX (idempotent-in-compound-mission):
                    // Do NOT push [PATCH_IDEMPOTENT_OK] into session evidence
                    // here. This is CRITIC time: no file write has occurred and
                    // no compiler check has run. The top-of-loop terminal gate
                    // treats [PATCH_IDEMPOTENT_OK] as a patch-contract satisfier,
                    // so pushing it now short-circuits Execute on the next turn --
                    // the executor never emits its own copy, the compiler critic
                    // never fires, and a mandatory [CMD_OK] criterion
                    // (e.g. `cargo check -p <pkg>`) can never be satisfied.
                    // The executor emits the identical marker after the action
                    // crosses the execution boundary (see
                    // RoboticExecutor::execute_motor_command); that post-execution
                    // emission is the one that must enter evidence.
                }
                Ok(PatchClass::NotPatch) => {}
                Err(e) => {
                    self.log(format!(
                        "[PATCH_MUTATION_CHECK] Unable to validate patch action: {}",
                        e
                    ));
                }
            }
        }

        self.log("\x1b[31m>> [RED TEAM] Criticizing proposal...\x1b[0m".to_string());
        session.budget.record_call();

        // NEXUS_FIX_LEDGER_NOTE: deterministic read ledger as authoritative fact for the critic
        let critic_evidence = match session
            .coverage
            .largest_interior_gap()
            .or_else(|| session.coverage.largest_gap())
        {
            Some((gap_s, gap_e)) => format!(
                "{}\n\n[COVERAGE LEDGER - AUTHORITATIVE] Deterministic read ledger: unread range {}-{} of {} (coverage {:.1}%). A read_file_chunk starting at line {} is the correct next step and MUST NOT be vetoed as skipping lines.",
                session.local_evidence,
                gap_s,
                gap_e,
                session.coverage.total_lines().unwrap_or(0),
                session.coverage.coverage_pct(),
                gap_s
            ),
            None => match session.coverage.total_lines() {
                Some(total) => format!(
                    "[COVERAGE LEDGER - AUTHORITATIVE] 100% of {} lines have been read; no unread range exists. Do NOT veto for unread lines.\n{}\n[COVERAGE LEDGER - AUTHORITATIVE] 100% of {} lines read.",
                    total, session.local_evidence, total
                ),
                None => session.local_evidence.clone(),
            },
        };
        // NEXUS_FIX_PATH_FACTS: filesystem ground truth for read-path vetoes
        let path_facts: String = actions
            .iter()
            .filter_map(|a| match a {
                AgentAction::ReadFile { _thought_process: None, path, .. } | AgentAction::ReadFileChunk { _thought_process: None, path, .. } => {
                    Some(format!(
                        "\n[FILESYSTEM FACT - AUTHORITATIVE] `{}` {} in the workspace.",
                        path,
                        if std::path::Path::new(path).exists() { "EXISTS" } else { "does NOT exist" }
                    ))
                }
                _ => None,
            })
            .collect();
        let critic_evidence = format!("{}{}", critic_evidence, path_facts);
        match EpistemicEngine::criticize_proposal(
            &self.client,
            &self.api_key,
            mission,
            &h_content,
            &proposal,
            world_context,
            &critic_evidence,
        )
        .await
        {
            Err(e) => {
                self.log(format!("\x1b[33m>> [CRITIC BYPASS] Parser error ({}). Proceeding with execution.\x1b[0m", e));
                if actions.iter().any(|a| matches!(a, AgentAction::Halt { .. })) {
                    self.log("\x1b[1;31m>> [CRITIC UNAVAILABLE] Refusing to execute a halt without a critic verdict. Re-strategizing.\x1b[0m".to_string());
                    return Ok(CortexState::Strategize);
                }
                    session.last_critic_rejected_actions = None;
                    Ok(CortexState::Execute { actions, proposal, belief })
            }
            Ok(critic) => {
                self.dump_debug(turn, "critic_output", &format!("{:?}", critic));
                self.log(format!(
                    "\x1b[31m>> [CRITIC] Verdict:\x1b[0m {} (Modifier: {:.2})",
                    critic.critique, critic.confidence_modifier
                ));

                // A deterministic source-level mutation fact outranks an
                // LLM claim that the patch is merely a no-op. Keep every
                // other critic veto intact.
                let critic_text = critic.critique.to_ascii_lowercase();
                let critic_claims_noop = critic_text.contains("no-op")
                    || critic_text.contains("no op")
                    || critic_text.contains("noop")
                    || critic_text.contains("identical")
                    || critic_text.contains("unchanged")
                    || critic_text.contains("same implementation");

                // A search_code action is not a read_file action, even if
                // the same file was previously read. The critic's
                // anti-redundant-READ rule is written for read_file and
                // must not be applied to a search: a mandatory
                // SEARCH_CODE_OK criterion requires a physical
                // search_code execution, and the file being already
                // read does not satisfy that criterion. Confirmed root
                // cause of the audit mission's turn-14 abort, where the
                // critic hallucinated a search_code proposal as a
                // read_file and vetoed it as redundant.
                let action_is_search =
                    matches!(actions.first(), Some(AgentAction::SearchCode { _thought_process: None, .. }));
                let strategy_is_search = h_content.starts_with("search_code:");
                let critic_veto_is_redundancy = critic_text.contains("redundant")
                    || critic_text.contains("already read")
                    || critic_text.contains("already contains")
                    || critic_text.contains("proceed immediately to writing")
                    || critic_text.contains("proceed immediately to patching");

                let action_is_write =
                    matches!(actions.first(), Some(AgentAction::WriteFile { _thought_process: None, .. }));
                let strategy_is_write = h_content.starts_with("write_file:");
                let critic_veto_is_pre_recon = critic_text.contains("prior evidence")
                    || critic_text.contains("search_code")
                    || critic_text.contains("read_file")
                    || critic_text.contains("before writing")
                    || critic_text.contains("information gathering")
                    || critic_text.contains("without prior evidence");

                if critic.confidence_modifier <= -0.25
                    && action_is_search
                    && strategy_is_search
                    && critic_veto_is_redundancy
                {
                    self.log(
                        "\x1b[1;36m>> [CRITIC FACT OVERRIDE] search_code action preserved despite anti-redundant-READ veto; a search is a distinct operation from a file read.\x1b[0m"
                            .to_string(),
                    );
                    session.local_evidence.push_str(
                        "[CRITIC_FACT_OVERRIDE] search_code preserved; anti-redundant-READ rule does not apply to search actions.\n",
                    );
                        session.last_critic_rejected_actions = None;
                    Ok(CortexState::Execute { actions, proposal, belief })
                } else if critic.confidence_modifier <= -0.25
                    && action_is_write
                    && strategy_is_write
                    && critic_veto_is_pre_recon
                {
                    self.log(
                        "\x1b[1;36m>> [CRITIC FACT OVERRIDE] write_file preserved; active strategy is authoritative and context is self-contained.\x1b[0m"
                            .to_string(),
                    );
                    session.local_evidence.push_str(
                        "[CRITIC_FACT_OVERRIDE] write_file action preserved against pre-recon veto; active strategy is authoritative.\n",
                    );
                        session.last_critic_rejected_actions = None;
                    Ok(CortexState::Execute { actions, proposal, belief })
                } else if critic.confidence_modifier <= -0.25
                    && patch_mutation_verified
                    && critic_claims_noop
                {
                    self.log(
                        "\x1b[1;36m>> [CRITIC FACT OVERRIDE] Deterministic AST validation proves the patch is a real source mutation; ignoring the critic's no-op veto.\x1b[0m"
                            .to_string(),
                    );
                    session.local_evidence.push_str(
                        "[CRITIC_FACT_OVERRIDE] PATCH_FUNCTION mutation verified by ReplaceFunctionBody; critic no-op classification rejected.\n",
                    );
                        session.last_critic_rejected_actions = None;
                    Ok(CortexState::Execute { actions, proposal, belief })
                } else if critic.confidence_modifier > -0.25
                    && !(action_is_write
                        && critic.confidence_modifier < 0.0
                        && (critic_text.contains("incomplete")
                            || critic_text.contains("only writes")))
                {
                        session.last_critic_rejected_actions = None;
                    Ok(CortexState::Execute { actions, proposal, belief })
                } else {
                    self.log("\x1b[33m>> [DIALECTIC FEEDBACK] Veto registered in graph. Re-strategizing...\x1b[0m".to_string());

                    // Preserve the exact proposal identity that the critic
                    // actually rejected. `actions` is already the filtered
                    // batch presented to the critic, so this must not use the
                    // original strategist batch.
                    let rejected_action_keys = actions
                        .iter()
                        .map(crate::cortical::action_identity::action_key)
                        .collect::<Vec<_>>();

                    session.last_critic_rejected_actions = Some(rejected_action_keys);

                    // Feed the critic's rejection back into the next
                    // Strategize -> Propose cycle. Graph-only feedback is not
                    // sufficient because the strategist receives local_evidence.
                    let veto_feedback = format!(
                        "\n[CRITIC_VETO] {} [CRITIC_VETO_ACTION] The rejected proposal MUST NOT be repeated. Select a materially different, mission-compliant action.\n",
                        critic.critique.replace('\n', " ")
                    );
                    session.local_evidence.push_str(&veto_feedback);

                    if let Some(h_id) = session.best_hypothesis_id {
                        let obs_id = session.graph.add_node(
                            NodeKind::Observation,
                            &format!("[CRITIC_VETO] {}", critic.critique),
                            1.0,
                            75,
                        );
                        session.graph.add_edge(obs_id, h_id, EdgeRelation::Contradicts, if action_is_write && strategy_is_write { 0.10 } else { 0.40 });
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
        // Session-scoped episode. session_id comes from the active
        // EpistemicSession; plan_id/task_id come from the
        // caller-set self.episode.
        let session_episode = EpisodeContext {
            session_id: Some(session.session_id.to_string()),
            plan_id: self.episode.plan_id.clone(),
            task_id: self.episode.task_id.clone(),
            mission_id: self.episode.mission_id.clone(),
        };
        let mut executor = RoboticExecutor::new(self.api_key.clone(), self.tx.clone());
        executor.episode = session_episode;
        let mut latest_evidence = String::new();
        let mut executed_actions: Vec<String> = Vec::new();
        let mut mutation_paths: Vec<String> = Vec::new();
        let mut last_is_halt = false;
        let mut rust_files_mutated = false;
        let evidence_checkpoint = session.local_evidence.len();

        for action in actions {
            // READ-ONLY MISSION ENFORCEMENT (absolute).
            //
            // When the mission contract declares read-only intent, refuse
            // every action that could mutate the substrate -- before the
            // ActionGate is consulted. This is an absolute block: it does
            // not depend on the strategist's proposal, the critic's
            // approval, or the planner's DAG shape. A mission that
            // declares read-only intent cannot write, patch, or
            // synthesize, regardless of what later task text says.
            //
            // The pattern is the same as the ActionGate rejection path a
            // few lines below: log, push evidence, decay belief, and
            // continue to the next action. If every action in the batch
            // is refused, `latest_evidence` stays empty and the loop
            // returns to Strategize without executing anything.
            if session.contract.read_only
                && crate::cortical::read_only_gate::violates_read_only(&action)
            {
                self.log(format!(
                    "\x1b[1;31m>> [READ-ONLY VIOLATION]\x1b[0m Mission declares read-only intent; refusing mutation action: {:?}",
                    action
                ));
                session.local_evidence.push_str(
                    "[READ_ONLY_VIOLATION] Mission is READ-ONLY. Write/patch/synthesize actions are not permitted. Provide the audit findings as a halt action instead.\n",
                );
                if let Some(h_id) = session.best_hypothesis_id {
                    let obs_id = session.graph.add_node(
                        NodeKind::Observation,
                        "[READ_ONLY_VIOLATION] Write action refused by READ-ONLY contract.",
                        1.0,
                        50,
                    );
                    session.graph.add_edge(obs_id, h_id, EdgeRelation::Contradicts, 0.50);
                    session.graph.propagate_beliefs();
                }
                continue;
            }

            let intent = IntentContract::new(&proposal, belief);
            let capability = capability_for(&action);

            if let AgentAction::AtomicPatch { _thought_process: None, patches, .. } = &action {
                for p in patches {
                    mutation_paths.push(p.path.clone());
                    if p.path.ends_with(".rs") {
                        rust_files_mutated = true;
                    }
                }
            }

            if let AgentAction::ExecuteCommand { _thought_process: None, command, .. } = &action {
                // Defensive: an arbitrary shell command (e.g. invoking an external
                // tool like nexus_ast_tool) can mutate a .rs file without going
                // through any recognized AgentAction write variant. If the command
                // string references a .rs path, treat it as a possible Rust
                // mutation so the compiler-critic gate below still runs -- silent
                // corruption from an external tool must not skip verification
                // just because it didn't arrive as a WriteFile/DiffPatch/etc.
                // Only commands that can plausibly write count as mutations.
                // Read-only commands (find, grep, cat, ls) that merely name a
                // .rs file or glob must not trigger compiler verification.
                let cmd_may_write = [
                    ">",
                    "sed -i",
                    "tee ",
                    "mv ",
                    "cp ",
                    "rm ",
                    "patch ",
                    "rustfmt",
                    "cargo fix",
                    "cargo fmt",
                    "nexus_ast_tool",
                ]
                .iter()
                .any(|m| command.contains(m));
                for token in command.split_whitespace() {
                    let clean = token.trim_matches(|c: char| {
                        !c.is_alphanumeric() && c != '.' && c != '/' && c != '_' && c != '-'
                    });
                    if cmd_may_write && clean.len() > 3 && clean.ends_with(".rs") {
                        mutation_paths.push(clean.to_string());
                        rust_files_mutated = true;
                    }
                }
            }
            let mutation_path = match &action {
                AgentAction::WriteFile { _thought_process: None, path, .. }
                | AgentAction::DiffPatch { _thought_process: None, path, .. }
                | AgentAction::PatchFunction { _thought_process: None, path, .. }
                | AgentAction::ReplaceLines { _thought_process: None, path, .. } => Some(path.clone()),
                AgentAction::SynthesizeReport { _thought_process: None, output_path, .. } => Some(output_path.clone()),
                _ => None,
            };

            if let Some(path) = mutation_path {
                mutation_paths.push(path.clone());
                if path.ends_with(".rs") {
                    rust_files_mutated = true;
                }
            }

            if matches!(action, AgentAction::Halt { _thought_process: None, .. }) {
                last_is_halt = true;
            }

            // Serialize the exact AgentAction before MotorCortex::translate()
            // consumes it. This is the authoritative action record that will
            // cross the execution/evaluation boundary.
            let executed_action_json = serde_json::to_string(&action)
                .map_err(|e| anyhow::anyhow!("Failed to serialize executed AgentAction: {}", e))?;

            // Build the canonical action key before MotorCortex::translate()
            // consumes the AgentAction.
            let action_key = crate::cortical::action_identity::action_key(&action);

            // Captured before the move below so a failed execution's error
            // string can be scoped to the attempted path (see error-arm
            // usage further down). Path-scoping lets downstream path-bound
            // completion conditions (e.g. ReadPathAcquired) recognize a
            // confirmed-absent target as valid inspection evidence.
            let attempted_path: Option<String> = match &action {
                AgentAction::ReadFile { _thought_process: None, path, .. }
                | AgentAction::ReadFileChunk { _thought_process: None, path, .. }
                | AgentAction::WriteFile { _thought_process: None, path, .. }
                | AgentAction::DiffPatch { _thought_process: None, path, .. }
                | AgentAction::ReplaceLines { _thought_process: None, path, .. } => Some(path.trim().to_string()),
                AgentAction::ReadFunction { _thought_process: None, path, .. }
                | AgentAction::PatchFunction { _thought_process: None, path, .. } => Some(path.trim().to_string()),
                _ => None,
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
                Err(e) => {
                    // attempted_path was captured before `action` was moved
                    // into MotorCortex::translate() above.
                    // A rejected action wrote nothing: it must not reach the
                    // post-mutation compile gate or the revert path.
                    if let Some(p) = &attempted_path {
                        mutation_paths.retain(|m| m != p);
                    }
                    match &attempted_path {
                        Some(p) => format!("[ACTION FAILED] path={} OS rejected action: {}", p, e),
                        None => format!("[ACTION FAILED] OS rejected action: {}", e),
                    }
                }
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

        // POST-WRITE CONFORMANCE GATE (Python deliverables).
        // [WRITE_OK] proves a file was written, not that it satisfies the
        // objective. Confirmed root cause of the sys_monitor.py false success:
        // a one-line import stub produced [WRITE_OK], the path-only contract
        // was satisfied, and the node completed with no further verification.
        // Requirements named in the objective are checked deterministically
        // against the file on disk; on failure the write evidence is discarded
        // so the contract stays unsatisfied (and reflection never sees it).
        if !session.contract.read_only && latest_evidence.contains("[WRITE_OK]") {
            let objective = session.contract.objective.clone();
            let mut failures: Vec<String> = Vec::new();
            for p in mutation_paths.iter().filter(|p| p.ends_with(".py")) {
                let Ok(content) = tokio::fs::read_to_string(p).await else { continue };
                let mut defects = python_conformance_defects(&objective, p, &content);
                if let Some(err) = python_syntax_error(p).await {
                    defects.push(format!("syntax error: {}", err));
                }
                if !defects.is_empty() {
                    failures.push(format!("`{}`: {}", p, defects.join("; ")));
                }
            }
            if !failures.is_empty() {
                let message = format!(
                    "[WRITE_CONFORMANCE_FAILED] The file was written but does not satisfy the mission: {}. Rewrite the COMPLETE file in ONE write_file call so every requirement is met.",
                    failures.join(" | ")
                );
                self.log(format!("\x1b[1;31m>> [WRITE CONFORMANCE] {}\x1b[0m", message));
                session.local_evidence.truncate(evidence_checkpoint);
                session.local_evidence.push_str(&message);
                session.local_evidence.push('\n');
                return Ok(CortexState::Strategize);
            }
        }

        if latest_evidence.trim().is_empty() {
            return Ok(CortexState::Strategize);
        }

        // Guard: only genuine *.rs file paths may trigger compiler verification.
        // Command text such as `find . -name "*.rs"` must not register as a mutation.
        let mutation_paths: Vec<_> = mutation_paths
            .iter()
            .filter(|p| {
                let pp = std::path::Path::new(*p);
                pp.extension().map(|e| e == "rs").unwrap_or(false)
            })
            .cloned()
            .collect();
        let rust_files_mutated = rust_files_mutated && !mutation_paths.is_empty();

        if rust_files_mutated {
            self.log(
                "\x1b[36m>> [COMPILER CRITIC] Resolving mutated Rust package(s) via cargo metadata...\x1b[0m"
                    .to_string(),
            );

            let target = match CompilerGate::resolve_target(&mutation_paths).await {
                Ok(target) => target,
                Err(error) => {
                    let message = format!("[COMPILER_FAULT] {}", error);
                    self.log(format!("\x1b[1;31m>> [COMPILER CRITIC] {}\x1b[0m", message));
                    session.local_evidence.truncate(evidence_checkpoint);
                    session.local_evidence.push_str(&message);
                    session.local_evidence.push('\n');
                    return Ok(CortexState::Strategize);
                }
            };

            let packages = match &target {
                RustVerificationTarget::WorkspacePackages { packages, .. } => packages.clone(),
                RustVerificationTarget::StandaloneFiles { files } => {
                    files.iter().map(|f| f.display().to_string()).collect()
                }
                RustVerificationTarget::NoVerificationNeeded => Vec::new(),
            };

            self.log(format!(
                "\x1b[36m>> [COMPILER CRITIC] Verification target(s): {}\x1b[0m",
                if packages.is_empty() { "none".to_string() } else { packages.join(", ") }
            ));

            match &target {
                RustVerificationTarget::WorkspacePackages { workspace_manifest, .. } => {
                    self.log(format!(
                        "\x1b[36m>> [COMPILER CRITIC] Running package-scoped cargo check through CompilerGate using {}...\x1b[0m",
                        workspace_manifest.display()
                    ));
                }
                RustVerificationTarget::StandaloneFiles { files } => {
                    let names: Vec<String> =
                        files.iter().map(|f| f.display().to_string()).collect();
                    self.log(format!(
                        "\x1b[36m>> [COMPILER CRITIC] Running standalone rustc verification through CompilerGate for [{}]...\x1b[0m",
                        names.join(", ")
                    ));
                }
                RustVerificationTarget::NoVerificationNeeded => {
                    self.log(
                        "\x1b[36m>> [COMPILER CRITIC] No physical compiler verification required.\x1b[0m"
                            .to_string(),
                    );
                }
            }

            let compile_result = CompilerGate::verify_target(&target).await;

            match compile_result {
                CompilerGateResult::Success { success_marker, targets_checked: _ } => {
                    session.compile_remediation.reset();

                    let mut success_evidence = String::new();
                    success_evidence.push('\n');
                    success_evidence.push_str(&success_marker);

                    // Mirror to both current-turn evidence and the session
                    // evidence buffer. The top-of-loop completion check and
                    // the DAG verifier both read session.local_evidence; the
                    // LLM evaluator reads latest_evidence.
                    latest_evidence.push_str(&success_evidence);
                    session.local_evidence.push_str(&success_evidence);

                    self.log(
                        "\x1b[32m>> [COMPILER CRITIC] Physical compiler verification OK. Proceeding to LLM Evaluator.\x1b[0m"
                            .to_string(),
                    );
                }

                CompilerGateResult::CompilationFailed {
                    error_preview: err_preview,
                    failed_targets: _,
                } => {
                    let decision = session.compile_remediation.classify(false);

                    match &decision {
                        CompileDecision::RetainForRetry { iteration } => {
                            session.active_mutation_paths = mutation_paths.clone();
                            self.log(format!(
                                "\x1b[1;33m>> [COMPILER CRITIC] Physical compile verification FAILED (iteration {}/{}). Retaining mutation; feeding diagnostic to strategist.\x1b[0m",
                                iteration,
                                COMPILE_RETRY_BUDGET
                            ));

                            if let Some(evidence) =
                                format_compile_failure_evidence(&decision, &packages, &err_preview)
                            {
                                session.local_evidence.push_str(&evidence);
                            }

                            return Ok(CortexState::Strategize);
                        }

                        CompileDecision::BudgetExhausted { total_iterations } => {
                            self.log(format!(
                                "\x1b[1;31m>> [COMPILER CRITIC] Compile-fail retry budget exhausted after {} iterations. Reverting mutations.\x1b[0m",
                                total_iterations
                            ));

                            let mut revert_all_succeeded = true;
                            for path in &mutation_paths {
                                if crate::podman::PodmanShield::execute_argv("git", &["checkout", "--", path])
                                .is_err()
                                {
                                    revert_all_succeeded = false;
                                }
                            }

                            session.local_evidence.truncate(evidence_checkpoint);

                            if let Some(evidence) =
                                format_compile_failure_evidence(&decision, &packages, &err_preview)
                            {
                                session.local_evidence.push_str(&evidence);
                            }
                            session.local_evidence.push('\n');

                            if revert_all_succeeded {
                                session.compile_remediation.reset();
                            } else {
                                session.local_evidence.push_str(
                                    "[COMPILE_REVERT_FAILED] One or more `git checkout` reverts did not succeed. Source may still be broken. The remediation episode remains open.\n",
                                );
                            }

                            if let Some(h_id) = session.best_hypothesis_id {
                                let obs_id = session.graph.add_node(
                                    NodeKind::Observation,
                                    if revert_all_succeeded {
                                        "[COMPILE_FAIL] Patch failed physical compiler verification. Reverted."
                                    } else {
                                        "[COMPILE_REVERT_FAILED] Patch failed; revert did not complete."
                                    },
                                    1.0,
                                    90,
                                );

                                session.graph.add_edge(
                                    obs_id,
                                    h_id,
                                    EdgeRelation::Contradicts,
                                    0.99,
                                );

                                session.graph.propagate_beliefs();
                            }

                            return Ok(CortexState::Strategize);
                        }

                        CompileDecision::Success => {
                            // Unreachable: this arm only runs when the compiler
                            // returned a non-zero exit status. Defensive fallback:
                            // treat as exhaustion, but require a successful revert
                            // before resetting the episode.
                            self.log(
                                "\x1b[1;31m>> [COMPILER CRITIC] Internal inconsistency: Success decision on failed compile. Treating as exhaustion.\x1b[0m"
                                    .to_string(),
                            );

                            let mut revert_all_succeeded = true;
                            for path in &mutation_paths {
                                if crate::podman::PodmanShield::execute_argv("git", &["checkout", "--", path])
                                .is_err()
                                {
                                    revert_all_succeeded = false;
                                }
                            }

                            if revert_all_succeeded {
                                session.compile_remediation.reset();
                            } else {
                                session.local_evidence.push_str(
                                    "[COMPILE_REVERT_FAILED] Internal inconsistency path: revert did not complete. Episode remains open.\n",
                                );
                            }

                            return Ok(CortexState::Strategize);
                        }
                    }
                }

                CompilerGateResult::ExecutionFault { error_message } => {
                    let message = format!("[COMPILER_FAULT] {}", error_message);

                    self.log(format!("\x1b[1;31m>> [COMPILER CRITIC] {}\x1b[0m", message));

                    session.local_evidence.truncate(evidence_checkpoint);

                    // A compiler-verification launch failure is still a failed
                    // compiler verification attempt. Keep it inside the same
                    // explicit remediation state machine so the retry budget
                    // cannot be bypassed merely because cargo failed to spawn.
                    let decision = session.compile_remediation.classify(false);

                    match &decision {
                        CompileDecision::RetainForRetry { iteration } => {
                            session.active_mutation_paths = mutation_paths.clone();

                            session.local_evidence.push_str(&format!(
                                "[COMPILER_FAULT_RETRY] iteration {}/{}\n",
                                iteration, COMPILE_RETRY_BUDGET
                            ));
                            session.local_evidence.push_str(&message);
                            session.local_evidence.push('\n');

                            return Ok(CortexState::Strategize);
                        }

                        CompileDecision::BudgetExhausted { total_iterations } => {
                            session.local_evidence.push_str(&format!(
                                "[COMPILER_FAULT_BUDGET_EXHAUSTED] {} failed compiler-verification attempts.\n",
                                total_iterations
                            ));
                            session.local_evidence.push_str(&message);
                            session.local_evidence.push('\n');

                            let mut revert_all_succeeded = true;

                            for path in &mutation_paths {
                                if crate::podman::PodmanShield::execute_argv("git", &["checkout", "--", path])
                                .is_err()
                                {
                                    revert_all_succeeded = false;
                                }
                            }

                            if revert_all_succeeded {
                                session.compile_remediation.reset();
                                session.active_mutation_paths.clear();
                            } else {
                                session.local_evidence.push_str(
                                    "[COMPILE_REVERT_FAILED] One or more `git checkout` reverts did not succeed. Source may still be broken. The remediation episode remains open.\n",
                                );
                            }

                            if let Some(h_id) = session.best_hypothesis_id {
                                let obs_id = session.graph.add_node(
                                    NodeKind::Observation,
                                    if revert_all_succeeded {
                                        "[COMPILER_FAULT] Compiler verification could not launch; mutation reverted."
                                    } else {
                                        "[COMPILER_FAULT] Compiler verification could not launch; revert failed."
                                    },
                                    1.0,
                                    90,
                                );

                                session.graph.add_edge(
                                    obs_id,
                                    h_id,
                                    EdgeRelation::Contradicts,
                                    0.99,
                                );

                                session.graph.propagate_beliefs();
                            }

                            return Ok(CortexState::Strategize);
                        }

                        CompileDecision::Success => {
                            // classify(false) cannot produce Success; retain a
                            // defensive branch so the compiler enforces
                            // exhaustive handling if the state machine changes.
                            session.local_evidence.push_str(&message);
                            session.local_evidence.push('\n');
                            return Ok(CortexState::Strategize);
                        }
                    }
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

    /// Prevent a single failed action from driving the active hypothesis
    /// below the selection threshold. If the hypothesis was selectable
    /// before this evaluation (`pre_eval_belief >= floor`), clamp its
    /// post-evaluation score back up to the floor.
    ///
    /// This preserves anti-loop behavior -- repeated failures still decay
    /// the hypothesis through subsequent turns -- while ensuring a single
    /// `PATH_NOT_FOUND`, `[ACTION FAILED]`, or strongly negative LLM
    /// evaluation does not remove the hypothesis from the candidate set
    /// on the same turn the failure occurred. `select_best_hypothesis`
    /// filters hypotheses with `belief_score < 0.15`; without this floor
    /// one failed action can exhaust a node before it can try a different
    /// approach.
    ///
    /// The floor is not applied when the hypothesis was already below the
    /// threshold before this evaluation: a hypothesis that is already
    /// dead should not be resurrected by a single turn's accounting.
    fn floor_hypothesis_belief_after_failure(
        session: &mut EpistemicSession,
        h_id: uuid::Uuid,
        pre_eval_belief: f32,
        floor: f32,
    ) {
        session.graph.restore_floor(h_id, pre_eval_belief, floor);
    }

    /// Apply a direct decay to the active hypothesis's belief.
    ///
    /// This is the chokepoint for direct belief subtraction
    /// (`belief - delta`) used by strategy-violation and anti-loop
    /// pivots.
    ///
    /// F4: no floor clamp is applied. The belief may cross SELECT_FLOOR
    /// and become unselectable, which is exactly what forced-pivot
    /// callers intend. Decay never resurrects: it only floors at 0.0.
    /// The evaluation-failure rescue path remains
    /// `floor_hypothesis_belief_after_failure`.
    fn decay_hypothesis_belief(session: &mut EpistemicSession, h_id: uuid::Uuid, delta: f32) {
        let _ = session.graph.decay(h_id, delta);
    }

    async fn handle_evaluate(
        &mut self,
        session: &mut EpistemicSession,
        mission: &str,
        evidence: String,
        _proposal: String,
        executed_action: String,
        belief: f32,
        is_halt_action: bool,
        mutation_path: Option<String>,
    ) -> Result<CortexState> {
        self.log(
            "\x1b[33m>> [CORTEX] Evaluating physical evidence against strategy...\x1b[0m"
                .to_string(),
        );
        session.budget.record_call();

        let h_id = session.best_hypothesis_id.unwrap_or_default();
        // The Cortex force-pins `halt` for read-only audits once coverage is complete,
        // but the graph hypothesis still reads `read_file:...`. Judge the halt against
        // what it was actually asked to do.
        let graph_h_content = session.graph.content_of(h_id);
        let h_content = if is_halt_action && session.contract.read_only {
            "halt: deliver the complete read-only audit findings in the halt reasoning".to_string()
        } else {
            graph_h_content
        };
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

                // If deterministic physical proof already succeeded, do not let an evaluator timeout discard progress
                if evidence.contains("[COMPILE_OK]") || evidence.contains("[PATCH_FUNCTION_OK]") {
                    let fallback_eval = crate::epistemic_engine::CriticResponse {
                        critique: "Deterministic physical compiler check passed; evaluator fallback applied.".to_string(),
                        confidence_modifier: 0.35,
                    };
                    session.record_evidence(&evidence, &fallback_eval, h_id);
                } else if ["[READ_CHUNK_OK]", "[READ_OK]", "[READ_FUNCTION_OK]", "[SEARCH_CODE_OK]"]
                    .iter()
                    .any(|m| evidence.contains(m))
                {
                    // An evaluator outage must not erase successful recon from the
                    // graph view the strategist and critic read, or they diverge
                    // from the coverage ledger (which reads local_evidence) and
                    // deadlock on "gaps" that were already read.
                    let neutral = crate::epistemic_engine::CriticResponse {
                        critique: "Evaluator unavailable; recon evidence recorded with neutral modifier.".to_string(),
                        confidence_modifier: 0.0,
                    };
                    session.record_evidence(&evidence, &neutral, h_id);
                }

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
                        Self::floor_hypothesis_belief_after_failure(session, h_id, belief, 0.15);
                        return Ok(CortexState::Strategize);
                    }
                }
                self.log(format!(
                    "\x1b[33m>> [EVALUATION]\x1b[0m {} (Modifier: {:.2})",
                    eval.critique, eval.confidence_modifier
                ));
                session.record_evidence(&evidence, &eval, h_id);

                Self::floor_hypothesis_belief_after_failure(session, h_id, belief, 0.15);

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

                // DETERMINISTIC HALT-CLAIM GATE (read-only audits): every backticked
                // symbol in the report must exist in evidence actually read.
                if is_halt_action && session.contract.read_only {
                    let corpus = crate::cortical::halt_claims::verification_corpus(
                        &session.local_evidence,
                        &evidence,
                    );
                    let bad = crate::cortical::halt_claims::unverified_symbols(&executed_action, &corpus);
                    if !bad.is_empty() {
                        self.log(format!(
                            "\x1b[1;31m>> [HALT CLAIM GATE] Unverified symbols in report: {:?}. Rejecting halt.\x1b[0m",
                            bad
                        ));
                        session.local_evidence = session.local_evidence.replace(&evidence, "");
                        session.local_evidence.push_str(&format!(
                            "\n[HALT_REJECTED_SYMBOLS] Report named symbols not found in any file content read: {}. Re-read the relevant ranges and cite only functions that appear in the evidence.\n",
                            bad.join(", ")
                        ));
                        return Ok(CortexState::Strategize);
                    }
                }

                // HALT REPORT GATE: the report must be findings, not process or intent.
                if is_halt_action && session.contract.read_only {
                    let corpus = crate::cortical::halt_claims::verification_corpus(
                        &session.local_evidence,
                        &evidence,
                    );
                    let report = crate::cortical::halt_claims::report_text(&executed_action);
                    let defects = crate::cortical::halt_claims::report_defects(&report, &corpus);
                    if !defects.is_empty() {
                        self.log(format!(
                            "\x1b[1;31m>> [HALT REPORT GATE] Rejecting halt: {:?}\x1b[0m",
                            defects
                        ));
                        session.local_evidence = session.local_evidence.replace(&evidence, "");
                        session.local_evidence.push_str(&format!(
                            "\n[HALT_REJECTED_REPORT] The halt `reasoning` field IS the deliverable. Put the full audit there: each large function by name with its line range, plus concrete findings. Defects: {}\n",
                            defects.join("; ")
                        ));
                        return Ok(CortexState::Strategize);
                    }
                }

                let current_belief = session
                    .graph
                    .nodes
                    .get(&h_id)
                    .map(|n| n.belief_score)
                    .unwrap_or(belief)
                    .max(belief + eval.confidence_modifier);

                let deterministic_contract_satisfied =
                    crate::completion_engine::SubgoalCompletionEngine::evaluate(
                        &session.contract,
                        &session.local_evidence,
                        if is_halt_action && session.contract.read_only {
                            1.0
                        } else {
                            current_belief
                        },
                    );

                let command_only_contract = session.contract.conditions.len() == 1
                    && matches!(
                        session.contract.conditions.first(),
                        Some(crate::completion_engine::CompletionCondition::CommandExecutionVerified { .. })
                    );

                let requires_semantic_verification =
                    crate::completion_engine::requires_semantic_verification(
                        &session.contract.objective,
                    );

                if is_halt_action
                    && deterministic_contract_satisfied
                    && command_only_contract
                    && !requires_semantic_verification
                {
                    self.log(
                        "\x1b[1;36m>> [CORTEX TERMINAL] Deterministic command contract satisfied by physical CMD_OK evidence. No additional model verification required. Node complete.\x1b[0m"
                            .to_string(),
                    );

                    return Ok(CortexState::Complete {
                        belief: current_belief.max(0.95),
                        evidence: session.collect_evidence(h_id),
                    });
                }

                if is_halt_action
                    && deterministic_contract_satisfied
                    && (eval.confidence_modifier >= 0.0 || session.contract.read_only)
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
                    let unmet = crate::outcome_verifier::OutcomeVerifier::unmet_conditions(
                        &session.local_evidence,
                        current_belief,
                        &session.contract.conditions,
                    );
                    self.log(format!(
                        "\x1b[1;33m>> [CORTEX] HALT rejected: deterministic completion contract is not satisfied. Unmet: {:?}\x1b[0m",
                        unmet
                    ));
                    session.local_evidence.push_str(&format!(
                        "\n[HALT_REJECTED] Deterministic completion contract not satisfied. Unmet: {:?}\n",
                        unmet
                    ));
                    return Ok(CortexState::Strategize);
                }

                if eval.confidence_modifier < 0.0 {
                    if let Some(path) = &mutation_path {
                        // argv form: a path containing shell metacharacters
                        // cannot be interpreted as a second command.
                        let revert_ok = crate::podman::PodmanShield::execute_argv(
                            "git",
                            &["checkout", "--", path],
                        )
                        .is_ok();

                        if revert_ok {
                            // Only on a confirmed successful revert may the
                            // evidence and the active mutation-path entry be
                            // discarded. A failed revert leaves the source
                            // mutated on disk; removing the evidence would
                            // erase the only record that a mutation occurred.
                            session.active_mutation_paths.retain(|p| p != path);
                            session.local_evidence = session.local_evidence.replace(&evidence, "");
                        } else {
                            session.local_evidence.push_str(
                                "[NEGATIVE_CONFIDENCE_REVERT_FAILED] Mutation revert did not succeed. Evidence preserved and active mutation path retained.\n",
                            );
                        }
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

/// Classification of a single `PatchFunction` action against the on-disk
/// source.
///
/// Pure: no logging, no session mutation, no file writes.
///
/// This is the canonical definition of what "mutation vs idempotent" means
/// for a `PatchFunction`. The inline classification block inside the Critic
/// path performs the same computation; the intent is that any future refactor
/// routes through this function so the semantics live in one place.
///
/// Regression context: the original defect was that `[PATCH_IDEMPOTENT_OK]`
/// was being pushed into `session.local_evidence` at CRITIC time (pre-
/// execution), which caused the top-of-loop terminal gate to fire on the
/// next turn -- short-circuiting Execute, preventing the executor from
/// emitting its own copy of the marker, and preventing the compiler critic
/// from satisfying a mandatory `[CMD_OK]` criterion. The fix removed that
/// premature push. The tests below lock the classification semantics so a
/// future refactor cannot silently reintroduce a similar misrouting.
#[derive(Debug, PartialEq, Eq)]
enum PatchClass {
    Mutation { path: String, function_name: String },
    Idempotent { path: String, function_name: String },
    NotPatch,
}

async fn classify_patch_action(action: &AgentAction) -> anyhow::Result<PatchClass> {
    let AgentAction::PatchFunction { _thought_process: None, path, function_name, new_body, .. } = action else {
        return Ok(PatchClass::NotPatch);
    };

    let original = tokio::fs::read_to_string(path).await?;
    let patched = nexus_ast_tool::mutation::ReplaceFunctionBody::apply_for_path(
        std::path::Path::new(path),
        &original,
        function_name,
        new_body,
    )?;

    Ok(if patched != original {
        PatchClass::Mutation { path: path.clone(), function_name: function_name.clone() }
    } else {
        PatchClass::Idempotent { path: path.clone(), function_name: function_name.clone() }
    })
}

/// Deterministic conformance checks for a written Python file. Requirements are
/// derived from the objective text:
///   * `name()`                     -> file must define `def name(`
///   * snake_case identifiers       -> must appear in the file
///   * `import ONLY a, b and c`     -> only those top-level modules may be imported
///   * `under|at most N lines`      -> line cap (all lines counted)
/// Returns human-readable defects; empty means conformant.
fn python_conformance_defects(objective: &str, path: &str, content: &str) -> Vec<String> {
    let mut defects: Vec<String> = Vec::new();
    let obj_lower = objective.to_ascii_lowercase();

    // 1. functions named as `name()`
    let mut required_fns: Vec<String> = Vec::new();
    let mut from = 0usize;
    while let Some(off) = objective[from..].find("()") {
        let end = from + off;
        let ident_rev: String = objective[..end]
            .chars()
            .rev()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
            .collect();
        let name: String = ident_rev.chars().rev().collect();
        let starts_ok = name.chars().next().map(|c| !c.is_ascii_digit()).unwrap_or(false);
        let start = end - name.len();
        let before = &objective[..start];
        let is_attr_call = before.ends_with('.');
        let defining = before.split_whitespace().rev().take(3).any(|w| {
            let w = w
                .trim_matches(|c: char| !c.is_ascii_alphanumeric())
                .to_ascii_lowercase();
            w == "def" || w == "function" || w == "method" || w.starts_with("defin")
        });
        if starts_ok && !is_attr_call && defining && !required_fns.contains(&name) {
            required_fns.push(name);
        }
        from = end + 2;
    }
    for name in &required_fns {
        let def_prefix = format!("def {}(", name);
        let async_prefix = format!("async def {}(", name);
        let defined = content.lines().any(|l| {
            let t = l.trim_start();
            t.starts_with(&def_prefix) || t.starts_with(&async_prefix)
        });
        if !defined {
            defects.push(format!("missing `def {}(...)` required by the objective", name));
        }
    }

    // 2. snake_case identifiers named in the objective must appear in the file
    let path_stem = std::path::Path::new(path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_string();
    let mut idents: Vec<String> = Vec::new();
    for tok in objective.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_')) {
        let tok = tok.trim_matches('_');
        if tok.len() < 3 || !tok.contains('_') {
            continue;
        }
        // marker tokens such as WRITE_OK
        if tok.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_') {
            continue;
        }
        if tok == path_stem || required_fns.iter().any(|f| f == tok) {
            continue;
        }
        if !idents.iter().any(|i| i == tok) {
            idents.push(tok.to_string());
        }
    }
    for tok in &idents {
        if !content.contains(tok.as_str()) {
            defects.push(format!("missing identifier `{}` named in the objective", tok));
        }
    }

    // 3. `import ONLY a, b and c` allowlist
    if let Some(pos) = obj_lower.find("import only") {
        let after = &objective[pos + "import only".len()..];
        let sentence = after.split(". ").next().unwrap_or(after).trim().trim_end_matches('.');
        let mut allowed: Vec<String> = Vec::new();
        'items: for item in sentence.split(',') {
            let words: Vec<&str> = item.split_whitespace().collect();
            if words.is_empty() {
                continue;
            }
            let mut i = if words[0].eq_ignore_ascii_case("and") { 1 } else { 0 };
            let mut batch: Vec<String> = Vec::new();
            let mut conforming = true;
            while i < words.len() {
                let w = words[i];
                if !w.is_empty()
                    && w.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.')
                {
                    batch.push(w.split('.').next().unwrap_or(w).to_string());
                    i += 1;
                    if i < words.len() {
                        if words[i].eq_ignore_ascii_case("and") {
                            i += 1;
                        } else {
                            conforming = false;
                            break;
                        }
                    }
                } else {
                    conforming = false;
                    break;
                }
            }
            if !conforming || batch.is_empty() {
                break 'items;
            }
            allowed.extend(batch);
        }
        if !allowed.is_empty() {
            let mut bad: Vec<String> = Vec::new();
            for line in content.lines() {
                let t = line.trim_start();
                let mut modules: Vec<String> = Vec::new();
                if let Some(rest) = t.strip_prefix("import ") {
                    for part in rest.split(';').next().unwrap_or("").split(',') {
                        let m = part.trim().split_whitespace().next().unwrap_or("");
                        modules.push(m.split('.').next().unwrap_or("").to_string());
                    }
                } else if let Some(rest) = t.strip_prefix("from ") {
                    let m = rest.split_whitespace().next().unwrap_or("");
                    if !m.starts_with('.') {
                        modules.push(m.split('.').next().unwrap_or("").to_string());
                    }
                }
                for m in modules {
                    if !m.is_empty()
                        && m != "__future__"
                        && !allowed.contains(&m)
                        && !bad.contains(&m)
                    {
                        bad.push(m);
                    }
                }
            }
            if !bad.is_empty() {
                defects.push(format!("imports not allowed by the objective: {}", bad.join(", ")));
            }
        }
    }

    // 4. line cap
    let words: Vec<String> = objective
        .split_whitespace()
        .map(|w| w.trim_matches(|c: char| !c.is_ascii_alphanumeric()).to_ascii_lowercase())
        .collect();
    let mut max_lines: Option<usize> = None;
    for (idx, w) in words.iter().enumerate() {
        if w != "lines" || idx == 0 {
            continue;
        }
        let Ok(n) = words[idx - 1].parse::<usize>() else { continue };
        let prev = if idx >= 2 { words[idx - 2].as_str() } else { "" };
        let prev2 = if idx >= 3 { words[idx - 3].as_str() } else { "" };
        let limit = if matches!(
            (prev2, prev),
            (_, "under") | (_, "below") | ("fewer", "than") | ("less", "than")
        ) {
            Some(n.saturating_sub(1))
        } else if matches!((prev2, prev), ("at", "most") | ("up", "to"))
            || (prev2 == "more" && prev == "than" && idx >= 4 && words[idx - 4] == "no")
        {
            Some(n)
        } else {
            None
        };
        if let Some(l) = limit {
            max_lines = Some(max_lines.map_or(l, |m| m.min(l)));
        }
    }
    if let Some(limit) = max_lines {
        let n_lines = content.lines().count();
        if n_lines > limit {
            defects.push(format!("{} lines exceeds the objective's limit of {}", n_lines, limit));
        }
    }

    defects
}

/// `ast.parse` the file with python3 (no bytecode side effects). Returns None when the file
/// parses, or when python3 is unavailable (the check is skipped, never blocking).
async fn python_syntax_error(path: &str) -> Option<String> {
    let out = tokio::process::Command::new("python3")
        .args([
            "-c",
            "import ast,sys; ast.parse(open(sys.argv[1], encoding='utf-8').read(), sys.argv[1])",
            path,
        ])
        .output()
        .await
        .ok()?;
    if out.status.success() {
        None
    } else {
        Some(
            String::from_utf8_lossy(&out.stderr)
                .lines()
                .last()
                .unwrap_or("syntax error")
                .trim()
                .to_string(),
        )
    }
}

#[cfg(test)]
mod conformance_tests {
    use super::python_conformance_defects;

    const OBJ: &str = "Write a Python script exactly named 'sys_monitor.py' in the current directory. The script must import ONLY json, sys, and psutil, define a function get_metrics() returning a dict with cpu_percent and memory_percent, print it as JSON, and remain under 20 lines.";

    #[test]
    fn rejects_import_only_stub() {
        let d = python_conformance_defects(OBJ, "sys_monitor.py", "import json, sys, psutil");
        assert!(d.iter().any(|x| x.contains("get_metrics")), "{d:?}");
        assert!(d.iter().any(|x| x.contains("cpu_percent")), "{d:?}");
        assert!(d.iter().any(|x| x.contains("memory_percent")), "{d:?}");
    }

    #[test]
    fn accepts_conforming_script() {
        let src = "import json\nimport sys\nimport psutil\n\ndef get_metrics():\n    return {\"cpu_percent\": psutil.cpu_percent(), \"memory_percent\": psutil.virtual_memory().percent}\n\nif __name__ == \"__main__\":\n    json.dump(get_metrics(), sys.stdout)\n";
        let d = python_conformance_defects(OBJ, "sys_monitor.py", src);
        assert!(d.is_empty(), "{d:?}");
    }

    #[test]
    fn rejects_forbidden_import_and_long_file() {
        let src = format!("import os\n{}", "x = 1\n".repeat(24));
        let d = python_conformance_defects(OBJ, "sys_monitor.py", &src);
        assert!(d.iter().any(|x| x.contains("imports not allowed")), "{d:?}");
        assert!(d.iter().any(|x| x.contains("exceeds")), "{d:?}");
    }
}

/// Normalizes a strategy string for duplicate detection: quotes dropped, whitespace
/// collapsed, ASCII-lowercased. `execute_command "x"` and `execute_command 'x'` compare equal.
fn normalize_strategy(s: &str) -> String {
    s.chars()
        .filter(|c| !matches!(c, '\'' | '"' | '`'))
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase()
}

#[cfg(test)]
mod conformance_regression_tests {
    use super::python_conformance_defects;

    #[test]
    fn attribute_calls_in_objective_are_not_required_definitions() {
        let obj = "Write a file named \"sys_monitor_v2.py\" containing EXACTLY this code:\nimport json\nimport os\ndef get_metrics():\n    return {\"load_avg\": os.getloadavg()}\nprint(json.dumps(get_metrics()))";
        let src = "import json\nimport os\ndef get_metrics():\n    return {\"load_avg\": os.getloadavg()}\nprint(json.dumps(get_metrics()))\n";
        let d = python_conformance_defects(obj, "sys_monitor_v2.py", src);
        assert!(d.is_empty(), "{d:?}");
    }

    #[test]
    fn stub_is_still_rejected_for_the_exact_code_objective() {
        let obj = "Write a file named \"sys_monitor_v2.py\" containing EXACTLY this code:\nimport json\nimport os\ndef get_metrics():\n    return {\"load_avg\": os.getloadavg()}\nprint(json.dumps(get_metrics()))";
        let d = python_conformance_defects(obj, "sys_monitor_v2.py", "import json import os def get_metrics(): return {");
        assert!(d.iter().any(|x| x.contains("get_metrics")), "{d:?}");
    }
}

/// Best-effort action name for proposals that use `action` / `tool` keys, ReAct nesting, or
/// {"halt": ...} instead of `"type"`, so a parse fault can still name the real schema fields.
fn fallback_action_type(raw: &str) -> Option<String> {
    for key in ["\"action\"", "\"tool\""] {
        if let Some(pos) = raw.find(key) {
            let after = raw[pos + key.len()..].trim_start();
            if let Some(rest) = after.strip_prefix(':') {
                if let Some(rest) = rest.trim_start().strip_prefix('"') {
                    if let Some(end) = rest.find('"') {
                        return Some(rest[..end].to_string());
                    }
                }
            }
        }
    }
    if raw.contains("\"halt\"") {
        return Some("halt".to_string());
    }
    // bare function-call style text such as `read_file_chunk path 1244 250`
    for name in [
        "read_file_chunk",
        "read_function",
        "read_file",
        "search_code",
        "search_memory",
        "write_file",
        "diff_patch",
        "patch_function",
        "replace_lines",
        "execute_command",
    ] {
        if raw.contains(name) {
            return Some(name.to_string());
        }
    }
    None
}

#[cfg(test)]
#[path = "cortical/loop_tests.rs"]
mod tests;
