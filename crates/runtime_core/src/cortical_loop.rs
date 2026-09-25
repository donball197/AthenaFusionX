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
use crate::compile_remediation::{
    format_compile_failure_evidence, CompileDecision, COMPILE_RETRY_BUDGET,
};
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
}

/// Resolve the Cargo package(s) owning the supplied mutated paths.
///
/// Cargo metadata is authoritative here. This deliberately avoids parsing
/// Cargo.toml by hand and avoids depending on the daemon's current directory.
///
/// The runtime_core manifest is known at compile time, so the workspace
/// manifest can be derived from CARGO_MANIFEST_DIR and supplied explicitly
/// to Cargo metadata.
async fn resolve_mutation_packages(
    mutation_paths: &[String],
) -> Result<(std::path::PathBuf, Vec<String>), String> {
    let workspace_manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .map(|p| p.join("Cargo.toml"))
        .ok_or_else(|| {
            "unable to derive workspace Cargo.toml from CARGO_MANIFEST_DIR".to_string()
        })?;

    if !workspace_manifest.exists() {
        return Err(format!(
            "workspace Cargo.toml does not exist: {}",
            workspace_manifest.display()
        ));
    }

    let output = tokio::process::Command::new("cargo")
        .args(["metadata", "--no-deps", "--format-version=1", "--manifest-path"])
        .arg(&workspace_manifest)
        .output()
        .await
        .map_err(|e| format!("cargo metadata failed to execute: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let preview = stderr.lines().take(20).collect::<Vec<_>>().join("\n");

        return Err(format!("cargo metadata exited non-zero ({}):\n{}", output.status, preview));
    }

    let meta: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("cargo metadata JSON parse failed: {e}"))?;

    let workspace_root = meta
        .get("workspace_root")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "cargo metadata omitted workspace_root".to_string())?;

    let workspace_root = std::path::PathBuf::from(workspace_root);

    let packages = meta
        .get("packages")
        .and_then(|v| v.as_array())
        .ok_or_else(|| "cargo metadata omitted packages".to_string())?;

    let mut resolved = Vec::new();

    for mutated_path in mutation_paths {
        let absolute_path = {
            let candidate = std::path::Path::new(mutated_path);

            if candidate.is_absolute() {
                candidate.to_path_buf()
            } else {
                workspace_root.join(candidate)
            }
        };

        let mut owner = None;

        for package in packages {
            let Some(name) = package.get("name").and_then(|v| v.as_str()) else {
                continue;
            };

            let Some(manifest_path) = package.get("manifest_path").and_then(|v| v.as_str()) else {
                continue;
            };

            let package_root = std::path::Path::new(manifest_path).parent().ok_or_else(|| {
                format!("invalid package manifest path in cargo metadata: {}", manifest_path)
            })?;

            if absolute_path.starts_with(package_root) {
                owner = Some(name.to_string());
                break;
            }
        }

        let Some(package_name) = owner else {
            return Err(format!(
                "unable to resolve Cargo package for mutated path: {}",
                mutated_path
            ));
        };

        if !resolved.contains(&package_name) {
            resolved.push(package_name);
        }
    }

    if resolved.is_empty() {
        return Err("no Cargo package could be resolved for the Rust mutation".to_string());
    }

    Ok((workspace_manifest, resolved))
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

                if deterministic_contract_satisfied && compile_remediation_resolved {
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
    fn action_key(action: &AgentAction) -> String {
        match action {
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
            AgentAction::PatchFunction { path, function_name, new_body, .. } => {
                use std::hash::{Hash, Hasher};
                let mut hasher = std::collections::hash_map::DefaultHasher::new();
                new_body.hash(&mut hasher);
                format!("patch_function:{}:{}:{:016x}", path, function_name, hasher.finish())
            }
            AgentAction::SynthesizeReport { output_path, .. } => {
                format!("synthesize_report:{}", output_path)
            }
            AgentAction::ExecuteCommand { command, .. } => {
                format!("execute_command:{}", command.trim())
            }
            // Halt is a terminal control-flow signal, not a substantive action.
            // Dedupe by type alone: two halt proposals are identical attempts
            // regardless of how the model phrases its reasoning. Without this,
            // the strategist can spam halt with cosmetic reasoning variations
            // and the anti-loop filter never fires -- the critic vetoes each
            // instance instead, burning a belief-decay cycle per turn.
            AgentAction::Halt { .. } => "halt".to_string(),
            _ => format!("{:?}", action),
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
                        pt == expected_path || (pt.starts_with(&prefix) && !pt.contains(".."))
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
            // Allow Halt to be evaluated; handle_evaluate verifies if artifacts exist
            if matches!(action, AgentAction::Halt { .. }) {
                return true;
            }
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
    ) -> bool {
        use crate::completion_engine::CompletionCondition as CC;

        for cond in &session.contract.conditions {
            if Self::condition_is_met(cond, &session.local_evidence) {
                continue;
            }
            let advances = match cond {
                CC::ReadMarkerAcquired => matches!(
                    action,
                    AgentAction::ReadFile { .. } | AgentAction::ReadFileChunk { .. }
                ),
                CC::SearchMarkerAcquired => matches!(action, AgentAction::SearchCode { .. }),
                CC::SearchMemoryMarkerAcquired => {
                    matches!(action, AgentAction::SearchMemory { .. })
                }
                CC::WriteMarkerAcquired => matches!(
                    action,
                    AgentAction::WriteFile { .. } | AgentAction::SynthesizeReport { .. }
                ),
                CC::PatchMarkerAcquired => matches!(
                    action,
                    AgentAction::PatchFunction { .. }
                        | AgentAction::DiffPatch { .. }
                        | AgentAction::ReplaceLines { .. }
                ),
                CC::CommandMarkerAcquired => {
                    matches!(action, AgentAction::ExecuteCommand { .. })
                }
                CC::CommandExecutionVerified { .. } => {
                    matches!(action, AgentAction::ExecuteCommand { .. })
                }
                CC::ReadFunctionMarkerAcquired { .. } => {
                    matches!(action, AgentAction::ReadFunction { .. })
                }
                CC::SourceCodeEvidenceRequired => matches!(
                    action,
                    AgentAction::ReadFile { .. }
                        | AgentAction::ReadFileChunk { .. }
                        | AgentAction::SearchCode { .. }
                ),
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
        matches!(
            crate::outcome_verifier::OutcomeVerifier::verify(
                "probe",
                evidence,
                1.0,
                std::slice::from_ref(cond),
            ),
            crate::outcome_verifier::CompletionStatus::Approved
        )
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

        let critic_veto_present = session.local_evidence.contains("[CRITIC_VETO]");
        let patch_function_strategy = active_strategy.starts_with("patch_function:");

        self.log(format!(
            "[CORTEX RECOVERY CHECK] strategy=`{}` patch_function={} critic_veto={} write_phase_ready={} evidence_bytes={}",
            active_strategy,
            patch_function_strategy,
            critic_veto_present,
            write_phase_ready,
            session.local_evidence.len()
        ));

        let enhanced_mission = if write_phase_ready {
            format!(
                "{}\n\n[PHASE MANDATE: RECONNAISSANCE VERIFIED]\nThe deterministic reconnaissance contract has been satisfied. The required deliverable MUST now be produced at '{}' using the native `write_file` action.",
                mission,
                unwritten_targets[0]
            )
        } else if patch_function_strategy && critic_veto_present {
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
                if let Some(node) = session.graph.nodes.get_mut(&h_id) {
                    node.belief_score = (node.belief_score - 0.20).max(0.0);
                    self.log(format!(
                        "\x1b[1;31m>> [PARSE_FAULT] Hypothesis belief decayed to {:.2} to force pivot\x1b[0m",
                        node.belief_score
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
                Self::action_advances_unmet_contract_condition(first_action, session);
            if !strategy_ok && !advances_unmet {
                self.log(format!(
                    "\x1b[1;31m>> [STRATEGY VIOLATION]\x1b[0m Selected strategy '{}' produced incompatible action '{:?}'. Rejecting proposal and forcing re-strategy.",
                    h_content,
                    first_action
                ));

                if let Some(h_id) = session.best_hypothesis_id {
                    if let Some(node) = session.graph.nodes.get_mut(&h_id) {
                        node.belief_score = (node.belief_score - 0.20).max(0.0);
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
            let action_key = Self::action_key(&action);

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
            self.log(
                "\x1b[36m>> [COMPILER CRITIC] Resolving mutated Rust package(s) via cargo metadata...\x1b[0m"
                    .to_string(),
            );

            let (workspace_manifest, packages) =
                match resolve_mutation_packages(&mutation_paths).await {
                    Ok(result) => result,
                    Err(error) => {
                        let message = format!("[COMPILER_FAULT] {}", error);

                        self.log(format!("\x1b[1;31m>> [COMPILER CRITIC] {}\x1b[0m", message));

                        session.local_evidence.truncate(evidence_checkpoint);
                        session.local_evidence.push_str(&message);
                        session.local_evidence.push('\n');

                        if let Some(h_id) = session.best_hypothesis_id {
                            let obs_id =
                                session.graph.add_node(NodeKind::Observation, &message, 1.0, 90);

                            session.graph.add_edge(obs_id, h_id, EdgeRelation::Contradicts, 0.99);

                            session.graph.propagate_beliefs();
                        }

                        return Ok(CortexState::Strategize);
                    }
                };

            self.log(format!(
                "\x1b[36m>> [COMPILER CRITIC] Package(s): {}\x1b[0m",
                packages.join(", ")
            ));

            let mut command = tokio::process::Command::new("cargo");

            command.arg("check").arg("--manifest-path").arg(&workspace_manifest);

            for package in &packages {
                command.arg("-p").arg(package);
            }

            self.log(format!(
                "\x1b[36m>> [COMPILER CRITIC] Running package-scoped cargo check using {}...\x1b[0m",
                workspace_manifest.display()
            ));

            let compile_output = command.output().await;

            match compile_output {
                Ok(output) if output.status.success() => {
                    session.compile_remediation.reset();

                    let mut success_evidence =
                        String::from("\n[COMPILE_OK] Mutated package(s) compile successfully.\n");

                    // Bridge to the DAG verifier's command criterion. The
                    // compiler critic's package-scoped cargo check is
                    // semantically equivalent to the cargo check a mission
                    // would request. When the mission explicitly required
                    // such a command, emit the exact [CMD_OK] marker so
                    // CommandExecutionVerified (or legacy CMD_OK) is
                    // satisfied without a redundant execute_command action.
                    let required_cmd =
                        crate::completion_engine::extract_required_command(&session.mission);

                    let requirement_ok = match &required_cmd {
                        Some(cmd) => {
                            let c = cmd.trim();
                            c.starts_with("cargo check")
                                && packages.iter().any(|pkg| c.contains(pkg.as_str()))
                        }
                        None => true,
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
                        "\x1b[32m>> [COMPILER CRITIC] Package-scoped compile OK. Proceeding to LLM Evaluator.\x1b[0m"
                            .to_string(),
                    );
                }

                Ok(output) => {
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    let stderr = String::from_utf8_lossy(&output.stderr);

                    let err_preview = stdout
                        .lines()
                        .chain(stderr.lines())
                        .take(15)
                        .collect::<Vec<_>>()
                        .join("\n");

                    let decision = session.compile_remediation.classify(false);

                    match &decision {
                        CompileDecision::RetainForRetry { iteration } => {
                            session.active_mutation_paths = mutation_paths.clone();
                            self.log(format!(
                                "\x1b[1;33m>> [COMPILER CRITIC] Package-scoped compile FAILED (iteration {}/{}). Retaining mutation; feeding diagnostic to strategist.\x1b[0m",
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
                                        "[COMPILE_FAIL] Patch failed package-scoped cargo check. Reverted."
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

                Err(error) => {
                    let message = format!(
                        "[COMPILER_FAULT] Failed to execute package-scoped cargo check: {}",
                        error
                    );

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
}
