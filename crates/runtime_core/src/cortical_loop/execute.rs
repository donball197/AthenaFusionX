//! CorticalLoop::handle_execute (moved verbatim from cortical_loop.rs).

use super::*;

impl CorticalLoop {
    pub(super) async fn handle_execute(
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
        let mut turn_typed_evidence: Vec<crate::evidence_adapter::TypedEvidenceRecord> = Vec::new();
        let mut executed_actions: Vec<String> = Vec::new();
        let mut mutation_paths: Vec<String> = Vec::new();
        let pre_write_corpus = session.local_evidence.clone();
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
                // A read-only mission forbids mutations of the substrate.
                // It does NOT forbid running read-only commands. `git status`,
                // `git log`, `git branch`, `git rev-parse`, `find`, `cat`,
                // `grep`, `ls`, `head`, `tail`, `wc`, `sort` are the very
                // operations a read-only audit needs to gather evidence.
                // `is_read_only_safe_command` statically analyzes a shell
                // command (segmenting on pipes/&&/||, checking the first token
                // against SAFE_COMMANDS, and checking git subcommands against
                // SAFE_GIT, with flag-level escapes like --output refused).
                // Only when that analysis says the command is safe do we
                // exempt the action from this violation block.
                //
                // Root cause of the 2026-10-25 crate-audit Stage A abort:
                // Stage A requires `git status`, `git branch`, `git rev-parse`;
                // the gate refused all three as read-only violations because
                // it classified every ExecuteCommand as mutating regardless
                // of the actual command text.
                let exec_is_provably_safe = matches!(
                    &action,
                    AgentAction::ExecuteCommand { _thought_process: None, command, .. }
                        if crate::cortical::read_only_gate::is_read_only_safe_command(command)
                );
                if !exec_is_provably_safe {
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
                // else: provably-safe read-only ExecuteCommand — fall through
                // and let the executor run it.
            }

            let intent = IntentContract::new(&proposal, belief);
            let capability = capability_for(&action);

            // Candidate paths are scoped to this action. They are
            // rolled back if the action is rejected or execution fails.
            let mutation_paths_checkpoint = mutation_paths.len();
            let rust_files_mutated_checkpoint = rust_files_mutated;

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
                | AgentAction::ReplaceLines { _thought_process: None, path, .. } => {
                    Some(path.clone())
                }
                AgentAction::SynthesizeReport { _thought_process: None, output_path, .. } => {
                    Some(output_path.clone())
                }
                _ => None,
            };

            if let Some(path) = mutation_path {
                mutation_paths.push(path.clone());
                if path.ends_with(".rs") {
                    rust_files_mutated = true;
                }
            }

            // Serialize the exact AgentAction before MotorCortex::translate()
            // consumes it. This is the authoritative action record that will
            // cross the execution/evaluation boundary.
            let action_is_halt =
                matches!(&action, AgentAction::Halt { _thought_process: None, .. });

            let executed_action_json = serde_json::to_string(&action)
                .map_err(|e| anyhow::anyhow!("Failed to serialize executed AgentAction: {}", e))?;
            let action_snapshot = action.clone();

            // Build the canonical action key before MotorCortex::translate()
            // consumes the AgentAction.
            let action_key = crate::cortical::critic_filter::attempt_key(session, &action);

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
                | AgentAction::ReplaceLines { _thought_process: None, path, .. } => {
                    Some(path.trim().to_string())
                }
                AgentAction::ReadFunction { _thought_process: None, path, .. }
                | AgentAction::PatchFunction { _thought_process: None, path, .. } => {
                    Some(path.trim().to_string())
                }
                _ => None,
            };

            let motor_cmd =
                MotorCortex::translate(&intent, None, action.clone(), capability, proposal.clone());

            if let Err(gate_err) = self.action_gate.approve(&motor_cmd) {
                mutation_paths.truncate(mutation_paths_checkpoint);
                rust_files_mutated = rust_files_mutated_checkpoint;
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
            // BUG-17: log the action itself so the code-shift listener
            // can answer "did we touch path P?" without parsing a hash.
            // Without this call, `executed_action_log` stays empty and
            // every self-write is misclassified as an external shift.
            session.record_action_attempt(&action);
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
                Ok(ev) => {
                    // The latest successfully executed action determines
                    // whether this batch ended with a HALT action.
                    last_is_halt = action_is_halt;
                    ev
                }
                Err(e) => {
                    // The executor did not confirm success. Discard this
                    // action's candidate paths so they cannot independently
                    // trigger compilation or become a revert target.
                    mutation_paths.truncate(mutation_paths_checkpoint);
                    rust_files_mutated = rust_files_mutated_checkpoint;

                    match &attempted_path {
                        Some(p) => format!("[ACTION FAILED] path={} OS rejected action: {}", p, e),
                        None => format!("[ACTION FAILED] OS rejected action: {}", e),
                    }
                }
            };

            let typed_rec =
                crate::evidence_adapter::from_executed_action(&action_snapshot, &evidence);
            session.typed_evidence.push(typed_rec.clone());
            turn_typed_evidence.push(typed_rec);

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
                let obj_lower = session.contract.objective.to_lowercase();
                let is_existence_check = obj_lower.contains("check for")
                    || obj_lower.contains("check whether")
                    || obj_lower.contains("legacy")
                    || obj_lower.contains("exists")
                    || obj_lower.contains("existence");

                if is_existence_check {
                    // Absence is the desired evidence for an existence-check objective.
                    let obs_id = session.graph.add_node(
                        NodeKind::Observation,
                        "[PATH_DOES_NOT_EXIST] Confirmed absence — this satisfies an existence-check objective. Do not retry the path; proceed with remaining required actions.",
                        1.0,
                        95,
                    );
                    // Prefer Supports if available; otherwise the observation alone is enough.
                    session.graph.add_edge(obs_id, h_id, EdgeRelation::Supports, 0.60);
                    session.graph.propagate_beliefs();
                } else {
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
                let content = match tokio::fs::read_to_string(p).await {
                    Ok(content) => content,
                    Err(err) => {
                        failures.push(format!(
                            "`{}`: unable to read file for conformance verification: {}",
                            p, err
                        ));
                        continue;
                    }
                };
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
                // Invalidate the specific failed write(s) so completion
                // conditions cannot treat them as satisfied. Unrelated
                // evidence is preserved. The failure marker blocks HALT.
                for fail in &failures {
                    // failures entries look like `path`: defect details
                    let path_part = fail.split(':').next().unwrap_or("").trim().trim_matches('`');
                    if path_part.is_empty() {
                        continue;
                    }
                    let mut kept = String::new();
                    for line in session.local_evidence.lines() {
                        let drop = line.contains("[WRITE_OK]") && line.contains(path_part);
                        if !drop {
                            kept.push_str(line);
                            kept.push('\n');
                        }
                    }
                    session.local_evidence = kept;
                }
                session.local_evidence.push_str(&message);
                session.local_evidence.push('\n');
                return Ok(CortexState::Strategize);
            }

            // All candidate Python paths passed conformance. Remove stale
            // failure records only after this successful verification.
            if mutation_paths.iter().any(|p| p.ends_with(".py")) {
                session.local_evidence = session
                    .local_evidence
                    .lines()
                    .filter(|line| !line.starts_with("[WRITE_CONFORMANCE_FAILED]"))
                    .collect::<Vec<_>>()
                    .join("\n");
                if !session.local_evidence.is_empty() {
                    session.local_evidence.push('\n');
                }
            }

            // PHASE 3: Content-consistency check for written report artifacts.
            // Prevents fabricated reports from satisfying WritePathAcquired.
            let allow_obj = !session.contract.conditions.iter().any(|c| {
                use crate::completion_engine::CompletionCondition as CC;
                matches!(
                    c,
                    CC::ReadMarkerAcquired
                        | CC::ReadPathAcquired { .. }
                        | CC::ReadFunctionMarkerAcquired { .. }
                        | CC::SearchMarkerAcquired
                        | CC::SearchTermAcquired { .. }
                        | CC::SearchCheckedAcquired { .. }
                        | CC::SourceCodeEvidenceRequired
                        | CC::CommandMarkerAcquired
                        | CC::CommandExecutionVerified { .. }
                )
            });
            let mut report_failures: Vec<(String, Vec<String>)> = Vec::new();
            for p in mutation_paths
                .iter()
                .filter(|p| crate::cortical::halt_claims::is_report_artifact(p))
            {
                if let Ok(content) = tokio::fs::read_to_string(p).await {
                    let bad = crate::cortical::halt_claims::write_report_unverified_symbols(
                        p,
                        &content,
                        &pre_write_corpus,
                        &session.contract.objective,
                        allow_obj,
                    );
                    if !bad.is_empty() {
                        report_failures.push((p.clone(), bad));
                    }
                }
            }
            if !report_failures.is_empty() {
                for (p, bad) in &report_failures {
                    let msg = format!(
                        "[WRITE_CONFORMANCE_FAILED] Report `{}` cites symbol(s) never observed in READ_OK/evidence: {}. Read the defining source file(s) before citing those symbols, or remove unverified claims.",
                        p,
                        bad.join(", ")
                    );
                    self.log(format!("\x1b[1;31m>> [REPORT CONSISTENCY] {}\x1b[0m", msg));
                    let mut kept = String::new();
                    for line in session.local_evidence.lines() {
                        let drop = (line.contains("[WRITE_OK]") || line.contains("[WRITE_NOOP]"))
                            && line.contains(p.as_str());
                        if !drop {
                            kept.push_str(line);
                            kept.push('\n');
                        }
                    }
                    session.local_evidence = kept;
                    session.local_evidence.push_str(&msg);
                    session.local_evidence.push('\n');
                    for rec in session.typed_evidence.iter_mut() {
                        if rec.kind == crate::evidence_adapter::TypedEvidenceKind::Write
                            && rec
                                .evidence
                                .target
                                .as_ref()
                                .is_some_and(|t| crate::evidence_paths::paths_equivalent(&t.0, p))
                        {
                            rec.evidence.success = false;
                            rec.reliability = 0.0;
                        }
                    }
                }
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
            match self
                .verify_and_reconcile_compiler(
                    session,
                    &mutation_paths,
                    evidence_checkpoint,
                    &mut latest_evidence,
                )
                .await?
            {
                CompilerVerificationOutcome::Passed => {}
                CompilerVerificationOutcome::NeedsStrategize => {
                    return Ok(CortexState::Strategize);
                }
            }
        }

        Ok(CortexState::Evaluate {
            evidence: latest_evidence,
            executed_action: executed_actions.join("\n"),
            belief,
            is_halt_action: last_is_halt,
            mutation_path: mutation_paths.last().cloned(),
            typed_evidence: turn_typed_evidence,
        })
    }
}
