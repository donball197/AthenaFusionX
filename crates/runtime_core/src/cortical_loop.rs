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
use crate::contract_status::{ContractEvaluator, ContractStatus};
use crate::cortical::capability::capability_for;
use crate::cortical::compiler_reconciler::CompilerVerificationOutcome;
use crate::cortical::critic_filter::AntiLoopOutcome;
use crate::cortical::prompt_builder::build_enhanced_mission;
use crate::cortical::python_conformance::{python_conformance_defects, python_syntax_error};
use crate::cortical::terminal_gate::TerminalGate;
use crate::distributed_scheduler::DistributedScheduler;
use crate::epistemic_engine::EpistemicEngine;
use crate::executor::RoboticExecutor;
use crate::intent_contract::IntentContract;
use crate::memory_cortex::MemoryCortex;
use crate::motor_cortex::MotorCortex;
use crate::policy::Capability;
use crate::shared_state::VmoState;
use crate::world_model::WorldModel;

mod strategize;
mod critic;
mod execute;
mod evaluate;

#[derive(Debug)]
pub enum CortexOutcome {
    CompletionProposed { belief: f32, evidence: String },
    TurnsExhausted { best_belief: f32, evidence: String },
}

struct CriticInput {
    actions: Vec<AgentAction>,
    proposal: String,
    belief: f32,
}

struct EvaluationInput {
    evidence: String,
    executed_action: String,
    belief: f32,
    is_halt_action: bool,
    mutation_path: Option<String>,
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
            if crate::revert::revert_mutation_path(path).is_err() {
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
        let world_context =
            crate::cortical::util::compress_context(&context_chunks.join("\n"), 6000);

        if let Some(evidence) = ancestor_evidence {
            if !evidence.trim().is_empty() {
                use sha2::{Digest, Sha256};
                let trimmed = evidence.trim();
                let digest = format!("{:x}", Sha256::digest(trimmed.as_bytes()));
                session.local_evidence.push_str(&format!(
                    "\n[DAG LINEAGE INHERITED EVIDENCE sha256:{}]\n{}\n[DAG LINEAGE COMPLETE sha256:{}]\n",
                    digest,
                    // FIX BUG-06: inherit only physical evidence lines; drop
                    // [RECON_REQUESTED], [CRITIC_*], [HALT_REJECTED_*] chatter.
                    trimmed.lines().filter(|l| {
                        let l = l.trim_start();
                        l.starts_with("[READ_OK]") || l.starts_with("[READ_CHUNK_OK]")
                            || l.starts_with("[CMD_OK]") || l.starts_with("[WRITE_OK]")
                            || l.starts_with("[PATCH_OK]") || l.starts_with("[REPLACE_LINES_OK]")
                    }).collect::<Vec<_>>().join("\n"),
                    digest
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
                // BUG-17: keys are opaque hashes, so substring-matching
                // them never matched hash-keyed variants (ReadFile/
                // WriteFile/DiffPatch/...). Use the canonical predicate
                // against the stored actions instead.
                if session
                    .executed_action_log
                    .iter()
                    .any(|a| crate::cortical::action_identity::action_touches_path(a, &event.path))
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
                let term_gate = TerminalGate::evaluate(&session, current_belief);

                if term_gate.contract_satisfied && !term_gate.compile_remediation_resolved {
                    self.log(
                        "\x1b[1;33m>> [CORTEX] Contract satisfied by static evidence, but an unresolved compile failure is active. Continuing remediation.\x1b[0m"
                            .to_string(),
                    );
                }

                if term_gate.is_autonomous_completion_satisfied() {
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

                        self.log_turns_exhausted(&session, current_belief);
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
                if Self::evidence_satisfies_contract(&session) {
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
                    self.handle_strategize(turn, &mut session, mission, &world_context).await?
                }
                CortexState::Critique { actions, proposal, belief } => {
                    self.handle_critic(
                        turn,
                        &mut session,
                        mission,
                        &world_context,
                        CriticInput { actions, proposal, belief },
                    )
                    .await?
                }
                CortexState::Execute { actions, proposal, belief } => {
                    self.handle_execute(&mut session, actions, proposal, belief).await?
                }
                CortexState::Evaluate {
                    evidence,
                    executed_action,
                    belief,
                    is_halt_action,
                    mutation_path,
                } => {
                    self.handle_evaluate(
                        &mut session,
                        mission,
                        EvaluationInput {
                            evidence,
                            executed_action,
                            belief,
                            is_halt_action,
                            mutation_path,
                        },
                    )
                    .await?
                }
                CortexState::Complete { belief, evidence } => {
                    return Ok(CortexOutcome::CompletionProposed { belief, evidence })
                }
                CortexState::Fail { belief, .. } => {
                    // A successful rollback deletes or restores the mutated
                    // files, so evidence recorded for those mutations (e.g.
                    // [WRITE_OK]) no longer describes the disk. It must not
                    // be used to declare the node complete.
                    let mut reverted_mutations = false;
                    if session.compile_remediation.is_active() {
                        // Read before the call: a successful rollback clears it.
                        let had_paths = !session.active_mutation_paths.is_empty();
                        let rollback_ok = Self::rollback_active_mutations(&mut session);
                        reverted_mutations = had_paths && rollback_ok;
                        if reverted_mutations {
                            self.log(
                                "\x1b[1;33m>> [CORTEX] Fail branch reverted active mutations; accumulated evidence for them is stale and cannot satisfy the contract.\x1b[0m"
                                    .to_string(),
                            );
                        }
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
                    if Self::evidence_satisfies_contract(&session) && !reverted_mutations {
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
                    self.log_turns_exhausted(&session, belief);
                    return Ok(CortexOutcome::TurnsExhausted {
                        best_belief: belief,
                        evidence: session.local_evidence.clone(),
                    });
                }
            };
        }
    }

    /// True when accumulated physical evidence already satisfies the
    /// deterministic contract and no semantic verification is required.
    /// Belief is probed at 1.0 so a decayed MinBeliefScore cannot circularly
    /// block the check.
    fn evidence_satisfies_contract(session: &EpistemicSession) -> bool {
        use crate::completion_engine::{requires_semantic_verification, SubgoalCompletionEngine};
        SubgoalCompletionEngine::evaluate(&session.contract, &session.local_evidence, 1.0)
            && !requires_semantic_verification(&session.contract.objective)
    }

    fn log_turns_exhausted(&self, session: &EpistemicSession, belief: f32) {
        let g = TerminalGate::evaluate(session, belief);
        self.log(format!(
            "\x1b[1;35m>> [CORTEX DIAG] TurnsExhausted: contract_satisfied={} compile_resolved={} semantic_required={} belief={:.2} evidence_bytes={}\x1b[0m",
            g.contract_satisfied,
            g.compile_remediation_resolved,
            crate::completion_engine::requires_semantic_verification(&session.contract.objective),
            belief,
            session.local_evidence.len()
        ));
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
pub(crate) use crate::cortical::patch_classification::{classify_patch_action, PatchClass};

#[cfg(test)]
#[path = "cortical/loop_tests.rs"]
mod tests;
