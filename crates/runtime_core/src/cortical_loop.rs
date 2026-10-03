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
use nexus_action_identity_algorithms::{action_key as calculate_action_key, ActionIdentityInput};
use nexus_reconnaissance_algorithms::ReconnaissancePlanner;

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
            if crate::podman::PodmanShield::execute(&format!("git checkout -- {}", path)).is_err() {
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
        let world_context = Self::compress_context(&context_chunks.join("\n"), 2500);

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

        let is_mutation_task = session.contract.is_mutation_task
            || mission.to_lowercase().contains("synthesize_report")
            || mission.to_lowercase().contains(".md");
        let is_write_task = is_mutation_task
            || session.contract.objective.to_lowercase().contains("write")
            || session.contract.objective.to_lowercase().contains("export")
            || session.contract.objective.to_lowercase().contains(".md")
            || session.contract.objective.to_lowercase().contains(".json")
            || mission.to_lowercase().contains("synthesize_report");

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

    /// Canonical anti-loop key for an action.
    ///
    /// Two actions with the same key are treated as duplicates. For
    /// `PatchFunction`, the key includes a hash of `new_body` so a
    /// corrective retry on the same symbol with a different body is not
    /// filtered as a repeat of an already-executed attempt. `reasoning` is
    /// deliberately excluded: it is model-produced prose and must not
    /// affect the identity of the action.
    /// Runtime adapter for the independent action-identity algorithm.
    ///
    /// AgentAction remains owned by runtime_core. The extracted algorithm
    /// receives only the fields that define canonical identity. Variants that
    /// historically used the Debug fallback preserve that exact behavior.
    fn action_key(action: &AgentAction) -> String {
        let input = match action {
            AgentAction::ReadFile { path, .. } => Some(ActionIdentityInput::ReadFile { path }),
            AgentAction::ReadFunction { path, function_name, .. } => {
                Some(ActionIdentityInput::ReadFunction { path, function_name })
            }
            AgentAction::ReadFileChunk { path, start_line, .. } => {
                Some(ActionIdentityInput::ReadFileChunk { path, start_line: *start_line })
            }
            AgentAction::SearchCode { path, query, .. } => {
                Some(ActionIdentityInput::SearchCode { path, query })
            }
            AgentAction::SearchMemory { query, .. } => {
                Some(ActionIdentityInput::SearchMemory { query })
            }
            AgentAction::WriteFile { path, content, .. } => {
                Some(ActionIdentityInput::WriteFile { path, content })
            }
            AgentAction::DiffPatch { path, .. } => Some(ActionIdentityInput::DiffPatch { path }),
            AgentAction::PatchFunction { path, function_name, new_body, .. } => {
                Some(ActionIdentityInput::PatchFunction { path, function_name, new_body })
            }
            AgentAction::SynthesizeReport { output_path, .. } => {
                Some(ActionIdentityInput::SynthesizeReport { output_path })
            }
            AgentAction::ExecuteCommand { command, .. } => {
                Some(ActionIdentityInput::ExecuteCommand { command })
            }
            _ => None,
        };

        // A halt carries its answer in `reasoning`. Key on a hash of the
        // text so a revised halt after a rejection is not a "duplicate".
        if let AgentAction::Halt { .. } = action {
            use std::hash::{Hash, Hasher};
            let mut h = std::collections::hash_map::DefaultHasher::new();
            format!("{:?}", action).hash(&mut h);
            return format!("halt:{:016x}", h.finish());
        }

        match input {
            Some(input) => calculate_action_key(input),
            None => format!("{:?}", action),
        }
    }

    /// Deterministically enforce the Cortex strategy/action boundary.
    ///
    /// The Cortex selects the active strategy. The LLM Strategist may
    /// operationalize that strategy, but may not silently substitute a
    /// different native action.
    fn action_matches_strategy(strategy: &str, action: &AgentAction) -> bool {
        let strategy = strategy.trim();

        // A Halt proposal's legitimacy is decided downstream by the real
        // completion check (deterministic_contract_satisfied / handle_evaluate),
        // which verifies actual evidence against the contract. Gating Halt here
        // on matching the active strategy's literal token blocks the model from
        // ever exiting once a strategy's evidence goal is met but the strategy
        // string doesn't itself say "halt" -- previously carved out only for
        // write_file: (see below); generalized after two more independent
        // stalls hit the identical bug under read_file: and search_code:
        // strategies (router.rs audit: 0/1 nodes, belief decayed 0.64->0.04;
        // reflection_test.rs mission: 0/3 nodes).
        if matches!(action, AgentAction::Halt { .. }) {
            return true;
        }

        // HALT-PREFIX RECOVERY: a strategy that begins with "halt:" must not
        // permanently block reconnaissance actions. Without this, a HaltDelivered
        // analysis node that later requires source inspection can never escape
        // the initial halt-prefixed hypothesis.
        if strategy.starts_with("halt:") {
            return matches!(
                action,
                AgentAction::SearchCode { .. }
                    | AgentAction::ReadFile { .. }
                    | AgentAction::ReadFileChunk { .. }
                    | AgentAction::ReadFunction { .. }
                    | AgentAction::SearchMemory { .. }
            );
        }

        if let Some(expected_path) = strategy.strip_prefix("read_file:") {
            // A `read_file:` strategy targets a specific file, not
            // literally the single-shot ReadFile action. When that file
            // exceeds the single-read size, the tool's own truncation
            // notice instructs the Strategist to continue via
            // `read_file_chunk` on the same path -- that is a legitimate
            // operationalization of "inspect this file", not a strategy
            // substitution. Confirmed root cause of a permanent
            // STRATEGY VIOLATION loop (33 turns burned, belief decayed to
            // 0.00, 0/12 DAG nodes) on any file too large for one read:
            // every compliant ReadFileChunk continuation was rejected as
            // incompatible, and the only "compatible" move (repeating the
            // already-truncated ReadFile) tripped anti-loop dedup instead.
            let expected_path = expected_path.trim();
            return matches!(
                action,
                AgentAction::ReadFile { path, .. } if path.trim() == expected_path
            ) || matches!(
                action,
                AgentAction::ReadFileChunk { path, .. } if path.trim() == expected_path
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
            let first = parts.next().unwrap_or("").trim();
            let second = parts.next().map(|s| s.trim());

            return match second {
                Some(expected_query) => {
                    // Well-formed "search_code:<path>:<query>" -- the shape
                    // every deterministic branch (e.g. the SEARCH_CODE_OK:
                    // compound-hypothesis fix) always emits.
                    //
                    // The path component is a DIRECTORY SCOPE, not a
                    // single-file pin. The model may legitimately refine
                    // it to a specific file within that scope; the query
                    // is authoritative and must match exactly. Requiring
                    // exact path equality rejected every refinement as a
                    // STRATEGY VIOLATION -- confirmed root cause of the
                    // audit mission's turn-1 abort, where the model
                    // proposed `.../cortical_loop.rs` against a strategy
                    // that scoped `.../src`.
                    //
                    // `..` is disallowed to keep the relaxation from
                    // being usable for path traversal; the query still
                    // has to match verbatim.
                    let expected_path = first;
                    let expected_path_norm = expected_path.trim_end_matches('/');
                    let prefix = format!("{}/", expected_path_norm);
                    // Query is a REFINEMENT, not a byte-exact contract.
                    //
                    // The DAG verifier (orchestrator_v2/verifier.rs,
                    // criterion "SEARCH_CODE_OK:<term>") checks only that
                    // the evidence block contains <term> as a substring
                    // anywhere in the text -- including matched code
                    // lines. It does not require the query string recorded
                    // in the evidence to match the criterion verbatim.
                    // The matcher was stricter than the verifier and
                    // rejected legitimate paraphrases. Confirmed root
                    // cause of the SEARCH_CODE_OK:fn test_ abort.
                    //
                    // Accept either direction of substring containment,
                    // guarded by a 4-character minimum so a degenerate
                    // query ("t", "_", "fn") cannot match everything.
                    let actual_q = if let AgentAction::SearchCode { query, .. } = action {
                        query.trim().to_lowercase()
                    } else {
                        String::new()
                    };
                    let expected_q = expected_query.trim().to_lowercase();
                    let min_len = actual_q.len().min(expected_q.len());
                    let query_matches = actual_q == expected_q
                        || (min_len >= 4
                            && (actual_q.contains(&expected_q) || expected_q.contains(&actual_q)));

                    // Path check shared by the search itself and any
                    // follow-up read under the same directory scope.
                    let path_ok = |p: &str| -> bool {
                        let pt = p.trim();
                        // The Strategist may propose an absolute path (e.g.
                        // "/root/AthenaFusionX/vendor/openfang/Cargo.toml")
                        // even when the strategy's scope was recorded
                        // relative to the workspace root (e.g.
                        // "vendor/openfang"). The original check only
                        // matched literal equality or a relative prefix,
                        // so any absolute-path proposal against a
                        // relative-scoped search_code strategy was
                        // rejected as a STRATEGY VIOLATION regardless of
                        // whether it was the obviously correct file.
                        // Confirmed root cause of the OpenFang audit
                        // mission's node_audit_openfang_workspace stall
                        // (60 turns burned, best belief 0.15, 5/10 DAG
                        // nodes): every read_file proposal targeting the
                        // real vendor/openfang/Cargo.toml used an absolute
                        // path and was rejected on every attempt.
                        pt == expected_path
                            || (pt.starts_with(&prefix) && !pt.contains(".."))
                            || pt.ends_with(&format!("/{}", expected_path_norm))
                            || (pt.contains(&format!("/{}/", expected_path_norm))
                                && !pt.contains(".."))
                    };

                    // Accept the search action itself, AND accept a
                    // read_file / read_file_chunk that inspects a file
                    // under the searched directory. After locating a
                    // symbol the model legitimately wants to read the
                    // region it found; rejecting that as a strategy
                    // substitution burned 4 turns of belief decay per
                    // node. The verifier still requires
                    // [SEARCH_CODE_OK] in evidence, so a read alone
                    // cannot satisfy a SEARCH_CODE_OK criterion.
                    match action {
                        AgentAction::SearchCode { path, .. } => path_ok(path) && query_matches,
                        AgentAction::ReadFileChunk { path, .. } => path_ok(path),
                        AgentAction::ReadFile { path, .. } => path_ok(path),
                        // Accept an ExecuteCommand that is a read-only
                        // existence/inspection check referencing the search
                        // target -- e.g. `ls -la <path>` or `test -f <path>`.
                        // search_code cannot resolve a target outside a
                        // source tree (e.g. /tmp), and the model correctly
                        // diagnosed that mismatch; rejecting the pivot as a
                        // strategy violation left it with no valid way to
                        // confirm a non-source path's existence. Confirmed
                        // root cause of the reflection_test.rs mission's
                        // task_1 stall (0/3 nodes, belief decayed
                        // 0.42->0.02).
                        AgentAction::ExecuteCommand { command, .. } => {
                            let cmd = command.trim();
                            let is_inspection_cmd = ["ls ", "ls -", "stat ", "test ", "file "]
                                .iter()
                                .any(|verb| cmd.starts_with(verb));
                            // The hypothesis generator sometimes emits
                            // "search_code:<generic_scope>:<real_target>"
                            // (e.g. defaulting path to a generic src scope
                            // and putting the actual mission target in the
                            // query slot) instead of the well-formed
                            // path:query shape. Check the command against
                            // either slot so a correct inspection command
                            // targeting the real path (carried here in
                            // expected_query) is not rejected merely
                            // because the malformed hypothesis put it in
                            // the wrong position. Confirmed root cause of
                            // the reflection_test.rs mission's task_1
                            // stall persisting after the first
                            // ExecuteCommand fix: expected_path resolved
                            // to a generic scope while the real target sat
                            // in expected_query.
                            let expected_query_lower = expected_query.trim().to_lowercase();
                            is_inspection_cmd
                                && (cmd.contains(expected_path)
                                    || cmd.to_lowercase().contains(&expected_query_lower))
                        }
                        _ => false,
                    }
                }
                None => {
                    // Freeform "search_code:<query>" with no path component --
                    // produced by the general 3-strategy LLM hypothesis
                    // generator, which is not constrained to the path:query
                    // shape the deterministic branches always use. Pinning an
                    // empty/bogus path here is unsatisfiable by construction,
                    // same failure family as the already-fixed search_memory:
                    // exact-match bug. Confirmed root cause of a permanent
                    // STRATEGY VIOLATION stall (60 turns, best belief 0.50,
                    // 2/12 DAG nodes): STRATEGY_VIOLATION deliberately never
                    // decays belief (operational failure != epistemic
                    // contradiction), so a structurally malformed strategy
                    // string had no way to ever pivot to a sibling hypothesis.
                    // Require only that the query matches; path is free-form.
                    let expected_query = first;
                    matches!(
                        action,
                        AgentAction::SearchCode { query, .. } if query.trim() == expected_query
                    )
                }
            };
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
            let _expected_function = parts.next().unwrap_or("").trim();

            return matches!(
                action,
                AgentAction::PatchFunction {
                    path,
                    function_name,
                    ..
                } if path.trim() == expected_path
                    && !function_name.trim().is_empty()
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

    /// Returns true if executing `action` would produce evidence that
    /// satisfies a currently-unmet condition in the session contract.
    ///
    /// Multi-criteria safety net: when the planner packs several
    /// criteria into one node, the cortex's single-strategy model
    /// cannot satisfy them sequentially -- after the first criterion
    /// is met, the active strategy never transitions to the next, so
    /// every action for the remaining criteria is rejected. Accepting
    /// any action that advances an unmet criterion lets the model
    /// progress through the criteria in any order.
    ///
    /// The DAG verifier independently requires each criterion's marker
    /// in evidence, so this relaxation cannot authorize completion
    /// without the required physical evidence.
    fn action_advances_unmet_contract_condition(
        action: &AgentAction,
        session: &EpistemicSession,
        mission: &str,
    ) -> bool {
        use crate::completion_engine::CompletionCondition as CC;

        for cond in &session.contract.conditions {
            if Self::condition_is_met(cond, &session.local_evidence) {
                continue;
            }
            let advances = match cond {
                CC::ReadMarkerAcquired => match Self::extract_named_target(mission, "READ_OK:") {
                    Some(target) => {
                        matches!(
                            action,
                            AgentAction::ReadFile { path, .. } if path.trim() == target
                        ) || matches!(
                            action,
                            AgentAction::ReadFileChunk { path, .. } if path.trim() == target
                        )
                    }
                    None => matches!(
                        action,
                        AgentAction::ReadFile { .. } | AgentAction::ReadFileChunk { .. }
                    ),
                },
                CC::ReadPathAcquired { path: target } => {
                    matches!(
                        action,
                        AgentAction::ReadFile { path, .. } if path.trim() == target
                    ) || matches!(
                        action,
                        AgentAction::ReadFileChunk { path, .. } if path.trim() == target
                    )
                }
                CC::SearchMarkerAcquired => matches!(action, AgentAction::SearchCode { .. }),
                CC::SearchTermAcquired { term } => {
                    let expected = term.trim().to_lowercase();
                    matches!(
                        action,
                        AgentAction::SearchCode { query, .. }
                            if query.trim().to_lowercase().contains(&expected)
                                || expected.contains(&query.trim().to_lowercase())
                    )
                }
                CC::SearchMemoryMarkerAcquired => {
                    matches!(action, AgentAction::SearchMemory { .. })
                }
                CC::WriteMarkerAcquired => match Self::extract_named_target(mission, "WRITE_OK:") {
                    Some(target) => {
                        matches!(
                            action,
                            AgentAction::WriteFile { path, .. } if path.trim() == target
                        ) || matches!(
                            action,
                            AgentAction::SynthesizeReport { output_path, .. }
                                if output_path.trim() == target
                        )
                    }
                    None => matches!(
                        action,
                        AgentAction::WriteFile { .. } | AgentAction::SynthesizeReport { .. }
                    ),
                },
                CC::WritePathAcquired { path: target } => {
                    matches!(
                        action,
                        AgentAction::WriteFile { path, .. } if path.trim() == target
                    ) || matches!(
                        action,
                        AgentAction::SynthesizeReport { output_path, .. }
                            if output_path.trim() == target
                    )
                }
                CC::PatchMarkerAcquired => {
                    match Self::extract_named_target(mission, "PATCH_FUNCTION_OK:") {
                        Some(target) => {
                            let mut parts = target.splitn(2, ':');
                            let expected_path = parts.next().unwrap_or("").trim();
                            let expected_fn = parts.next().unwrap_or("").trim();
                            if expected_path.is_empty() || expected_fn.is_empty() {
                                matches!(
                                    action,
                                    AgentAction::PatchFunction { .. }
                                        | AgentAction::DiffPatch { .. }
                                        | AgentAction::ReplaceLines { .. }
                                )
                            } else {
                                matches!(
                                    action,
                                    AgentAction::PatchFunction { path, function_name, .. }
                                        if path.trim() == expected_path
                                            && function_name.trim() == expected_fn
                                )
                            }
                        }
                        None => matches!(
                            action,
                            AgentAction::PatchFunction { .. }
                                | AgentAction::DiffPatch { .. }
                                | AgentAction::ReplaceLines { .. }
                        ),
                    }
                }
                CC::CommandMarkerAcquired => {
                    matches!(action, AgentAction::ExecuteCommand { .. })
                }
                CC::CommandExecutionVerified { .. } => {
                    matches!(action, AgentAction::ExecuteCommand { .. })
                }
                CC::ReadFunctionMarkerAcquired { .. } => {
                    match Self::extract_named_target(mission, "READ_FUNCTION_OK:") {
                        Some(target) => {
                            let mut parts = target.splitn(2, ':');
                            let expected_path = parts.next().unwrap_or("").trim();
                            let expected_fn = parts.next().unwrap_or("").trim();
                            if expected_path.is_empty() || expected_fn.is_empty() {
                                matches!(action, AgentAction::ReadFunction { .. })
                            } else {
                                matches!(
                                    action,
                                    AgentAction::ReadFunction { path, function_name, .. }
                                        if path.trim() == expected_path
                                            && function_name.trim() == expected_fn
                                )
                            }
                        }
                        None => matches!(action, AgentAction::ReadFunction { .. }),
                    }
                }
                CC::SourceCodeEvidenceRequired => matches!(
                    action,
                    AgentAction::ReadFile { .. }
                        | AgentAction::ReadFileChunk { .. }
                        | AgentAction::SearchCode { .. }
                ),
                CC::HaltDelivered => matches!(action, AgentAction::Halt { .. }),
                CC::LineageEvidenceAcquired | CC::MinBeliefScore(_) => false,
            };
            if advances {
                return true;
            }
        }
        false
    }

    /// Probes the outcome verifier against a single condition to
    /// determine whether it is already satisfied by the current
    /// evidence. Belief is passed as 1.0 so a MinBeliefScore condition
    /// would pass; MinBeliefScore is excluded from the match above
    /// because no action advances it directly.
    fn condition_is_met(
        cond: &crate::completion_engine::CompletionCondition,
        evidence: &str,
    ) -> bool {
        crate::outcome_verifier::OutcomeVerifier::mission_completion_authorized(
            evidence,
            1.0,
            std::slice::from_ref(cond),
        )
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
        unwritten_targets: &[&str],
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
        is_mutation_task: bool,
        is_write_task: bool,
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
                        let already = session.graph.find_by_content(&h).is_some();
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

        let critic_veto_present = session.local_evidence.contains("[CRITIC_VETO]");
        let patch_function_strategy = active_strategy.starts_with("patch_function:");

        self.log(format!(
            "[CORTEX RECOVERY CHECK] strategy=`{}` patch_function={} critic_veto={} write_phase_ready={} evidence_bytes={}",
            active_strategy,
            patch_function_strategy,
            critic_veto_present,
            write_phase_ready,
            session.local_evidence.len()
        )

);

        // --- AST-UNADDRESSABLE RECOVERY (Tree-sitter body missing) ---
        // When PatchFunction fails because the symbol has no Tree-sitter body,
        // demote the dead patch_function hypothesis and pivot to read_file of
        // the exact path named in the strategy. Do not guess a fallback path.
        // Do not synthesize replace_lines until READ_OK evidence exists.
        let tree_sitter_unaddressable = session.local_evidence.contains("has no Tree-sitter body")
            || session.local_evidence.contains("has no Tree-sitter body in Rust");

        if patch_function_strategy && tree_sitter_unaddressable {
            if let Some(h_id) = session.best_hypothesis_id {
                if session.graph.content_of(h_id).starts_with("patch_function:") {
                    session.graph.demote_hypothesis(h_id, 0.05);
                }
            }

            let path = active_strategy
                .strip_prefix("patch_function:")
                .and_then(|rest| rest.split(':').next())
                .map(str::trim)
                .filter(|p| !p.is_empty());

            if let Some(path) = path {
                let pivot_marker = format!("[AST_UNADDRESSABLE_PIVOTED]:{}", path);
                let already_pivoted = session.local_evidence.contains(&pivot_marker);

                let recon = format!("read_file:{}", path);
                let hid = session
                    .graph
                    .nodes
                    .iter()
                    .find(|(_, n)| n.content == recon)
                    .map(|(id, _)| *id)
                    .unwrap_or_else(|| {
                        session.graph.add_node(NodeKind::Hypothesis, &recon, 0.58, 100)
                    });

                // Always keep strategy/hypothesis coherent; log/mark only once.
                session.best_hypothesis_id = Some(hid);
                active_strategy = recon;

                if !already_pivoted {
                    session.local_evidence.push_str("\n");
                    session.local_evidence.push_str(&pivot_marker);
                    session.local_evidence.push('\n');
                    self.log(
                        "\x1b[1;36m>> [CORTEX AST-UNADDRESSABLE RECOVERY]\x1b[0m \
                         Symbol has no Tree-sitter body; pivoting to read_file."
                            .to_string(),
                    );
                }
            } else {
                self.log(
                    "\x1b[1;31m>> [CORTEX AST-UNADDRESSABLE RECOVERY]\x1b[0m \
                     Could not parse path from patch_function strategy; recovery aborted."
                        .to_string(),
                );
            }
        }
        // --- end AST-UNADDRESSABLE RECOVERY ---

        // --- POST-READ MUTATION PIVOT ---
        // After READ_OK on a mutation mission, inject patch_function / replace_lines
        // so strict action_matches_strategy does not reject mutation proposals
        // while active_strategy is still read_file.
        {
            let read_ok_done = session.local_evidence.contains("[READ_OK]");
            let patch_marker_needed = session.contract.is_mutation_task
                || session.contract.conditions.iter().any(|c| {
                    let s = format!("{:?}", c);
                    s.contains("PatchMarker") || s.contains("MutationMarker")
                })
                || {
                    let m = mission.to_lowercase();
                    m.contains("patch")
                        || m.contains("modify ")
                        || m.contains("remove the unconditional")
                };
            let already_mutated = session.local_evidence.contains("[PATCH_FUNCTION_OK]")
                || session.local_evidence.contains("[DIFF_PATCH_OK]")
                || session.local_evidence.contains("[REPLACE_LINES_OK]")
                || session.local_evidence.contains("[WRITE_OK]");

            if read_ok_done && patch_marker_needed && !already_mutated && !write_phase_ready {
                let mut target_path: Option<String> = None;
                for line in session.local_evidence.lines().rev() {
                    let t = line.trim();
                    if let Some(rest) = t.strip_prefix("[READ_OK]") {
                        let p = rest.trim().split_whitespace().next().unwrap_or("").trim();
                        if !p.is_empty() && p.ends_with(".rs") {
                            target_path = Some(p.to_string());
                            break;
                        }
                    }
                }
                if target_path.is_none() {
                    if let Some(rest) = active_strategy.strip_prefix("read_file:") {
                        let p = rest.trim();
                        if !p.is_empty() {
                            target_path = Some(p.to_string());
                        }
                    }
                }

                if let Some(ref path_str) = target_path {
                    let fn_name = {
                        let lower = mission.to_lowercase();
                        let dead = |n: &str| {
                            session.local_evidence.contains(&format!(
                                "Symbol `{}` has no Tree-sitter body", n
                            ))
                        };
                        // Prefer a symbol actually defined in the file read.
                        let mut found: Option<String> = None;
                        for line in session.local_evidence.lines() {
                            if line.contains("Defined symbols:") && line.contains(path_str.as_str()) {
                                for part in line.split("pub fn ").skip(1) {
                                    let name: String = part
                                        .chars()
                                        .take_while(|c| c.is_alphanumeric() || *c == '_')
                                        .collect();
                                    if !name.is_empty() && !dead(&name) {
                                        found = Some(name);
                                        break;
                                    }
                                }
                            }
                            if found.is_some() { break; }
                        }
                        found
                            .or_else(|| {
                                ["evaluate", "check_contract"]
                                    .iter()
                                    .find(|c| lower.contains(**c) && !dead(c))
                                    .map(|s| (*s).to_string())
                            })
                            .unwrap_or_else(|| "evaluate".to_string())
                    };

                    let mutation_strats = [
                        format!("patch_function:{}:{}", path_str, fn_name),
                        format!("replace_lines:{}", path_str),
                    ];

                    let mut injected = 0usize;
                    for strat in &mutation_strats {
                        let already = session.graph.find_by_content(strat).is_some();
                        if already {
                            if injected == 0 {
                                let existing_id = session.graph.find_by_content(strat);
                                if let Some(id) = existing_id {
                                    session.graph.promote_hypothesis(id, 0.56);
                                    session.best_hypothesis_id = Some(id);
                                    active_strategy = strat.clone();
                                    injected += 1;
                                    self.log(format!(
                                        "\x1b[1;36m>> [CORTEX POST-READ MUTATION PIVOT]\x1b[0m Promoted existing `{}`.",
                                        strat
                                    ));
                                }
                            }
                            continue;
                        }
                        let belief = if injected == 0 { 0.58 } else { 0.54 };
                        let hid = session.graph.add_node(
                            NodeKind::Hypothesis,
                            strat,
                            belief,
                            100,
                        );
                        if injected == 0 {
                            session.best_hypothesis_id = Some(hid);
                            active_strategy = strat.clone();
                            self.log(format!(
                                "\x1b[1;36m>> [CORTEX POST-READ MUTATION PIVOT]\x1b[0m Prefer `{}` after READ_OK on mutation mission.",
                                strat
                            ));
                        } else {
                            self.log(format!(
                                "\x1b[90m  -> Injected strategy: {}\x1b[0m",
                                strat
                            ));
                        }
                        injected += 1;
                    }

                    if injected > 0 {
                        session.graph.cap_matching(
                            |c| c.starts_with("read_file:") && c.contains(path_str.as_str()),
                            0.20,
                        );
                    }
                }
            }
        }
        // --- end POST-READ MUTATION PIVOT ---



        // HALT-VETO RECOVERY (HaltDelivered strategy-prefix lock fix):
        //
        // When the contract is solely HaltDelivered and the critic has just
        // vetoed a halt for lack of inspection evidence, the original
        // "halt: ..." hypothesis becomes sticky. Demote it and inject a
        // competing reconnaissance hypothesis so the node can escape.
        {
            let is_halt_delivered =
                session.contract.conditions.iter().any(|c| {
                    matches!(c, crate::completion_engine::CompletionCondition::HaltDelivered)
                });
            let critic_demands_inspection =
                session.local_evidence.lines().rev().take(60).any(|l| {
                    let lower = l.to_lowercase();
                    (lower.contains("critic")
                        || lower.contains("veto")
                        || lower.contains("dialectic"))
                        && (lower.contains("no source")
                            || lower.contains("not inspected")
                            || lower.contains("no [read_ok]")
                            || lower.contains("no [search")
                            || lower.contains("must search")
                            || lower.contains("mandatory first step")
                            || lower.contains("zero inspection")
                            || lower.contains("locate and read")
                            || lower.contains("first locate")
                            || lower.contains("hallucination")
                            || lower.contains("evidentiary support"))
                });
            if is_halt_delivered && critic_demands_inspection {
                if let Some(h_id) = session.best_hypothesis_id {
                    if session.graph.content_of(h_id).trim().starts_with("halt:") {
                        session.graph.demote_hypothesis(h_id, 0.05);
                    }
                }
                let recon = "search_code:crates/runtime_core/src evaluate_contract OR mission_completion_authorized OR HaltDelivered";
                let already = session.graph.find_by_content(recon).is_some();
                if !already {
                    let hid = session.graph.add_node(
                        crate::cognition_graph::NodeKind::Hypothesis,
                        recon,
                        0.56,
                        100,
                    );
                    session.best_hypothesis_id = Some(hid);
                    self.log(
                        "\x1b[1;36m>> [CORTEX HALT-VETO RECOVERY]\x1b[0m Injected reconnaissance hypothesis after critic veto on HaltDelivered contract."
                            .to_string(),
                    );
                }
            }
        }

        // POST-SEARCH EXPANSION (recon stall fix):
        //
        // The deterministic parsing of SEARCH_CODE MATCHES evidence is
        // delegated to the independent reconnaissance algorithm.
        // Cortex remains the executive coordinator: it evaluates the
        // contract, creates hypotheses, assigns belief, and selects the
        // active strategy.
        if !write_phase_ready
            && session.local_evidence.contains("[SEARCH_CODE_OK]")
            && session.local_evidence.contains("MATCHES:")
        {
            let contract_met = crate::completion_engine::SubgoalCompletionEngine::evaluate(
                &session.contract,
                &session.local_evidence,
                current_belief,
            );

            if !contract_met {
                let unread: Vec<String> = ReconnaissancePlanner::plan(&session.local_evidence, 4)
                    .into_iter()
                    .filter_map(|strategy| strategy.strip_prefix("read_file:").map(str::to_owned))
                    .collect();

                if !unread.is_empty() {
                    for (i, path) in unread.iter().enumerate() {
                        let strat = format!("read_file:{}", path);
                        let already = session.graph.find_by_content(&strat).is_some();

                        if already {
                            continue;
                        }

                        let belief = if i == 0 { 0.56 } else { 0.53 };
                        let hid = session.graph.add_node(NodeKind::Hypothesis, &strat, belief, 100);

                        if i == 0 {
                            session.best_hypothesis_id = Some(hid);
                            active_strategy = strat.clone();
                            self.log(format!(
                                "\x1b[1;36m>> [CORTEX POST-SEARCH EXPANSION]\x1b[0m Prefer `{}` (search hits not yet read).",
                                strat
                            ));
                        } else {
                            self.log(format!("\x1b[90m  -> Injected strategy: {}\x1b[0m", strat));
                        }
                    }
                }
            }
        }

        let enhanced_mission = if write_phase_ready {
            format!(
                "{}\n\n[PHASE MANDATE: RECONNAISSANCE VERIFIED]\nThe deterministic reconnaissance contract has been satisfied. The required deliverable MUST now be produced at '{}' using the native `write_file` action.",
                mission,
                unwritten_targets[0]
            )
        } else if active_strategy.starts_with("patch_function:") && critic_veto_present {
            format!(
                "{}\n\n[CRITIC VETO RECOVERY MANDATE]\nThe previous patch_function proposal was explicitly rejected by the Adversarial Critic. Do NOT repeat the rejected implementation or its `new_body`. Produce a materially different, mission-compliant patch_function action now. Preserve the inherited function signature. The current directive explicitly requires a clean, documented implementation, so the replacement MUST contain an appropriate documentation comment or equivalent source-level documentation while preserving the required `a + b` behavior. The next proposal MUST be a real source mutation, not a restatement or reformatting of the existing implementation.",
                mission
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
                    AgentAction::SearchCode { query, .. } if query.trim().is_empty()
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
            if should_synthesize_halt(&raw_proposal) {
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
fields. Do NOT repeat the rejected proposal verbatim.{}",
                raw_proposal.chars().take(500).collect::<String>(),
                schema_hint
            );
            session.local_evidence.push_str(&parse_fault_obs);
            session.local_evidence.push('\n');

            if let Some(h_id) = session.best_hypothesis_id {
                if let Some(new_belief) = session.graph.decay(h_id, 0.20) {
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

        // Compatibility is checked against the strategy the model was
        // actually asked to execute. When write_phase_ready forced a
        // write_file strategy, active_strategy differs from h_content;
        // comparing against h_content rejected correct write_file
        // proposals as STRATEGY VIOLATIONS. When write_phase_ready is
        // false, active_strategy == h_content and behavior is unchanged.
        if let Some(first_action) = actions.first() {
            let strategy_ok = Self::action_matches_strategy(&active_strategy, first_action);
            let advances_unmet =
                Self::action_advances_unmet_contract_condition(first_action, session, mission);
            if !strategy_ok && !advances_unmet {
                self.log(format!(
                    "\x1b[1;31m>> [STRATEGY VIOLATION]\x1b[0m Selected strategy '{}' produced incompatible action '{:?}'. Rejecting proposal and forcing re-strategy.",
                    h_content,
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

    /// Extract the target named on the first `<marker><target>` line of a
    /// mission's MANDATORY NODE SUCCESS CRITERIA block. Returns None when the
    /// mission does not name a target for that marker.
    ///
    /// Used by `action_advances_unmet_contract_condition` to keep the
    /// multi-criteria safety net parameter-aware: a `READ_OK:<path>` criterion
    /// must only be advanced by an action whose path matches the criterion,
    /// not by any read on any file.
    ///
    /// The marker is matched literally (e.g. "READ_OK:") and the target is
    /// the first non-empty line after the marker, trimmed of whitespace and
    /// of surrounding backticks or double quotes. This mirrors the
    /// extraction done by EpistemicEngine::generate_hypotheses for the same
    /// criteria blocks.

    fn extract_named_target(mission: &str, marker: &str) -> Option<String> {
        let pos = mission.find(marker)?;
        let remainder = &mission[pos + marker.len()..];
        let target = remainder.lines().next()?.trim().trim_matches('`').trim_matches('"');
        if target.is_empty() {
            None
        } else {
            Some(target.to_string())
        }
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
            let action_key = Self::action_key(&action);

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
                    AgentAction::PatchFunction { .. }
                        | AgentAction::ReadFunction { .. }
                        | AgentAction::ReadFileChunk { .. }
                        | AgentAction::ReadFile { .. }
                );

            if attempt_count >= 1 && !bypass_antiloop_for_retention {
                self.log(format!(
                    "\x1b[1;31m>> [ANTI-LOOP] Duplicate action [{}] filtered from batch.\x1b[0m",
                    action_key
                ));
                vetoed_keys.push(action_key);
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
                    matches!(actions.first(), Some(AgentAction::SearchCode { .. }));
                let strategy_is_search = h_content.starts_with("search_code:");
                let critic_veto_is_redundancy = critic_text.contains("redundant")
                    || critic_text.contains("already read")
                    || critic_text.contains("already contains")
                    || critic_text.contains("proceed immediately to writing")
                    || critic_text.contains("proceed immediately to patching");

                let action_is_write =
                    matches!(actions.first(), Some(AgentAction::WriteFile { .. }));
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
                    Ok(CortexState::Execute { actions, proposal, belief })
                } else if critic.confidence_modifier > -0.25 {
                    Ok(CortexState::Execute { actions, proposal, belief })
                } else {
                    self.log("\x1b[33m>> [DIALECTIC FEEDBACK] Veto registered in graph. Re-strategizing...\x1b[0m".to_string());
                    // Feed the critic's rejection back into the next
                    // Strategize -> Propose cycle. Graph-only feedback is not
                    // sufficient because the strategist receives local_evidence.
                    let veto_feedback = format!(
                        "\n[CRITIC_VETO]\n{}\n[CRITIC_VETO_ACTION] The rejected proposal MUST NOT be repeated. Select a materially different, mission-compliant action.\n",
                        critic.critique
                    );
                    session.local_evidence.push_str(&veto_feedback);

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
                && matches!(
                    action,
                    AgentAction::WriteFile { .. }
                        | AgentAction::DiffPatch { .. }
                        | AgentAction::PatchFunction { .. }
                        | AgentAction::SynthesizeReport { .. }
                        | AgentAction::ReplaceLines { .. }
                        | AgentAction::AtomicPatch { .. }
                )
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
            let action_key = Self::action_key(&action);

            // Captured before the move below so a failed execution's error
            // string can be scoped to the attempted path (see error-arm
            // usage further down). Path-scoping lets downstream path-bound
            // completion conditions (e.g. ReadPathAcquired) recognize a
            // confirmed-absent target as valid inspection evidence.
            let attempted_path: Option<String> = match &action {
                AgentAction::ReadFile { path, .. }
                | AgentAction::ReadFileChunk { path, .. }
                | AgentAction::WriteFile { path, .. }
                | AgentAction::DiffPatch { path, .. }
                | AgentAction::ReplaceLines { path, .. } => Some(path.trim().to_string()),
                AgentAction::ReadFunction { path, .. }
                | AgentAction::PatchFunction { path, .. } => Some(path.trim().to_string()),
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

                    // Bridge to the DAG verifier's command criterion. The
                    // compiler critic's package-scoped cargo check is
                    // semantically equivalent to the cargo check a mission
                    // would request. When the mission explicitly required
                    // such a command, emit the exact [CMD_OK] marker so
                    // CommandExecutionVerified (or legacy CMD_OK) is
                    // satisfied without a redundant execute_command action.
                    let required_cmd =
                        crate::completion_engine::extract_required_command(&session.mission);

                    let requirement_ok = match (&required_cmd, &target) {
                        (Some(cmd), RustVerificationTarget::WorkspacePackages { packages, .. }) => {
                            let c = cmd.trim();
                            c.starts_with("cargo check")
                                && packages.iter().any(|pkg| c.contains(pkg.as_str()))
                        }
                        (Some(_), RustVerificationTarget::StandaloneFiles { .. }) => false,
                        (Some(_), RustVerificationTarget::NoVerificationNeeded) => false,
                        (None, _) => true,
                    };

                    if requirement_ok {
                        let cmd_string = required_cmd
                            .unwrap_or_else(|| format!("cargo check -p {}", packages.join(" -p ")));
                        success_evidence.push_str(&format!(
                            "\n[CMD_OK] Command: {}\nStatus: exit status: 0\n",
                            cmd_string.trim(),
                        ));
                    }

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
                                if crate::podman::PodmanShield::execute(&format!(
                                    "git checkout -- {}",
                                    path
                                ))
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
                                if crate::podman::PodmanShield::execute(&format!(
                                    "git checkout -- {}",
                                    path
                                ))
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
                                if crate::podman::PodmanShield::execute(&format!(
                                    "git checkout -- {}",
                                    path
                                ))
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
    fn decay_hypothesis_belief(
        session: &mut EpistemicSession,
        h_id: uuid::Uuid,
        delta: f32,
    ) {
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
            session.graph.content_of(h_id);
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
    let AgentAction::PatchFunction { path, function_name, new_body, .. } = action else {
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

#[cfg(test)]
mod tests {
    use super::CorticalLoop;
    use crate::agent_protocol::AgentAction;
    use crate::cognition::epistemic_session::EpistemicSession;

    #[test]
    fn patch_function_key_is_body_sensitive() {
        let base = AgentAction::PatchFunction {
            path: "apps/math_utils/src/lib.rs".into(),
            function_name: "add".into(),
            new_body: "pub fn add(a: i32, b: i32) -> i32 { a + b }".into(),
            reasoning: "first".into(),
        };
        let same_body = AgentAction::PatchFunction {
            path: "apps/math_utils/src/lib.rs".into(),
            function_name: "add".into(),
            new_body: "pub fn add(a: i32, b: i32) -> i32 { a + b }".into(),
            reasoning: "retry with different reasoning".into(),
        };
        let diff_body = AgentAction::PatchFunction {
            path: "apps/math_utils/src/lib.rs".into(),
            function_name: "add".into(),
            new_body: "pub fn add(a: i32, b: i32) -> i32 {\n    a + b\n}".into(),
            reasoning: "first".into(),
        };

        let k_base = CorticalLoop::action_key(&base);
        let k_same = CorticalLoop::action_key(&same_body);
        let k_diff = CorticalLoop::action_key(&diff_body);

        assert_eq!(k_base, k_same, "reasoning must not affect the anti-loop key");
        assert_ne!(k_base, k_diff, "different new_body must produce a different key");
        assert!(
            k_base.starts_with("patch_function:apps/math_utils/src/lib.rs:add:"),
            "key must carry the patch_function prefix, path, and function"
        );
    }

    #[tokio::test]
    async fn classify_patch_action_detects_real_mutation() {
        let dir = std::env::temp_dir().join(format!("nexus_test_mutation_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("lib.rs");
        std::fs::write(&path, "pub fn add(a: i32, b: i32) -> i32 {\n    a + 0 + b\n}\n").unwrap();

        let action = AgentAction::PatchFunction {
            path: path.to_string_lossy().into_owned(),
            function_name: "add".into(),
            new_body: "pub fn add(a: i32, b: i32) -> i32 { a + b }".into(),
            reasoning: "test".into(),
        };

        match super::classify_patch_action(&action).await.unwrap() {
            super::PatchClass::Mutation { function_name, .. } => {
                assert_eq!(function_name, "add");
            }
            other => panic!("expected Mutation, got {:?}", other),
        }

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn classify_patch_action_detects_idempotent_body() {
        let dir =
            std::env::temp_dir().join(format!("nexus_test_idempotent_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("lib.rs");
        std::fs::write(&path, "pub fn add(a: i32, b: i32) -> i32 {\n    a + b\n}\n").unwrap();

        let action = AgentAction::PatchFunction {
            path: path.to_string_lossy().into_owned(),
            function_name: "add".into(),
            new_body: "pub fn add(a: i32, b: i32) -> i32 { a + b }".into(),
            reasoning: "test".into(),
        };

        match super::classify_patch_action(&action).await.unwrap() {
            super::PatchClass::Idempotent { function_name, .. } => {
                assert_eq!(function_name, "add");
            }
            other => panic!("expected Idempotent, got {:?}", other),
        }

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn classify_patch_action_ignores_non_patch_actions() {
        let action = AgentAction::ReadFile { path: "whatever.rs".into(), reasoning: "test".into() };
        assert!(matches!(
            super::classify_patch_action(&action).await.unwrap(),
            super::PatchClass::NotPatch
        ));
    }

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

    #[test]
    fn parse_failure_never_manufactures_a_typed_halt() {
        assert!(super::should_synthesize_halt("halt"));
        assert!(super::should_synthesize_halt("The task is done, halt."));
        assert!(!super::should_synthesize_halt(r#"[{"type":"halt","reasoning":{"a":1}}]"#));
        assert!(!super::should_synthesize_halt("no stop word here"));
    }

    #[test]
    fn extract_named_target_read_ok() {
        // Reproduces the 2026-09-14 audit run's turn-1 mission text.
        let mission = "OBJECTIVE: inspect\n\nMANDATORY NODE SUCCESS CRITERIA:\nREAD_OK:crates/runtime_core/src/cortical_loop.rs\n\nNODE COMPLETION RULE: ...";
        assert_eq!(
            CorticalLoop::extract_named_target(mission, "READ_OK:").as_deref(),
            Some("crates/runtime_core/src/cortical_loop.rs")
        );
    }

    #[test]
    fn read_only_contract_never_enters_write_phase() {
        // Exercise the production helper rather than duplicating its
        // predicate here. If !contract.read_only is ever removed from
        // CorticalLoop::write_phase_ready, this test fails.
        let mission = "READ-ONLY audit.";
        let contract =
            crate::completion_engine::SubgoalCompletionEngine::infer_contract("t", mission);

        let unwritten: Vec<&str> =
            contract.required_artifact_paths.iter().map(|a| a.path.as_str()).collect();

        assert!(!CorticalLoop::write_phase_ready(&contract, &unwritten, true));
    }

    #[test]
    fn floor_hypothesis_belief_after_failure_restores_threshold() {
        // Hypothesis was selectable before the evaluation (0.50),
        // collapsed to 0.05 after a single failed action, and must be
        // floored back to the selection threshold.
        let mut session = EpistemicSession::new("test");
        let h_id = session.graph.add_node(
            crate::cognition_graph::NodeKind::Hypothesis,
            "test hypothesis",
            0.05,
            100,
        );
        CorticalLoop::floor_hypothesis_belief_after_failure(&mut session, h_id, 0.50, 0.15);
        assert_eq!(session.graph.belief_of(h_id), Some(0.15));
    }

    #[test]
    fn floor_hypothesis_belief_after_failure_leaves_dead_hypotheses_alone() {
        // Hypothesis was already below the threshold before the
        // evaluation (0.10 < 0.15). The floor must not resurrect it.
        let mut session = EpistemicSession::new("test");
        let h_id = session.graph.add_node(
            crate::cognition_graph::NodeKind::Hypothesis,
            "test hypothesis",
            0.05,
            100,
        );
        CorticalLoop::floor_hypothesis_belief_after_failure(&mut session, h_id, 0.10, 0.15);
        assert_eq!(session.graph.belief_of(h_id), Some(0.05));
    }

    #[test]
    fn decay_hypothesis_belief_crosses_selection_floor() {
        // Pre-decay 0.30, delta 0.20 -> 0.10: the belief CROSSES
        // SELECT_FLOOR and becomes unselectable (F4: no clamp).
        let mut session = EpistemicSession::new("test");
        let h_id =
            session.graph.add_node(crate::cognition_graph::NodeKind::Hypothesis, "h", 0.30, 100);
        CorticalLoop::decay_hypothesis_belief(&mut session, h_id, 0.20);
        assert_eq!(session.graph.belief_of(h_id), Some((0.30_f32 - 0.20_f32).max(0.0)));
    }

    #[test]
    fn decay_hypothesis_belief_leaves_dead_hypotheses_alone() {
        // Pre-decay 0.10 is already below the floor. F4: no clamp exists
        // to raise it back; a dead hypothesis stays dead.
        let mut session = EpistemicSession::new("test");
        let h_id =
            session.graph.add_node(crate::cognition_graph::NodeKind::Hypothesis, "h", 0.10, 100);
        CorticalLoop::decay_hypothesis_belief(&mut session, h_id, 0.20);
        assert_eq!(session.graph.belief_of(h_id), Some(0.0));
    }

    #[test]
    fn extract_named_target_returns_none_when_absent() {
        let mission = "OBJECTIVE: inspect the code and report.";
        assert_eq!(CorticalLoop::extract_named_target(mission, "READ_OK:"), None);
    }

    #[test]
    fn extract_named_target_strips_backticks_and_quotes() {
        let mission = "READ_OK:`crates/runtime_core/src/cortical_loop.rs`\n";
        assert_eq!(
            CorticalLoop::extract_named_target(mission, "READ_OK:").as_deref(),
            Some("crates/runtime_core/src/cortical_loop.rs")
        );
        let mission2 = "READ_OK:\"crates/runtime_core/src/router.rs\"\n";
        assert_eq!(
            CorticalLoop::extract_named_target(mission2, "READ_OK:").as_deref(),
            Some("crates/runtime_core/src/router.rs")
        );
    }
}

/// True only for prose-only proposals that mention "halt". Typed JSON
/// proposals (containing a "type" field) that failed validation return false.
fn should_synthesize_halt(raw: &str) -> bool {
    raw.trim().to_lowercase().contains("halt") && !raw.contains("\"type\"")
}
