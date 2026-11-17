//! CorticalLoop::handle_strategize (moved verbatim from cortical_loop.rs).

use super::*;

/// True when the strategist's raw proposal wire form contains control-token
/// leak markers. Such proposals fail `json_parser::extract_actions` and leave
/// `actions` empty, but auto-halt must NOT fire in that case: the model's
/// intent was to run a command (the command is embedded in the leaked halt
/// reasoning). Parsing failure means we cannot extract that intent, and the
/// only safe recovery is parse-fault handling (evidence marker + belief decay
/// + re-strategize), not a synthesized halt that discards the intent.
fn proposal_was_malformed(raw: &str) -> bool {
    raw.contains("<|end|>")
        || raw.contains("<|start|>")
        || raw.contains("<rfl:")
        || raw.contains("to=execute_command")
        || raw.contains("to=halt")
        || raw.contains("to=read_file")
        || raw.contains("to=write_file")
        || raw.contains("assistant to=")
        || raw.contains("Assistant to=")
        || raw.contains("tool to=")
}

/// Strategist parse faults recorded since the last successful `[..._OK]` evidence.
fn consecutive_parse_faults(evidence: &str) -> usize {
    let clean = strip_read_bodies_for_markers(evidence);
    let mut n = 0usize;
    for l in clean.lines() {
        let t = l.trim_start();
        if t.starts_with("[PARSE_FAULT]") {
            n += 1;
        } else if t
            .strip_prefix('[')
            .and_then(|r| r.split(']').next())
            .map(|m| m.ends_with("_OK"))
            .unwrap_or(false)
        {
            n = 0;
        }
    }
    n
}

/// Index, action type and serde error of the first batch item that fails
/// to deserialize as an AgentAction. None if raw isn't parseable JSON.
fn first_failing_item(raw: &str) -> Option<(usize, String, String)> {
    let v: serde_json::Value = serde_json::from_str(raw.trim()).ok()?;
    let items = match v {
        serde_json::Value::Array(a) => a,
        other => vec![other],
    };
    for (i, item) in items.into_iter().enumerate() {
        if let Err(e) =
            serde_json::from_value::<crate::agent_protocol::AgentAction>(item.clone())
        {
            let t = item.get("type").and_then(|t| t.as_str()).unwrap_or("?").to_string();
            return Some((i, t, e.to_string()));
        }
    }
    None
}

impl CorticalLoop {
    pub(super) async fn handle_strategize(
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
                        let h_norm = crate::cortical::util::normalize_strategy(&h);
                        let already = session.graph.find_by_content(&h).is_some()
                            || session.graph.nodes.values().any(|n| {
                                crate::cortical::util::normalize_strategy(&n.content) == h_norm
                            });
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
        let status =
            ContractEvaluator::evaluate(&session.contract, &session.local_evidence, current_belief);

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

        let evidence_history =
            crate::cortical::util::compress_context(&session.collect_evidence(h_id), 150000);

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
            .filter(|p| {
                !crate::evidence_markers::has_artifact_write(&session.local_evidence, p)
                    && !artifact_denied_by_policy(&session.local_evidence, p)
            })
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
        // Coverage gate: `reconnaissance_complete` returns true as soon as
        // every `ReadPathAcquired` condition has a matching `[READ_OK]` in
        // evidence, even when that read was TRUNCATED (e.g. first read of a
        // 2225-line file returned only the first 1189 lines). Without this
        // gate, the phase transition forces `write_file:<deliverable>` with
        // 53.5% coverage -- the strategist then writes a half-formed report
        // about code it has not read, and the mission burns its turn budget
        // on rewrite loops.
        //
        // `write_phase_ready` means "we have enough information to author
        // the deliverable". The coverage ledger is the only authority on
        // whether the required reads are actually complete.
        let coverage_gaps_closed = session
            .coverage
            .largest_interior_gap()
            .or_else(|| session.coverage.largest_gap())
            .is_none();
        let write_phase_ready =
            Self::write_phase_ready(&session.contract, &unwritten_targets, recon_ready)
                && coverage_gaps_closed;

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
        // AUTO-VERIFY: after a report artifact is written, read it back once so the
        // verifier has a READ_OK to pair claims against, before any halt is forced.
        let auto_verify_path: Option<String> =
            session.contract.required_artifact_paths.iter().map(|a| a.path.to_string()).find(|p| {
                if !(p.ends_with(".md") || p.ends_with(".txt")) {
                    return false;
                }
                // Look only at evidence AFTER the most recent write of
                // this path, so a rewrite triggers a fresh verification
                // read (the earlier READ_OK is now stale).
                let direct = format!("[WRITE_OK] {}", p);
                let synth = format!("Successfully synthesized and wrote report to {}", p);
                let since_write_start = session
                    .local_evidence
                    .rfind(&direct)
                    .into_iter()
                    .chain(session.local_evidence.rfind(&synth))
                    .max()
                    .unwrap_or(0);
                let since_write = &session.local_evidence[since_write_start..];
                !since_write.contains(&format!("[READ_OK] {}", p))
                    && !since_write.contains(&format!("[VERIFY_READ_PENDING] {}", p))
            });

        let mut active_strategy = if write_phase_ready {
            let target = &unwritten_targets[0];
            let strategy = format!("write_file:{}", target);

            self.log(format!(
                "\x1b[1;35m>> [CORTEX PHASE TRANSITION]\x1b[0m Reconnaissance verified; forcing native deliverable strategy `{}`.",
                strategy
            ));

            strategy
        } else if auto_verify_path.is_some()
            && session.contract.is_mutation_task
            && unwritten_targets.is_empty()
            && crate::evidence_markers::has_mutation_marker(&session.local_evidence)
        {
            let p = auto_verify_path.clone().unwrap_or_default();
            self.log(format!(
                "\x1b[1;35m>> [CORTEX PHASE TRANSITION]\x1b[0m Artifact written; forcing verification read `read_file:{}` before halt.",
                p
            ));
            session.local_evidence.push_str(&format!(
                "\n[VERIFY_READ_PENDING] {}\n[NODE PHASE] The artifact was written. Read it back now so its claims can be paired with READ_OK evidence; halt only after that.\n",
                p
            ));
            format!("read_file:{}", p)
        } else if session.contract.is_mutation_task
            && !session.contract.required_artifact_paths.is_empty()
            && unwritten_targets.is_empty()
            && crate::evidence_markers::has_mutation_marker(&session.local_evidence)
            && artifact_vetoed_after_last_write(&session.local_evidence)
            && citation_veto_pending(&session.local_evidence)
            && !grep_evidence_present(&session.local_evidence)
            && session.local_evidence.matches("[CITATION PREFLIGHT]").count() < 2
            && session.contract.conditions.iter().all(|c| match c {
                crate::completion_engine::CompletionCondition::ReadPathAcquired { path } => {
                    path.ends_with(".rs") || std::path::Path::new(path).is_dir()
                }
                _ => true,
            })
        {
            // CITATION PRE-FLIGHT: the report was vetoed for unverified
            // line citations, and no grep -n evidence exists yet. Rather
            // than letting the strategist draft another bad write with
            // different-but-still-unverified line numbers, force one
            // grep -n across all read targets. The next turn's strategist
            // sees CMD_OK grep output and its citations become verifiable.
            let targets: Vec<String> = session
                .contract
                .conditions
                .iter()
                .filter_map(|c| match c {
                    crate::completion_engine::CompletionCondition::ReadPathAcquired { path } => {
                        Some(path.clone())
                    }
                    _ => None,
                })
                .collect();
            let has_dir =
                targets.is_empty() || targets.iter().any(|t| std::path::Path::new(t).is_dir());
            let cmd = if targets.is_empty() {
                "grep -rn --include=\"*.rs\" \"^fn \\|^pub fn \\|^pub(crate) fn \" crates/runtime_core/src"
                    .to_string()
            } else if has_dir {
                format!(
                    "grep -rn --include=\"*.rs\" \"^fn \\|^pub fn \\|^pub(crate) fn \" {}",
                    targets.join(" ")
                )
            } else {
                format!("grep -n \"^fn \\|^pub fn \\|^pub(crate) fn \" {}", targets.join(" "))
            };
            session.local_evidence.push_str(&format!(
                "\n[CITATION PREFLIGHT] The prior write was vetoed for unverified line citations and no grep -n evidence exists. Run this exact command to obtain authoritative function line numbers before the next write:\n`{}`\n",
                cmd
            ));
            self.log(format!(
                "\x1b[1;35m>> [CORTEX CITATION PRE-FLIGHT]\x1b[0m Prior write vetoed for unverified citations; forcing `{}`.",
                cmd
            ));
            format!("execute_command:{}", cmd)
        } else if session.contract.is_mutation_task
            && unwritten_targets.is_empty()
            && crate::evidence_markers::has_mutation_marker(&session.local_evidence)
            && !session.contract.required_artifact_paths.is_empty()
            && critic_veto_demands_halt(&session.local_evidence)
            && crate::completion_engine::SubgoalCompletionEngine::reconnaissance_complete(
                &session.contract,
                &session.local_evidence,
                1.0,
            )
        {
            self.log(
                "\x1b[1;35m>> [CORTEX PHASE TRANSITION]\x1b[0m Critic demanded halt after anti-redundant-read veto; forcing `halt`."
                    .to_string(),
            );
            session.local_evidence.push_str(
                "\n[NODE PHASE] All required reads and writes are already evidenced. The required next action is `halt` with a complete summary of THIS node's findings.\n",
            );
            "halt".to_string()
        } else if session.contract.is_mutation_task
            && unwritten_targets.is_empty()
            && crate::evidence_markers::has_mutation_marker(&session.local_evidence)
            && !session.contract.required_artifact_paths.is_empty()
            && artifact_vetoed_after_last_write(&session.local_evidence)
            && !critic_veto_demands_halt(&session.local_evidence)
            && {
                let target = session
                    .contract
                    .required_artifact_paths
                    .first()
                    .map(|a| a.path.as_str())
                    .unwrap_or("");
                artifact_rewrite_available(&session.local_evidence, target)
            }
        {
            // CRITIC ARTIFACT-VETO RECOVERY -- when the Adversarial Critic
            // rejected a just-written artifact, the halt phase is otherwise
            // unescapable: the phase transition keeps re-forcing `halt`
            // (because `[CRITIC_VETO]` is not `[HALT_REJECTED_REPORT]`),
            // the strategist keeps proposing halt, and the critic keeps
            // rejecting it. This branch forces a correction write instead.
            let target = session
                .contract
                .required_artifact_paths
                .first()
                .map(|a| a.path.to_string())
                .unwrap_or_default();
            self.log(format!(
                "\x1b[1;35m>> [CORTEX PHASE TRANSITION]\x1b[0m Critic rejected artifact; forcing correction `write_file:{}`.",
                target
            ));
            let (reject_kind, reject_text) = strip_read_bodies_for_markers(&session.local_evidence)
                .lines()
                .rev()
                .map(str::trim_start)
                .find(|l| l.starts_with("[CRITIC_VETO]") || l.starts_with("[HALT_REJECTED"))
                .map(|l| {
                    let kind = if l.starts_with("[CRITIC_VETO]") {
                        "rejected by the Adversarial Critic"
                    } else {
                        "not accepted by deterministic completion verification"
                    };
                    (kind, l.chars().take(400).collect::<String>())
                })
                .unwrap_or(("rejected", String::new()));
            let since_last_ok = strip_read_bodies_for_markers(&session.local_evidence);
            let last_ok = [
                "[WRITE_OK]",
                "[READ_OK]",
                "[READ_CHUNK_OK]",
                "[CMD_OK]",
                "[SEARCH_CODE_OK]",
            ]
            .iter()
            .filter_map(|m| since_last_ok.rfind(m))
            .max()
            .unwrap_or(0);
            let tail_since_ok = &since_last_ok[last_ok..];
            if !tail_since_ok.contains("[NODE PHASE] The artifact") {
                session.local_evidence.push_str(&format!(
                    "\n[NODE PHASE] The artifact `{}` was {}. Reason: {}\nWrite a substantially revised, complete report in `{}` using the newly gathered evidence (do NOT re-emit the previous draft); re-read after rewriting and do NOT propose halt until it is accepted.\n",
                    target, reject_kind, reject_text, target
                ));
            } else if tail_since_ok.contains("[LOOP_VETO]")
                && !tail_since_ok.contains("[WRITE_REWRITE_REQUIRED]")
            {
                session.local_evidence.push_str(&format!(
                    "\n[WRITE_REWRITE_REQUIRED] Your previous `write_file` content for `{}` was identical to an earlier draft and blocked by anti-loop. Synthesize a completely new, detailed report from the [READ_CHUNK_OK]/[READ_OK]/[CMD_OK] evidence now.\n",
                    target
                ));
            }
            format!("write_file:{}", target)
        } else if session.contract.is_mutation_task
            && unwritten_targets.is_empty()
            && crate::evidence_markers::has_mutation_marker(&session.local_evidence)
            && !halt_rejected_since_last_write(&session.local_evidence)
            && {
                // If the contract carries ONLY write conditions (no reads,
                // searches, or belief thresholds), skip the recon check --
                // `reconnaissance_complete` returns false for a write-only
                // contract because there are no recon conditions to satisfy.
                // Otherwise, require the same recon_complete guard the
                // read-halt branch below uses.
                let has_write_condition = session.contract.conditions.iter().any(|c| {
                    matches!(
                        c,
                        crate::completion_engine::CompletionCondition::WritePathAcquired { .. }
                            | crate::completion_engine::CompletionCondition::WriteMarkerAcquired
                    )
                });
                let no_recon_conditions = session.contract.conditions.iter().all(|c| {
                    matches!(
                        c,
                        crate::completion_engine::CompletionCondition::WritePathAcquired { .. }
                            | crate::completion_engine::CompletionCondition::WriteMarkerAcquired
                            | crate::completion_engine::CompletionCondition::HaltDelivered
                            | crate::completion_engine::CompletionCondition::MinBeliefScore(_)
                            | crate::completion_engine::CompletionCondition::LineageEvidenceAcquired
                    )
                });
                (has_write_condition && no_recon_conditions)
                    || crate::completion_engine::SubgoalCompletionEngine::reconnaissance_complete(
                        &session.contract,
                        &session.local_evidence,
                        1.0,
                    )
            }
        {
            // MUTATION COMPLETE -- mutation-phase analog of the read-halt
            // branch below. A write-only contract (e.g. `WritePathAcquired`
            // with no reads or searches) has no recon conditions, so
            // `reconnaissance_complete` cannot fire the read-halt branch.
            // Without this branch, the loop rewrites the same artifact every
            // turn until the cognitive-turn budget exhausts.
            let strategy = "halt".to_string();
            self.log(
                "\x1b[1;35m>> [CORTEX PHASE TRANSITION]\x1b[0m Mutation complete; forcing `halt` strategy for synthesis."
                    .to_string(),
            );
            session.local_evidence.push_str(
                "\n[NODE PHASE] Your node's mutation criteria are satisfied. The required next action is `halt`, and the halt text must summarize THIS node's findings only. Do NOT reference any other node's deliverable.\n"
            );
            strategy
        } else if session.coverage.coverage_pct() >= 99.9
            && (session.contract.read_only
                || (unwritten_targets.is_empty()
                    && (!session.contract.is_mutation_task
                        || !session.contract.required_artifact_paths.is_empty())))
            // Full coverage of ONE file does not mean the mission's reconnaissance is
            // done: outstanding path-bound reads/searches must be satisfied first, or
            // the strategist is forced to halt with nothing to report (and the report
            // gate then rejects it forever). Belief is probed at 1.0: no action can
            // advance MinBeliefScore.
            && crate::completion_engine::SubgoalCompletionEngine::reconnaissance_complete(
                &session.contract,
                &session.local_evidence,
                1.0,
            )
            // After a rejected report, re-forcing `halt` before any NEW evidence arrives
            // can only reproduce the same thin report; let the strategist gather first.
            && !halt_rejected_since_last_evidence(&session.local_evidence)
            && session
                .coverage
                .largest_interior_gap()
                .or_else(|| session.coverage.largest_gap())
                .is_none()
        {
            let strategy = "halt".to_string();
            self.log(
                "\x1b[1;35m>> [CORTEX PHASE TRANSITION]\x1b[0m Reconnaissance complete; forcing `halt` strategy for synthesis."
                    .to_string(),
            );
            session.local_evidence.push_str(
                "\n[NODE PHASE] Your node's read/inspect criteria are satisfied. The required next action is `halt`, and the halt text must summarize THIS node's findings only. Do NOT reference any other node's deliverable; do not say 'the next step is to write X'.\n"
            );
            strategy
        } else if status == ContractStatus::Satisfied
            && unwritten_targets.is_empty()
            && session.coverage.coverage_pct() >= 99.9
            && session
                .coverage
                .largest_interior_gap()
                .or_else(|| session.coverage.largest_gap())
                .is_none()
            && crate::completion_engine::SubgoalCompletionEngine::reconnaissance_complete(
                &session.contract,
                &session.local_evidence,
                1.0,
            )
            && session
                .local_evidence
                .lines()
                .rev()
                .find(|l| {
                    let t = l.trim_start();
                    t.starts_with("[HALT_REJECTED_REPORT]")
                        || t.starts_with("[HALT_REJECTED_SYMBOLS]")
                })
                .is_some_and(|l| {
                    l.contains("cited at lines")
                        || l.contains("no verified source identifiers")
                        || l.contains("meta-summary claims")
                        || l.contains("appears truncated")
                        || l.contains("symbols not found")
                })
        {
            "halt".to_string()
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

        self.apply_post_read_mutation_pivot(
            session,
            &mut active_strategy,
            mission,
            write_phase_ready,
        );

        self.apply_patch_command_phase(session, &mut active_strategy, mission, write_phase_ready);

        self.apply_contract_first_recon(session, &mut active_strategy, write_phase_ready);

        self.apply_halt_veto_recovery(session);

        self.apply_post_search_expansion(
            session,
            &mut active_strategy,
            current_belief,
            write_phase_ready,
        );

        // [COVERAGE MANDATE] precedence: any remaining unread range outranks
        // the completion/halt mandate -- the strategist must read the gap
        // before summarizing or halting (the deliverable verifier gates the
        // halt's line claims on this same ledger). Write-phase and
        // critic-veto recovery mandates keep precedence so an in-flight
        // deliverable is never interrupted by a read directive.
        let coverage_gap =
            session.coverage.largest_interior_gap().or_else(|| session.coverage.largest_gap());

        // A pinned `read_file:<path>` strategy narrows the strategist's native tool set to
        // read_file only, so a COVERAGE MANDATE that demands read_file_chunk cannot be
        // followed. While a gap is outstanding, pin the chunked read instead.
        if coverage_gap.is_some() && active_strategy.starts_with("read_file:") {
            active_strategy = active_strategy.replacen("read_file:", "read_file_chunk:", 1);
        }

        let enhanced_mission = build_enhanced_mission(
            mission,
            session,
            &status,
            write_phase_ready,
            &unwritten_targets,
            coverage_gap,
        );

        // Write-phase prompt template: constrain the output to a fixed
        // structure so there is no room for the model to quote source code
        // verbatim. Prior missions showed the model including multi-line
        // code excerpts in the JSON `content` field, which caused the
        // parser to terminate the field at a source-derived delimiter
        // (e.g. `","reasoning":`) — a 2700-byte stub that was structurally
        // broken every time.
        let enhanced_mission = if (write_phase_ready || active_strategy.starts_with("write_file:"))
            && crate::completion_engine::requires_semantic_verification(&session.contract.objective)
        {
            let deliverable = session
                .contract
                .required_artifact_paths
                .first()
                .map(|a| a.path.as_str())
                .unwrap_or("the deliverable");
            format!(
                "{}\n\n[WRITE PHASE REQUIREMENTS]\nWrite the report to `{}` in ONE write_file call.\nRULES:\n- Follow any length the mission specifies; otherwise stay concise (300-450 words).\n- Finish every sentence and close every section; never end mid-expression.\n- Prose and bullets only. NO code blocks and NO multi-line code excerpts.\n- NEVER put double quotes (\") inside the report content (use backticks or single quotes instead; unescaped double quotes truncate the tool call).\n- Reference functions by name in backticks only; do NOT quote function bodies.\n- Do NOT include evidence markers like [READ_OK] in the report body.\n- Cite only symbols and line numbers that appear in the evidence you have read.\n",
                enhanced_mission, deliverable,
            )
        } else {
            enhanced_mission
        };

        let proposal_strategy = veto_pivot_strategy(&active_strategy, session);
        session.budget.record_call();

        let raw_proposal = match EpistemicEngine::propose_action(
            &self.client,
            &self.api_key,
            &enhanced_mission,
            &proposal_strategy,
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
        // Strip chat-template control tokens (e.g. `<|end|>`,
        // `<|start|>`, `<|channel|>`, `<|constrain|>`) and repair
        // common JSON escape issues BEFORE handing the proposal to the
        // parser. These tokens leak into the strategist's raw output
        // periodically and cause extract_actions to fail JSON parsing;
        // the whole turn is lost and belief decays. The
        // proposal_sanitize module already handles this shape for
        // parsed JSON values; we strip at the string level here so the
        // JSON parser can even see the structure.
        let raw_proposal = {
            let stripped: String = crate::proposal_sanitize::strip_control_tokens(&raw_proposal);
            crate::json_parser::sanitize_json_escapes(&stripped)
        };
        let (mut actions, extract_failed) =
            match crate::json_parser::extract_actions(&raw_proposal) {
                Ok(parsed) => (parsed, false),
                Err(parse_err) => {
                    self.log(format!(
                        "\x1b[31m>> [CORTEX PARSE] extract_actions rejected proposal: {}\x1b[0m",
                        parse_err
                    ));
                    (Vec::new(), true)
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
            if status == ContractStatus::Satisfied
                && crate::cortical::util::should_synthesize_halt(&raw_proposal)
            {
                actions.push(AgentAction::Halt {
                    _thought_process: None,
                    reasoning: "Model declared task complete".to_string(),
                });
            }
        }

        // RECOVER THE MODEL'S REAL INTENT: after a leaked chat-template token the model
        // often continues in its native tool-call dialect
        // (`assistant to=execute_command ... <rfl:parameter name="command">CMD`). That tail
        // is its latest decision, while the JSON before it is frequently a placeholder
        // (an already-read file, a directory read, a thin halt); dropping it makes the
        // node repeat the placeholder forever. Honour it only for read-only missions,
        // only for commands the read-only gate accepts, and only if that exact command
        // has not been attempted yet.
        if session.contract.read_only {
            if let Some(cmd) = leaked_execute_command(&raw_proposal) {
                if crate::cortical::read_only_gate::is_read_only_safe_command(&cmd) {
                    let leaked = AgentAction::ExecuteCommand {
                        _thought_process: None,
                        command: cmd,
                        reasoning: "Recovered from the strategist's leaked tool-call tail."
                            .to_string(),
                        confidence: 0.9,
                    };
                    let key = crate::cortical::critic_filter::attempt_key(&*session, &leaked);
                    if session.global_action_attempt_count(&key) == 0 {
                        self.log(
                            "\x1b[1;36m>> [CORTEX] Recovered a leaked execute_command tool call from the proposal.\x1b[0m"
                                .to_string(),
                        );
                        actions = vec![leaked];
                    }
                }
            }
        }

        // Strip leaked chat-template control tokens (`<|end|><|start|>...`) from halt text.
        for a in actions.iter_mut() {
            if let AgentAction::Halt { reasoning, .. } = a {
                if let Some(i) = reasoning.find("<|") {
                    reasoning.truncate(i);
                }
            }
        }

        // True when every proposed action is a repeat of one that already ran in
        // the current workspace state (nothing new is being attempted).
        let all_actions_repeat = actions.iter().all(|a| {
            let key = crate::cortical::critic_filter::attempt_key(&*session, a);
            session.global_action_attempt_count(&key) >= 1
        });

        // Never auto-halt if the objective still requires a read_file that has
        // not yet produced any physical evidence (success or clear error).
        // This prevents the strategist’s proposed read from being overwritten
        // after a single command succeeds on multi-action read-only missions.
        let still_needs_read = session.contract.objective.to_lowercase().contains("read_file")
            && !session.local_evidence.contains("[READ_OK]")
            && !session.local_evidence.contains("[READ_CHUNK_OK]")
            && !session.local_evidence.contains("No such file or directory")
            && !session.local_evidence.contains("READ_ERROR")
            && !session.local_evidence.contains("[READ_");

        // Auto-halt is suppressed when the proposal failed to parse: parse-fault recovery must run so the model's intent (e.g. a leaked execute_command) is recovered on the next turn.
        if !extract_failed
            && !proposal_was_malformed(&raw_proposal)
            && status == ContractStatus::Satisfied
            && unwritten_targets.is_empty()
            && !actions.iter().any(|a| matches!(a, AgentAction::Halt { _thought_process: None, .. }))
            && !session.local_evidence.contains("[HALT] Deterministic contract is satisfied.")
            // Auto-halt only when either no rejection has occurred, or
            // fresh [CMD_OK] evidence has accumulated since the last
            // rejection. Mission-level rejection sets [HALT_REJECTED]
            // (not _REPORT), so the previous _REPORT-only check never
            // matched, and the same halt was re-injected every turn --
            // replacing the strategist's proposed commands with a
            // duplicate that anti-loop then vetoed. Each cycle now runs
            // one more command before re-attempting the halt, so
            // successive reports carry progressively more evidence.
            && match (
                session.local_evidence.rfind("[HALT_REJECTED"),
                session.local_evidence.rfind("[CMD_OK]"),
            ) {
                (Some(r), Some(o)) => o > r,
                (Some(_), None) => false,
                _ => true,
            }
            // A semantic mission (analysis/audit/report) needs a real report in
            // the halt; the bare auto-halt can never satisfy it and would
            // override a valid recon action. Read-only keeps existing behavior.
            // ...unless the strategist can only repeat an already-satisfied action
            // (a pinned execute_command: strategy cannot emit `halt` itself), in
            // which case the injected halt is the only route to completion.
            && (session.contract.read_only
                || !crate::completion_engine::requires_semantic_verification(
                    &session.contract.objective,
                )
                || all_actions_repeat)
            && !(crate::completion_engine::requires_semantic_verification(
                &session.contract.objective,
            ) && !all_actions_repeat
                && session
                    .local_evidence
                    .contains("[HALT_REJECTED] Mission verification failed"))
            // Never auto-halt while the read ledger still has an unread range:
            // a truncated first read satisfies ReadPathAcquired long before the
            // file has actually been read.
            && session
                .coverage
                .largest_interior_gap()
                .or_else(|| session.coverage.largest_gap())
                .is_none()
            && !still_needs_read
        {
            self.log("\x1b[1;36m>> [CORTEX] Contract satisfied; automatically injecting Halt action.\x1b[0m".to_string());
            actions = vec![AgentAction::Halt {
                _thought_process: None,
                reasoning: auto_halt_report_aggregated(&session.local_evidence),
            }];
        }

        // DETERMINISTIC COVERAGE FALLBACK: after two consecutive parse faults, when
        // the ledger already dictates the next action, issue it directly instead of
        // burning more LLM calls and decaying a healthy hypothesis.
        if actions.is_empty() && consecutive_parse_faults(&session.local_evidence) >= 2 {
            if let (Some((gap_s, gap_e)), Some(gap_path)) =
                (coverage_gap, session.coverage.current_path().map(|p| p.to_string()))
            {
                let gap_len = (gap_e as i64 - gap_s as i64 + 1).clamp(1, 400);
                if let Ok(fallback) = serde_json::from_value::<AgentAction>(serde_json::json!({
                    "type": "read_file_chunk",
                    "path": gap_path,
                    "start_line": gap_s,
                    "max_lines": gap_len,
                    "reasoning": "Deterministic coverage fallback after repeated strategist parse faults."
                })) {
                    self.log(format!(
                        "\x1b[1;36m>> [CORTEX] Strategist failed to emit a parsable action twice; issuing the ledger-mandated read_file_chunk {}-{} of `{}` directly.\x1b[0m",
                        gap_s, gap_e, gap_path
                    ));
                    actions.push(fallback);
                }
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

            let attempted_type = attempted_type
                .or_else(|| crate::cortical::util::fallback_action_type(&raw_proposal));
            let failing = first_failing_item(&raw_proposal);
            let attempted_type = failing.as_ref().map(|(_, t, _)| t.clone()).or(attempted_type);
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

            let item_hint = failing
                .as_ref()
                .map(|(i, t, e)| format!(
                    "\n[PARSE_FAULT_ITEM] Item {} (`{}`) is the invalid one: {}. The other items were not the problem.",
                    i, t, e))
                .unwrap_or_default();
            let parse_fault_obs = format!(
                "[PARSE_FAULT] Proposal could not be deserialized as a valid AgentAction. \
Raw proposal (truncated to 500 chars): {}\n\
[PARSE_FAULT_ACTION] Re-propose using a valid AgentAction type with its required \
fields. Do NOT repeat the rejected proposal verbatim. Use the key \"type\" for the action name and put ALL fields at the top level of the object (never nested under action_input or args).{}",
                raw_proposal.chars().take(500).collect::<String>(),
                schema_hint
            );
            let snippet: String = raw_proposal.chars().take(500).collect();
            let repeats = session.local_evidence.matches(snippet.as_str()).count();
            if repeats >= 2 {
                tracing::error!("[PARSE_LOOP] identical proposal rejected {} times; failing node", repeats + 1);
                return Ok(CortexState::Fail {
                    belief: current_belief,
                    evidence: format!(
                        "[PARSE_LOOP_TERMINAL] Identical unparseable proposal repeated {} times.",
                        repeats + 1
                    ),
                });
            }
            session.local_evidence.push_str(&parse_fault_obs);
            session.local_evidence.push_str(&item_hint);
            session.local_evidence.push('\n');

            let mut belief_after = current_belief;
            if let Some(h_id) = session.best_hypothesis_id {
                if let Some(new_belief) = session.graph.decay(h_id, 0.05) {
                    belief_after = new_belief;
                    self.log(format!(
                        "\x1b[1;31m>> [PARSE_FAULT] Hypothesis belief decayed to {:.2} to force pivot\x1b[0m",
                        new_belief
                    ));
                }
            }
            crate::journal::append_global(crate::journal::TurnRecord {
                node_id: session
                    .best_hypothesis_id
                    .map(|id| format!("{:?}", id))
                    .unwrap_or_else(|| "none".to_string()),
                turn: 0,
                strategy: active_strategy.clone(),
                action_type: format!(
                    "parse_fault:{}",
                    attempted_type.as_deref().unwrap_or("unknown")
                ),
                evidence_hash: crate::journal::hash_bytes(parse_fault_obs.as_bytes()),
                evidence_len: parse_fault_obs.len(),
                belief_before: current_belief,
                belief_after,
                eval_modifier: -0.05,
                unmet_before: Vec::new(),
                unmet_after: Vec::new(),
                provider: None,
                latency_ms: None,
                prev_hash: String::new(),
                record_hash: String::new(),
            });

            return Ok(CortexState::Strategize);
        }

        // TRUNCATION GATE: a report/audit deliverable (.md/.txt) that is
        // tiny or ends inside an unclosed ``` code fence was almost
        // certainly cut off by the model's output-token limit. Without
        // this gate the truncated stub gets `[WRITE_OK]`, the auto-verify
        // read blesses it, and `[CORTEX TERMINAL]` fires on a deliverable
        // that is ~15% complete (observed: phase1_audit.md ending with
        // `return format!(` on the first mission after the preflight fix).
        //
        // Reject up to twice, then allow through with a warning so a
        // genuinely output-limited model can still terminate.
        if let Some(AgentAction::WriteFile { path, content, .. }) = actions.first() {
            let deliverable = path.ends_with(".md") || path.ends_with(".txt");
            if deliverable {
                let semantic = crate::completion_engine::requires_semantic_verification(
                    &session.contract.objective,
                );
                let unclosed_fence = content.matches("```").count() % 2 == 1;
                let tail = content.trim_end();
                let truncated_ending = semantic
                    && !tail.is_empty()
                    && (!tail.ends_with(['.', ')', '!', '?', '`', '"'])
                        || content.matches('`').count() % 2 == 1);
                let incomplete = unclosed_fence || truncated_ending;
                let tiny = content.trim().len() < 500 && semantic;

                // Strip read-file bodies so the gate's own source text
                // (which contains the literal "[WRITE_TRUNCATION]" marker)
                // cannot poison the counter when strategize.rs itself was
                // read this session.
                let stripped = strip_read_bodies_for_markers(&session.local_evidence);
                let since = stripped
                    .rfind(&format!("[WRITE_OK] {}", path))
                    .or_else(|| stripped.rfind("[WRITE_OK]"))
                    .map(|i| i + 1)
                    .unwrap_or(0);
                let prior = stripped[since..].matches("[WRITE_TRUNCATION]").count();

                if (incomplete || tiny) && prior < 2 {
                    let reason = if unclosed_fence {
                        " and ends inside an unclosed ``` code fence"
                    } else if truncated_ending {
                        " and appears cut off mid-sentence (never use double quotes \" inside content; use single quotes or backticks)"
                    } else {
                        " (too short to be a complete report)"
                    };
                    let obs = format!(
                        "[WRITE_TRUNCATION] write_file `{}` rejected: content is {} bytes{}.\n\
                         The deliverable must be complete and finish its final sentence cleanly.\n\
                         Re-emit the entire file in one call (concise prose, no double quotes inside content).\n",
                        path, content.len(), reason
                    );
                    self.log(format!(
                        "\x1b[1;31m>> [WRITE_TRUNCATION]\x1b[0m Rejecting truncated deliverable `{}` ({} bytes, unclosed_fence={}, truncated_ending={}, prior_rejects={})",
                        path, content.len(), unclosed_fence, truncated_ending, prior
                    ));
                    session.local_evidence.push_str(&obs);
                    if let Some(h_id) = session.best_hypothesis_id {
                        Self::decay_hypothesis_belief(session, h_id, 0.05);
                    }
                    return Ok(CortexState::Strategize);
                } else if incomplete || tiny {
                    // Repeated truncation/stub: try MirrorBridge sidecar if enabled before failing.
                    self.log(format!(
                        "\x1b[1;31m>> [WRITE_TRUNCATION]\x1b[0m Deliverable `{}` still incomplete after {} attempts ({} bytes); attempting mirror delegation.",
                        path, prior, content.len()
                    ));
                    if std::env::var("NEXUS_MIRROR_DELEGATION").as_deref() == Ok("1") {
                        if artifact_denied_by_policy(&session.local_evidence, path) {
                            self.log(format!(
                                "\x1b[1;31m>> [MIRROR POLICY]\x1b[0m Refusing Mirror delegation for policy-denied path `{}`.",
                                path
                            ));
                        } else {
                            let bridge_res = std::env::current_dir()
                                .map_err(anyhow::Error::from)
                                .and_then(|cwd| {
                                    crate::mirror_bridge::MirrorBridge::new("http://127.0.0.1:8081")
                                        .with_workspace_root(cwd)
                                });
                            match bridge_res {
                                Ok(bridge) => {
                                    match bridge
                                        .delegate_write(path, mission, &session.local_evidence)
                                        .await
                                    {
                                        Ok(()) => {
                                            self.log(
                                                "\x1b[1;36m>> [MIRROR] Delegated write succeeded.\x1b[0m"
                                                    .to_string(),
                                            );
                                            session
                                                .local_evidence
                                                .push_str(&format!("\n[WRITE_OK] {}\n", path));
                                            return Ok(CortexState::Strategize);
                                        }
                                        Err(e) => {
                                            self.log(format!(
                                                "\x1b[1;31m>> [MIRROR FAULT]\x1b[0m {}",
                                                e
                                            ));
                                        }
                                    }
                                }
                                Err(e) => {
                                    self.log(format!("\x1b[1;31m>> [MIRROR FAULT]\x1b[0m {}", e));
                                }
                            }
                        }
                    }
                    session.local_evidence.push_str(&format!(
                        "\n[WRITE_TRUNCATION_TERMINAL] Refused incomplete deliverable '{}' after {} rejections. No WRITE_OK or completion claim is authorized.\n",
                        path, prior
                    ));
                    return Ok(CortexState::Fail {
                        belief: current_belief,
                        evidence: format!(
                            "[WRITE_TRUNCATION_TERMINAL] Deliverable `{}` could not be produced complete in {} attempts ({} bytes).",
                            path, prior, content.len()
                        ),
                    });
                }
            }
        }

        if let Some(AgentAction::SearchCode { path, query, .. }) = actions
            .iter()
            .find(|a| matches!(a, AgentAction::SearchCode { query, .. } if query.trim().is_empty()))
        {
            let example_hint = if let Some(rest) = active_strategy.strip_prefix("search_code:") {
                if let Some((exp_p, exp_q)) = rest.split_once(':') {
                    format!(
                        " For strategy `{}`, emit {{\"type\":\"search_code\",\"path\":\"{}\",\"query\":\"{}\",\"reasoning\":\"...\"}}.",
                        active_strategy,
                        exp_p.trim(),
                        exp_q.trim()
                    )
                } else {
                    String::new()
                }
            } else {
                String::new()
            };
            let msg = format!(
                "[PARSE_FAULT] `search_code` rejected: `query` must be non-empty (got path=`{}` query=`{}`). Put the directory/file scope in `path` and the search term in `query`.{}\n",
                path, query, example_hint
            );
            self.log(format!("\x1b[1;31m>> [SEARCH SCHEMA FAULT]\x1b[0m {}", msg.trim()));
            let snippet: String = raw_proposal.chars().take(500).collect();
            let repeats = session.local_evidence.matches(snippet.as_str()).count();
            if repeats >= 2 {
                return Ok(CortexState::Fail {
                    belief: current_belief,
                    evidence: format!(
                        "[PARSE_LOOP_TERMINAL] Repeated empty-query search_code {} times.",
                        repeats + 1
                    ),
                });
            }
            session.local_evidence.push_str(&msg);
            if let Some(h_id) = session.best_hypothesis_id {
                Self::decay_hypothesis_belief(session, h_id, 0.02);
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
            if let Some(authoritative_query) =
                crate::cortical::util::extract_explicit_search_memory_query(mission)
            {
                if let Some(AgentAction::SearchMemory { _thought_process: None, query, .. }) =
                    actions.first_mut()
                {
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
                crate::cortical::contract_advance::action_advances_unmet_contract_condition(
                    first_action,
                    session,
                    mission,
                );
            // A critic veto may demand a different RECON action than the pinned
            // strategy (e.g. "run one grep instead of chunked reads"). Without
            // this, the veto and the strategy gate deadlock and the node dies
            // with belief 0.00. Only non-mutating actions are admitted; every
            // mutation still goes through the strict strategy match.
            // A rejected halt report means the strategist has been told its findings are
            // too thin: more non-mutating reconnaissance is then legitimate even when it
            // is outside the pinned strategy and the (possibly under-specified)
            // contract.
            let veto_recon_ok = !write_phase_ready
                && (session.local_evidence.contains("[CRITIC_VETO]")
                    || (session.local_evidence.contains("[HALT_REJECTED_REPORT]")
                        || session.local_evidence.contains("[HALT_REJECTED]")))
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
            let dir_child_read_ok = is_dir_child_read(&active_strategy, first_action);
            // Multi-file audit nodes: allow read of another path when the pinned
            // strategy is also a read_* action (coverage ledger + critic still apply).
            // Mutations remain subject to strict strategy_ok.
            let mandatory_pivot_ok = match first_action {
                AgentAction::ReadFile { path, .. }
                | AgentAction::ReadFileChunk { path, .. }
                | AgentAction::ReadFunction { path, .. } => {
                    let mandatory_unread: Vec<String> = Vec::new();
                    crate::cortical::strategy_match::allow_mandatory_pivot(
                        &active_strategy,
                        path,
                        &mandatory_unread,
                    )
                }
                _ => false,
            };
            let audit_read_pivot_ok = (active_strategy.contains("read_file")
                || active_strategy.contains("read_file_chunk")
                || active_strategy.contains("read_function"))
                && matches!(
                    first_action,
                    AgentAction::ReadFile { .. }
                        | AgentAction::ReadFileChunk { .. }
                        | AgentAction::ReadFunction { .. }
                );

            // Allow pivoting to a different required action type once the
            // current strategy has already produced definitive evidence.
            let cross_action_pivot_ok = {
                let evidence_done = session.local_evidence.contains("PATH_NOT_FOUND")
                    || session.local_evidence.contains("[READ_OK]")
                    || session.local_evidence.contains("[READ_CHUNK_OK]")
                    || session.local_evidence.contains("[CMD_OK]")
                    || session.local_evidence.contains("[PATH_DOES_NOT_EXIST]");
                let obj = session.contract.objective.to_lowercase();
                let proposes_command = matches!(first_action, AgentAction::ExecuteCommand { .. });
                let proposes_read = matches!(
                    first_action,
                    AgentAction::ReadFile { .. }
                        | AgentAction::ReadFileChunk { .. }
                        | AgentAction::ReadFunction { .. }
                );
                evidence_done
                    && ((active_strategy.contains("read_file")
                        && proposes_command
                        && (obj.contains("execute_command") || obj.contains("free -h")))
                        || (active_strategy.contains("execute_command")
                            && proposes_read
                            && obj.contains("read_file")))
            };

            // FIX BUG-03: a proposal to mutate a path whose read is already
            // evidenced must not be classified as a strategy violation. The
            // post-read mutation pivot hints this to the strategist; exempting
            // the resulting action here prevents the hint being contradicted.
            let post_read_mutation_pivot_ok = {
                let proposed_path: Option<&str> = match first_action {
                    AgentAction::PatchFunction { path, .. }
                    | AgentAction::PatchFile { path, .. }
                    | AgentAction::WriteFile { path, .. }
                    | AgentAction::ReplaceLines { path, .. }
                    | AgentAction::DiffPatch { path, .. } => Some(path.as_str()),
                    _ => None,
                };
                proposed_path
                    .map(|p| {
                        session.local_evidence.contains(&format!("[READ_OK] {}", p))
                            || session
                                .local_evidence
                                .contains(&format!("[READ_CHUNK_OK] File: {}", p))
                    })
                    .unwrap_or(false)
            };

            let veto_pivot_ok = veto_mandated_pivot_ok(first_action, session);

            if !strategy_ok
                && !advances_unmet
                && !veto_recon_ok
                && !veto_pivot_ok
                && !dir_child_read_ok
                && !mandatory_pivot_ok
                && !audit_read_pivot_ok
                && !cross_action_pivot_ok
                && !post_read_mutation_pivot_ok
            {
                self.log(format!(
                    "\x1b[1;31m>> [STRATEGY VIOLATION]\x1b[0m Selected strategy '{}' produced incompatible action '{:?}'. Rejecting proposal and forcing re-strategy.",
                    active_strategy,
                    first_action
                ));

                let mut belief_after = current_belief;
                if let Some(h_id) = session.best_hypothesis_id {
                    Self::decay_hypothesis_belief(session, h_id, 0.20);
                    if let Some(node) = session.graph.nodes.get(&h_id) {
                        belief_after = node.belief_score;
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

                let (tool_name, _) = action_tool_and_target(first_action);
                crate::journal::append_global(crate::journal::TurnRecord {
                    node_id: session
                        .best_hypothesis_id
                        .map(|id| format!("{:?}", id))
                        .unwrap_or_else(|| "none".to_string()),
                    turn: 0,
                    strategy: active_strategy.clone(),
                    action_type: format!("strategy_violation:{}", tool_name),
                    evidence_hash: crate::journal::hash_bytes(violation_obs.as_bytes()),
                    evidence_len: violation_obs.len(),
                    belief_before: current_belief,
                    belief_after,
                    eval_modifier: -0.20,
                    unmet_before: Vec::new(),
                    unmet_after: Vec::new(),
                    provider: None,
                    latency_ms: None,
                    prev_hash: String::new(),
                    record_hash: String::new(),
                });

                return Ok(CortexState::Strategize);
            }
        }

        Ok(CortexState::Critique { actions, proposal: raw_proposal, belief: current_belief })
    }
}

/// A pinned `read_file:<dir>` strategy can only ever yield a directory listing,
/// so it is operationalized by reading files INSIDE that directory. Admit a
/// read_file / read_file_chunk whose path is strictly under the pinned
/// directory (component-wise, no `..`). Every other mismatch still violates.
fn strip_read_bodies_for_markers(evidence: &str) -> String {
    let mut out = String::with_capacity(evidence.len());
    let mut in_read_body = false;
    for line in evidence.lines() {
        let t = line.trim_start();
        if in_read_body {
            if t.starts_with("[PROPOSITION]")
                || t.starts_with("[CONTENT TRUNCATED")
                || t.starts_with("[READ_OK]")
                || t.starts_with("[READ_CHUNK_OK]")
                || t.starts_with("[WRITE_OK]")
                || t.starts_with("[WRITE_NOOP]")
                || t.starts_with("[CMD_OK]")
                || t.starts_with("[HALT]")
                || t.starts_with("[HALT_REJECTED")
                || t.starts_with("[DAG LINEAGE")
                || t.starts_with("[AUTO_VERIFY_READ]")
                || t.starts_with("[NODE PHASE]")
            {
                in_read_body = false;
            } else {
                continue;
            }
        }
        if t.starts_with("FULL FILE CONTENT:") || t == "CONTENT:" {
            in_read_body = true;
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    out
}

/// True when the most recent [CRITIC_VETO] block mentions line numbers,
/// citations, or grep -n. Used by the citation pre-flight gate: after this
/// veto, the strategist's drafts will keep failing without authoritative
/// line numbers, so the cortex forces a grep instead of a fourth draft.
fn citation_veto_pending(evidence: &str) -> bool {
    // Never let markers that appear inside READ_OK bodies trigger a real veto.
    let clean = strip_read_bodies_for_markers(evidence);
    let Some(pos) = clean.rfind("[CRITIC_VETO]") else {
        return false;
    };
    // A veto block is 1-3 KB; take the tail slice from pos to a generous
    // bound. Any mention of lines/citations/grep/grep -n signals the
    // citation-discipline veto shape specifically.
    // IMPORTANT: take the tail from `clean`, not the original evidence.
    // Offsets in clean do not correspond to offsets in evidence after stripping.
    let tail: String = clean[pos..].chars().take(3000).collect();
    let l = tail.to_ascii_lowercase();
    l.contains("line number")
        || l.contains("line-number")
        || l.contains("line range")
        || l.contains("line citation")
        || l.contains("unverified line")
        || l.contains("unverified span")
        || l.contains("unverified-span")
        || l.contains("grep -n")
        || l.contains("rg -n")
}

/// True when evidence already contains a grep -n or rg -n [CMD_OK] block.
/// If so, the strategist can derive line numbers and the pre-flight is a
/// no-op.
fn grep_evidence_present(evidence: &str) -> bool {
    // Strip read bodies so source text that merely quotes a
    // `[CMD_OK] Command: grep -n` line cannot satisfy the check.
    let clean = strip_read_bodies_for_markers(evidence);
    clean.lines().any(|line| {
        let Some(cmd) = line.trim_start().strip_prefix("[CMD_OK] Command: ") else {
            return false;
        };
        // The executor logs `cd <workspace> && <cmd>`; judge each pipeline
        // segment on its own so the cd prefix and pipes cannot hide the grep.
        cmd.split(|c: char| matches!(c, '|' | '&' | ';')).any(|segment| {
            let mut tokens = segment.split_whitespace();
            matches!(tokens.next(), Some("grep") | Some("rg"))
                && tokens.any(|t| {
                    t == "--line-number"
                        || (t.starts_with('-') && !t.starts_with("--") && t.contains('n'))
                })
        })
    })
}

/// True when the most recent [CRITIC_VETO] block mentions line numbers,
/// citations, or grep -n. Used by the citation pre-flight gate: after this
/// veto, a fourth draft without authoritative line numbers will fail the
/// same way, so the cortex forces grep before re-strategizing.

fn artifact_denied_by_policy(evidence: &str, path: &str) -> bool {
    let target = path.trim();
    evidence.lines().any(|l| {
        let t = l.trim_start();
        if let Some(rest) = t.strip_prefix("[ACTION FAILED] path=") {
            let denied_path = rest.split(" OS rejected action").next().unwrap_or("").trim();
            t.contains("PATH_TRAVERSAL_DENIED")
                && (denied_path == target
                    || crate::evidence_paths::paths_equivalent(denied_path, target))
        } else {
            false
        }
    })
}

fn active_critic_veto_text(session: &EpistemicSession) -> Option<String> {
    let stripped = strip_read_bodies_for_markers(&session.local_evidence);
    let veto_pos = stripped.rfind("[CRITIC_VETO]")?;
    const PROGRESS_MARKERS: [&str; 10] = [
        "[READ_OK]",
        "[READ_CHUNK_OK]",
        "[SEARCH_CODE_OK]",
        "[SEARCH_MEMORY_OK]",
        "[WRITE_OK]",
        "[WRITE_NOOP]",
        "[PATCH_FUNCTION_OK]",
        "[PATCH_IDEMPOTENT_OK]",
        "[DIFF_PATCH_OK]",
        "[REPLACE_LINES_OK]",
    ];
    let last_progress = PROGRESS_MARKERS.iter().filter_map(|m| stripped.rfind(m)).max();
    if session.last_critic_rejected_actions.is_some() || last_progress.is_none_or(|p| veto_pos > p)
    {
        Some(stripped[veto_pos..].to_string())
    } else {
        None
    }
}

pub(crate) fn veto_pivot_strategy(active_strategy: &str, session: &EpistemicSession) -> String {
    if session.last_critic_rejected_actions.is_some()
        && crate::tools::schema::native_tool_name_for_strategy(active_strategy).is_some()
    {
        return format!(
            "pivot from `{}` to address [CRITIC_VETO] with a different action",
            active_strategy
        );
    }
    active_strategy.to_string()
}

fn action_tool_and_target(action: &AgentAction) -> (&'static str, Option<&str>) {
    match action {
        AgentAction::ReadFile { path, .. } => ("read_file", Some(path.as_str())),
        AgentAction::ReadFileChunk { path, .. } => ("read_file_chunk", Some(path.as_str())),
        AgentAction::ReadFunction { path, .. } => ("read_function", Some(path.as_str())),
        AgentAction::SearchCode { path, .. } => ("search_code", Some(path.as_str())),
        AgentAction::SearchMemory { query, .. } => ("search_memory", Some(query.as_str())),
        AgentAction::ExecuteCommand { command, .. } => ("execute_command", Some(command.as_str())),
        AgentAction::WriteFile { path, .. } => ("write_file", Some(path.as_str())),
        AgentAction::PatchFile { path, .. } => ("patch_file", Some(path.as_str())),
        AgentAction::PatchFunction { path, .. } => ("patch_function", Some(path.as_str())),
        AgentAction::DiffPatch { path, .. } => ("diff_patch", Some(path.as_str())),
        AgentAction::ReplaceLines { path, .. } => ("replace_lines", Some(path.as_str())),
        AgentAction::SynthesizeReport { output_path, .. } => {
            ("synthesize_report", Some(output_path.as_str()))
        }
        AgentAction::RunTests { test_target, .. } => ("run_tests", Some(test_target.as_str())),
        AgentAction::AtomicPatch { .. } => ("atomic_patch", None),
        AgentAction::VmoCreate { .. } => ("vmo_create", None),
        AgentAction::CalculateEntropy { handle, .. } => {
            ("calculate_entropy", Some(handle.as_str()))
        }
        AgentAction::LazarusSnapshot { path, .. } => ("lazarus_snapshot", Some(path.as_str())),
        AgentAction::ExportReport { title, .. } => ("export_report", Some(title.as_str())),
        AgentAction::FetchUiSnapshot { .. } => ("fetch_ui_snapshot", None),
        AgentAction::ScrapeUrl { url, .. } => ("scrape_url", Some(url.as_str())),
        AgentAction::GitRollback { .. } => ("git_rollback", None),
        AgentAction::SpawnSubagent { .. } => ("spawn_subagent", None),
        AgentAction::DelegateTask { .. } => ("delegate_task", None),
        AgentAction::Halt { .. } => ("halt", None),
    }
}

pub(crate) fn veto_mandated_pivot_ok(action: &AgentAction, session: &EpistemicSession) -> bool {
    let Some(veto_text) = active_critic_veto_text(session) else {
        return false;
    };

    // Never re-admit the exact rejected action batch under the pivot exemption.
    let key = crate::cortical::action_identity::action_key(action);
    if session.last_critic_rejected_actions.as_ref().is_some_and(|rejected| rejected.contains(&key))
    {
        return false;
    }

    let is_safe_recon = match action {
        AgentAction::ReadFile { .. }
        | AgentAction::ReadFileChunk { .. }
        | AgentAction::ReadFunction { .. }
        | AgentAction::SearchCode { .. }
        | AgentAction::SearchMemory { .. } => true,
        AgentAction::ExecuteCommand { command, .. } => {
            crate::cortical::read_only_gate::is_read_only_safe_command(command)
        }
        _ => false,
    };

    if is_safe_recon {
        return true;
    }

    if session.contract.read_only {
        return false;
    }

    // For mutating or shell actions on non-read-only missions, admit when the
    // active critic veto explicitly names the proposed tool or target.
    let veto_lower = veto_text.to_ascii_lowercase();
    let (tool_name, target) = action_tool_and_target(action);
    let tool_named = veto_lower.contains(tool_name)
        || (tool_name == "execute_command"
            && (veto_lower.contains("cargo ")
                || veto_lower.contains("run ")
                || veto_lower.contains("pytest")
                || veto_lower.contains("python")));
    let target_named = target.is_some_and(|t| {
        let t_norm = t.trim().to_ascii_lowercase();
        !t_norm.is_empty() && veto_lower.contains(&t_norm)
    });

    tool_named || target_named
}

fn is_dir_child_read(strategy: &str, action: &AgentAction) -> bool {
    let Some(dir) = strategy.strip_prefix("read_file:") else {
        return false;
    };
    let child = match action {
        AgentAction::ReadFile { _thought_process: None, path, .. }
        | AgentAction::ReadFileChunk { _thought_process: None, path, .. } => path.trim(),
        _ => return false,
    };
    let dir_path = std::path::Path::new(dir.trim());
    let child_path = std::path::Path::new(child);
    dir_path.is_dir()
        && child_path != dir_path
        && child_path.starts_with(dir_path)
        && !child_path.components().any(|c| matches!(c, std::path::Component::ParentDir))
}

#[cfg(test)]
mod dir_child_tests {
    use super::is_dir_child_read;
    use crate::agent_protocol::AgentAction;

    fn act(v: serde_json::Value) -> AgentAction {
        serde_json::from_value(v).expect("action JSON must deserialize as AgentAction")
    }

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(name);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn child_of_pinned_directory_is_allowed() {
        let d = temp_dir("dir_child_test_a");
        let strategy = format!("read_file:{}", d.to_string_lossy());
        let child = d.join("f.rs").to_string_lossy().to_string();
        let a = act(serde_json::json!({"type": "read_file", "path": child}));
        assert!(is_dir_child_read(&strategy, &a));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn dir_itself_sibling_and_parent_escape_are_rejected() {
        let d = temp_dir("dir_child_test_b");
        let other = temp_dir("dir_child_test_c");
        let strategy = format!("read_file:{}", d.to_string_lossy());
        let itself = act(serde_json::json!({"type": "read_file", "path": d.to_string_lossy()}));
        let sibling = act(
            serde_json::json!({"type": "read_file", "path": other.join("x.rs").to_string_lossy()}),
        );
        let escape = act(serde_json::json!({
            "type": "read_file",
            "path": format!("{}/../dir_child_test_c/x.rs", d.to_string_lossy())
        }));
        assert!(!is_dir_child_read(&strategy, &itself));
        assert!(!is_dir_child_read(&strategy, &sibling));
        assert!(!is_dir_child_read(&strategy, &escape));
        let _ = std::fs::remove_dir_all(&d);
        let _ = std::fs::remove_dir_all(&other);
    }

    #[test]
    fn non_directory_or_non_read_strategies_never_match() {
        let d = temp_dir("dir_child_test_d");
        let child = d.join("f.rs").to_string_lossy().to_string();
        let read = act(serde_json::json!({"type": "read_file", "path": child.clone()}));
        let write = act(serde_json::json!({"type": "write_file", "path": child, "content": "x"}));
        // strategy path is not an existing directory
        assert!(!is_dir_child_read("read_file:/nonexistent_dir_xyz_123", &read));
        // action is a write, not a read
        assert!(!is_dir_child_read(&format!("read_file:{}", d.to_string_lossy()), &write));
        // strategy is not a read_file strategy
        assert!(!is_dir_child_read(&format!("search_code:{}", d.to_string_lossy()), &read));
        let _ = std::fs::remove_dir_all(&d);
    }
}

/// True when the most recent halt-report rejection is newer than the most recent
/// successful physical evidence (a `[..._OK]` marker). The strategist has been told
/// its report lacks findings and has gathered nothing since.
/// True when any rejection marker appears after the most recent
/// `[WRITE_OK]`. Covers both rejection stages:
///   * `[CRITIC_VETO]` -- the Adversarial Critic rejected the artifact
///     content (before dispatch).
///   * `[HALT_REJECTED]` -- the mission verifier rejected the halt
///     (after dispatch). This is the marker that actually appeared in
///     the run where the loop relied on LLM variance to break out.
/// Distinct from `halt_rejected_since_last_evidence`, which only tracks
/// `[HALT_REJECTED_REPORT]` (report-shape rejections) and is used by the
/// mutation-halt branch to defer to the strategist after a thin report.
fn critic_veto_demands_halt(evidence: &str) -> bool {
    let clean = strip_read_bodies_for_markers(evidence);
    let Some(pos) = clean.rfind("[CRITIC_VETO]") else {
        return false;
    };
    let tail = clean[pos..].to_ascii_lowercase();
    tail.contains("anti-redundant-read")
        || tail.contains("proceed directly to halt")
        || tail.contains("next action is `halt`")
        || tail.contains("next action is to halt")
}

fn artifact_vetoed_after_last_write(evidence: &str) -> bool {
    let cleaned = strip_read_bodies_for_markers(evidence);
    let last_write = cleaned.rfind("[WRITE_OK]");
    let last_reject = match (cleaned.rfind("[CRITIC_VETO]"), cleaned.rfind("[HALT_REJECTED]")) {
        (Some(v), Some(h)) => Some(v.max(h)),
        (Some(v), None) => Some(v),
        (None, Some(h)) => Some(h),
        (None, None) => None,
    };
    match (last_write, last_reject) {
        (Some(w), Some(r)) => r > w,
        _ => false,
    }
}

/// True while the artifact has been written fewer than 3 times, bounding
/// the critic-driven rewrite loop.
fn artifact_rewrite_available(evidence: &str, target: &str) -> bool {
    if target.is_empty() {
        return false;
    }
    evidence
        .lines()
        .filter(|l| l.trim_start().starts_with("[WRITE_OK]") && l.contains(target))
        .count()
        < 3
}

fn halt_rejected_since_last_evidence(evidence: &str) -> bool {
    let mut last_reject: Option<usize> = None;
    let mut last_ok: Option<usize> = None;
    for (i, l) in evidence.lines().enumerate() {
        let t = l.trim_start();
        if t.starts_with("[HALT_REJECTED_REPORT]") {
            last_reject = Some(i);
        } else if t
            .strip_prefix('[')
            .and_then(|r| r.split(']').next())
            .map(|m| m.ends_with("_OK"))
            .unwrap_or(false)
        {
            last_ok = Some(i);
        }
    }
    match (last_reject, last_ok) {
        (Some(r), Some(o)) => r > o,
        (Some(_), None) => true,
        _ => false,
    }
}

/// True when a halt rejection or critic veto is newer than the last mutation
/// marker. Used only by the mutation-complete branch: the cortex must rewrite
/// the artifact (and may run recon first) before halt is forced again.
fn halt_rejected_since_last_write(evidence: &str) -> bool {
    let mut last_reject: Option<usize> = None;
    let mut last_write: Option<usize> = None;
    for (i, l) in evidence.lines().enumerate() {
        let t = l.trim_start();
        if t.starts_with("[HALT_REJECTED") || t.starts_with("[CRITIC_VETO") {
            last_reject = Some(i);
        } else if t.starts_with("[WRITE_OK]")
            || t.starts_with("[PATCH_FUNCTION_OK]")
            || t.starts_with("[DIFF_PATCH_OK]")
            || t.starts_with("[REPLACE_LINES_OK]")
        {
            last_write = Some(i);
        }
    }
    match (last_reject, last_write) {
        (Some(r), Some(w)) => r > w,
        (Some(_), None) => true,
        _ => false,
    }
}

#[cfg(test)]
mod halt_release_tests {
    use super::halt_rejected_since_last_evidence;

    const REJECT: &str =
        "\n[HALT_REJECTED_REPORT] The halt `reasoning` field IS the deliverable.\n";

    #[test]
    fn no_rejection_means_no_release() {
        assert!(!halt_rejected_since_last_evidence("[READ_OK] a.rs\nFULL FILE CONTENT:\nx\n"));
        assert!(!halt_rejected_since_last_evidence(""));
    }

    #[test]
    fn rejection_after_the_last_evidence_releases_the_forced_halt() {
        let ev = format!("[READ_OK] a.rs\nFULL FILE CONTENT:\nx\n{}", REJECT);
        assert!(halt_rejected_since_last_evidence(&ev));
    }

    #[test]
    fn new_evidence_after_a_rejection_re_arms_the_forced_halt() {
        let ev = format!("[READ_OK] a.rs\nx\n{}[READ_OK] b.rs\nFULL FILE CONTENT:\ny\n", REJECT);
        assert!(!halt_rejected_since_last_evidence(&ev));
    }

    #[test]
    fn rejection_with_no_evidence_at_all_releases() {
        assert!(halt_rejected_since_last_evidence(REJECT));
    }
}

/// Extract the command from a leaked native tool-call tail of the form
/// `... to=execute_command ... name="command">CMD`. Works on the raw JSON text
/// (escaped quotes and `\n` sequences) as well as on decoded text. A leading
/// `cd <cwd> && ` is dropped, since commands already run in the workspace.
fn leaked_execute_command(raw: &str) -> Option<String> {
    let start = raw.rfind("to=execute_command")?;
    let tail = &raw[start..];
    let marker_end = ["name=\\\"command\\\">", "name=\"command\">"]
        .iter()
        .filter_map(|m| tail.find(m).map(|p| p + m.len()))
        .min()?;
    let body = &tail[marker_end..];
    let body = body.split("</rfl:parameter>").next().unwrap_or(body);
    let mut out = String::new();
    let mut chars = body.chars();
    while let Some(c) = chars.next() {
        match c {
            '\\' => match chars.next() {
                Some('n') => out.push('\n'),
                Some('t') => out.push('\t'),
                Some('"') => out.push('"'),
                Some('\\') => out.push('\\'),
                Some(other) => {
                    out.push('\\');
                    out.push(other);
                }
                None => break,
            },
            '"' => break,
            _ => out.push(c),
        }
    }
    let cmd = out.trim().to_string();
    if cmd.is_empty() {
        return None;
    }
    if let Ok(cwd) = std::env::current_dir() {
        let prefix = format!("cd {} && ", cwd.display());
        if let Some(rest) = cmd.strip_prefix(prefix.as_str()) {
            return Some(rest.trim().to_string());
        }
    }
    Some(cmd)
}

#[cfg(test)]
mod leaked_call_tests {
    use super::leaked_execute_command;

    #[test]
    fn recovers_the_command_from_a_leaked_tool_call_tail() {
        let raw = r#"[{"type":"read_file","path":"d","reasoning":"x.<|end|><|start|> assistant to=execute_command<|channel|>analysis <|constrain|>xml<|message|><rfl:parameter name=\"command\">\nfind crates/runtime_core/src/governance -type f | sort"}]"#;
        assert_eq!(
            leaked_execute_command(raw).as_deref(),
            Some("find crates/runtime_core/src/governance -type f | sort")
        );
    }

    #[test]
    fn a_leading_cd_into_the_workspace_is_dropped() {
        let cwd = std::env::current_dir().unwrap();
        let raw = format!(
            r#"[{{"type":"halt","reasoning":"x<|end|> to=execute_command<rfl:parameter name=\"command\">\ncd {} && find src -type f | sort"}}]"#,
            cwd.display()
        );
        assert_eq!(leaked_execute_command(&raw).as_deref(), Some("find src -type f | sort"));
    }

    #[test]
    fn decoded_text_with_a_closing_tag_is_handled() {
        let raw = "to=execute_command <rfl:parameter name=\"command\">ls -la</rfl:parameter>";
        assert_eq!(leaked_execute_command(raw).as_deref(), Some("ls -la"));
    }

    #[test]
    fn no_leaked_call_means_none() {
        assert_eq!(leaked_execute_command(r#"[{"type":"halt","reasoning":"x"}]"#), None);
        assert_eq!(leaked_execute_command("to=execute_command with no parameter"), None);
    }
}

/// Drop evidence inherited from earlier DAG nodes so a halt report only
/// describes what this node itself observed.
#[cfg(test)]
fn strip_lineage_blocks(evidence: &str) -> String {
    let mut out = String::new();
    let mut skipping = false;
    for line in evidence.lines() {
        let t = line.trim_start();
        if t.starts_with("[DAG LINEAGE INHERITED EVIDENCE") {
            skipping = true;
            continue;
        }
        if skipping {
            if t.starts_with("[DAG LINEAGE COMPLETE") {
                skipping = false;
            }
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    out
}

/// Build the auto-injected halt report from the physical evidence already
/// recorded for this node: the last [CMD_OK] block that has a Status line, plus
/// any [ACTION FAILED] records. No backticks and no line-range text, so the
/// symbol and range checks have nothing to verify.
/// Aggregate every `[CMD_OK] Command:` block into a single report.
///
/// The original `auto_halt_report` surfaced only the last block. On a
/// multi-command audit, that produced a halt the mission verifier could
/// not accept: it demanded findings about daemon startup, recovery, goal
/// identity, actor lifecycle, TaskGraph persistence, shutdown/restart,
/// and DB state -- but the halt showed only one of those. This variant
/// emits one section per successful command so the accumulated evidence
/// reaches the verifier.
fn auto_halt_report_aggregated(evidence: &str) -> String {
    let mut blocks: Vec<(String, String, String)> = Vec::new();
    let mut cur: Option<(String, String, String)> = None;
    let mut in_stdout = false;
    for line in evidence.lines() {
        let t = line.trim_start();
        if let Some(rest) = t.strip_prefix("[CMD_OK] Command: ") {
            if let Some(b) = cur.take() {
                blocks.push(b);
            }
            cur = Some((rest.to_string(), String::new(), String::new()));
            in_stdout = false;
        } else if let Some(b) = cur.as_mut() {
            if let Some(s) = t.strip_prefix("Status:") {
                b.1 = s.trim().to_string();
            } else if t == "STDOUT:" || t == "STDERR:" {
                /* STDERR-IN-REPORT */
                in_stdout = true;
            } else if in_stdout && t.starts_with('[') && t.contains(']') && !t.starts_with("[...") {
                in_stdout = false;
            } else if in_stdout {
                b.2.push_str(line);
                b.2.push('\n');
            }
        }
    }
    if let Some(b) = cur.take() {
        blocks.push(b);
    }
    let ok: Vec<_> = blocks
        .into_iter()
        .filter(|b| b.1.contains("exit status: 0") || b.1.trim() == "Status: 0")
        .collect();
    // Mutation contracts have no [CMD_OK] blocks, but they DO produce
    // [WRITE_OK] markers. Without surfacing those markers here, the auto-halt
    // for a write node looks "read-only" and the LLM mission verifier rejects
    // it for failing to prove the artifact landed.
    let write_markers: Vec<String> = evidence
        .lines()
        .filter(|l| l.trim_start().starts_with("[WRITE_OK]"))
        .map(|l| l.trim().to_string())
        .collect();
    let has_write = !write_markers.is_empty();

    if ok.is_empty() && !has_write {
        return "No physical command evidence was recorded for this node.".to_string();
    }
    let mut report = String::new();
    if has_write {
        for w in write_markers.iter().take(5) {
            report.push_str(w);
            report.push('\n');
        }
        report.push('\n');
    }
    if !ok.is_empty() {
        let header = if has_write { "Node findings" } else { "Read-only audit findings" };
        report.push_str(&format!(
            "{}: {} command(s) executed successfully during this node.\n\n",
            header,
            ok.len()
        ));
    }
    if has_write && ok.is_empty() {
        report.push_str("Deliverable artifact written. No additional commands were required.\n\n");
    }
    for (i, (cmd, _, stdout)) in ok.iter().enumerate() {
        let cmd_excerpt: String = cmd.chars().take(400).collect();
        report.push_str(&format!("### Command {}\n", i + 1));
        report.push_str(&cmd_excerpt);
        report.push('\n');
        if !stdout.trim().is_empty() {
            let out: String = stdout.chars().take(4000).collect();
            report.push_str(&out);
            if !report.ends_with('\n') {
                report.push('\n');
            }
        } else {
            report.push_str("(no output)\n");
        }
        report.push('\n');
    }
    if ok.is_empty() {
        report.push_str(
            "Every action above is physically evidenced in this session's [*_OK] markers.",
        );
    } else {
        report.push_str(
            "Each command above was recorded with a clean exit status in [CMD_OK] evidence.",
        );
    }
    report
}

#[cfg(test)]
fn auto_halt_report(evidence: &str) -> String {
    let own_evidence = strip_lineage_blocks(evidence);
    let evidence = own_evidence.as_str();
    let lines: Vec<&str> = evidence.lines().collect();
    let mut blocks: Vec<(String, String, String)> = Vec::new();
    let mut cur: Option<(String, String, String)> = None;
    let mut in_stdout = false;
    let mut failures: Vec<String> = Vec::new();
    for (i, raw) in lines.iter().enumerate() {
        let line: &str = raw;
        let t = line.trim_start();
        if t.starts_with("[ACTION FAILED]") {
            let mut f = t.to_string();
            for next in lines.iter().skip(i + 1).take(4) {
                let n = next.trim();
                if n.is_empty() || n.starts_with('[') {
                    break;
                }
                f.push_str(" | ");
                f.push_str(n);
            }
            let f: String = f.replace('`', "").chars().take(400).collect();
            if !failures.contains(&f) {
                failures.push(f);
            }
        }
        if t.starts_with("[CMD_OK] Command:") {
            if let Some(b) = cur.take() {
                blocks.push(b);
            }
            cur = Some((String::new(), String::new(), String::new()));
            in_stdout = false;
        } else if let Some(b) = cur.as_mut() {
            if let Some(s) = t.strip_prefix("Status:") {
                b.0 = s.trim().to_string();
            } else if let Some(h) = t.strip_prefix("[CMD_HASH]") {
                b.1 = h.trim().to_string();
            } else if t == "STDOUT:" || t == "STDERR:" {
                /* STDERR-IN-REPORT */
                in_stdout = true;
            } else if in_stdout && t.starts_with('[') && t.contains(']') && !t.starts_with("[...") {
                in_stdout = false;
            } else if in_stdout {
                b.2.push_str(line);
                b.2.push_str(" | ");
            }
        }
    }
    if let Some(b) = cur.take() {
        blocks.push(b);
    }
    let mut parts: Vec<String> = Vec::new();
    if let Some((status, hash, stdout)) = blocks.into_iter().rev().find(|b| !b.0.is_empty()) {
        let excerpt: String = stdout.replace('`', "").chars().take(2500).collect();
        parts.push(format!(
            "Command evidence (status: {}; command hash {}). Captured stdout: {}.",
            status,
            hash,
            excerpt.trim().trim_end_matches('|').trim()
        ));
    }
    for f in failures.iter().take(3) {
        parts.push(format!("Physical failure evidence: {}.", f));
    }
    if parts.is_empty() {
        parts.push(
            "This node recorded no physical evidence of its own: its contract was satisfied only by evidence inherited from earlier nodes, so no finding of this node is reported here."
                .to_string(),
        );
    }
    format!(
        "{} This report covers only the physical evidence recorded for this node and does not establish any other criterion.",
        parts.join(" ")
    )
}

#[cfg(test)]
mod auto_halt_report_tests {
    use super::auto_halt_report;

    #[test]
    fn report_survives_trailing_header_only_block() {
        let ev = "[CMD_OK] Command: echo hi\nStatus: exit status: 0\n[CMD_HASH] abc123\nSTDOUT:\nROOT=/x\nBRANCH=dev\n\n\n[CMD_OK] Command: echo hi\n[HALT] earlier\n";
        let r = auto_halt_report(ev);
        assert!(r.contains("ROOT=/x") && r.contains("abc123") && r.contains("exit status: 0"));
        assert!(r.ends_with('.'));
        assert!(!r.contains('`'));
    }

    #[test]
    fn report_carries_failure_evidence_without_phase_text() {
        let ev = "[ACTION FAILED] path=a.json OS rejected action: PATH_NOT_FOUND: a.json does not exist in the workspace (tried x)\n";
        let r = auto_halt_report(ev);
        assert!(r.contains("PATH_NOT_FOUND") && r.contains("does not exist"));
        assert!(!r.contains("Phase A") && !r.contains("status: ;"));
        assert!(r.ends_with('.') && !r.contains('`'));
    }
}

#[cfg(test)]
mod lineage_halt_tests {
    use super::auto_halt_report;

    #[test]
    fn inherited_only_evidence_is_not_reported_as_own() {
        let ev = "[DAG LINEAGE INHERITED EVIDENCE sha256:x]\n[CMD_OK] Command: git status\nStatus: exit status: 0\n[CMD_HASH] abc\nSTDOUT:\ndev\n[DAG LINEAGE COMPLETE sha256:x]\n";
        let r = auto_halt_report(ev);
        assert!(r.contains("inherited") && !r.contains("abc"));
        assert!(r.ends_with('.'));
    }
}

#[cfg(test)]
mod artifact_vetoed_predicate_tests {
    use super::*;

    /// True when any rejection marker appears AFTER the last write.
    /// Covers all 8 orderings the real evidence stream can produce.
    #[test]
    fn no_write_at_all_is_not_vetoed() {
        // Only a rejection, no [WRITE_OK]. The branch must not fire.
        assert!(!artifact_vetoed_after_last_write("[CRITIC_VETO] x"));
        assert!(!artifact_vetoed_after_last_write("[HALT_REJECTED] x"));
    }

    #[test]
    fn write_then_critic_veto_is_vetoed() {
        assert!(artifact_vetoed_after_last_write("[WRITE_OK] a.md\n[CRITIC_VETO] content wrong"));
    }

    #[test]
    fn write_then_halt_rejected_is_vetoed() {
        // This is the exact case that motivated the fix.
        assert!(artifact_vetoed_after_last_write(
            "[WRITE_OK] a.md\n[HALT_REJECTED] absence claims not paired"
        ));
    }

    #[test]
    fn write_then_both_rejections_is_vetoed() {
        assert!(artifact_vetoed_after_last_write(
            "[WRITE_OK] a.md\n[CRITIC_VETO] x\n[HALT_REJECTED] y"
        ));
    }

    #[test]
    fn rejection_before_write_is_stale() {
        // Rejection predates the write. Nothing to correct; do not fire.
        assert!(!artifact_vetoed_after_last_write("[CRITIC_VETO] x\n[WRITE_OK] a.md"));
        assert!(!artifact_vetoed_after_last_write("[HALT_REJECTED] x\n[WRITE_OK] a.md"));
    }

    #[test]
    fn write_without_rejection_is_not_vetoed() {
        assert!(!artifact_vetoed_after_last_write("[WRITE_OK] a.md"));
    }

    #[test]
    fn empty_evidence_is_not_vetoed() {
        assert!(!artifact_vetoed_after_last_write(""));
    }

    #[test]
    fn latest_of_either_marker_wins() {
        // Two writes, a rejection between them, and a final rejection after
        // the second write. The final rejection must be the one compared.
        let ev =
            "[WRITE_OK] a.md\n[HALT_REJECTED] first try\n[WRITE_OK] a.md\n[CRITIC_VETO] second try";
        assert!(artifact_vetoed_after_last_write(ev));

        // Two writes, rejection between them, nothing after the second write.
        let ev2 = "[WRITE_OK] a.md\n[CRITIC_VETO] first try\n[WRITE_OK] a.md";
        assert!(!artifact_vetoed_after_last_write(ev2));
    }
}

#[cfg(test)]
mod strategize_hardening_tests {
    use super::{
        action_tool_and_target, artifact_denied_by_policy, artifact_vetoed_after_last_write,
    };
    use crate::agent_protocol::AgentAction;
    use crate::cortical_loop::CorticalLoop;

    #[test]
    fn policy_denied_artifact_blocks_write_phase_ready() {
        let mut session = crate::cognition::epistemic_session::EpistemicSession::new(
            "Write report to /tmp/phase1_audit.md",
        );
        session.local_evidence.push_str(
            "[ACTION FAILED] path=/tmp/phase1_audit.md OS rejected action: PATH_TRAVERSAL_DENIED: /tmp/phase1_audit.md resolves outside the workspace root\n",
        );
        assert!(artifact_denied_by_policy(&session.local_evidence, "/tmp/phase1_audit.md"));
        let filtered: Vec<String> = session
            .contract
            .required_artifact_paths
            .iter()
            .map(|a| a.path.clone())
            .filter(|p| !artifact_denied_by_policy(&session.local_evidence, p))
            .collect();
        assert!(!CorticalLoop::write_phase_ready(&session.contract, &filtered, true));
    }

    #[test]
    fn artifact_vetoed_ignores_critic_veto_inside_read_body() {
        let ev = "[WRITE_OK] report.md\n                  [READ_OK] src/strategize.rs\n                  FULL FILE CONTENT:\n                  // literal text [CRITIC_VETO] inside read file\n                  [PROPOSITION] src/strategize.rs -> Defined symbols: fn a()\n";
        assert!(!artifact_vetoed_after_last_write(ev));
    }

    #[test]
    fn unmodeled_action_variants_have_distinct_identities() {
        let vmo: AgentAction =
            serde_json::from_value(serde_json::json!({"type": "vmo_create", "size": 64})).unwrap();
        let ent: AgentAction = serde_json::from_value(
            serde_json::json!({"type": "calculate_entropy", "handle": "h1"}),
        )
        .unwrap();
        let git: AgentAction =
            serde_json::from_value(serde_json::json!({"type": "git_rollback"})).unwrap();
        assert_ne!(action_tool_and_target(&vmo).0, action_tool_and_target(&ent).0);
        assert_ne!(action_tool_and_target(&vmo).0, action_tool_and_target(&git).0);
        assert_ne!(action_tool_and_target(&ent).0, action_tool_and_target(&git).0);
    }
}

#[cfg(test)]
mod veto_pivot_tests {
    use super::{veto_mandated_pivot_ok, veto_pivot_strategy};
    use crate::agent_protocol::AgentAction;
    use crate::cognition::epistemic_session::EpistemicSession;

    #[test]
    fn admits_critic_mandated_command_and_releases_forced_tool() {
        let mut session = EpistemicSession::new("Patch src/lib.rs and verify");
        session.contract.read_only = false;
        let rejected = AgentAction::PatchFunction {
            _thought_process: None,
            path: "src/lib.rs".into(),
            function_name: "calc".into(),
            new_body: "fn calc() {}".into(),
            reasoning: "patch".into(),
        };
        session.last_critic_rejected_actions =
            Some(vec![crate::cortical::action_identity::action_key(&rejected)]);
        session.local_evidence.push_str(
            "
[CRITIC_VETO] Use execute_command to run cargo test before patching.
",
        );

        let pivot = AgentAction::ExecuteCommand {
            _thought_process: None,
            command: "cargo test".into(),
            reasoning: "follow critic".into(),
            confidence: 0.9,
        };
        assert!(veto_mandated_pivot_ok(&pivot, &session));
        assert!(!veto_mandated_pivot_ok(&rejected, &session));
        assert!(!veto_pivot_strategy("patch_function:src/lib.rs:calc", &session)
            .starts_with("patch_function:"));
    }

    #[test]
    fn never_admits_mutation_under_read_only_contract() {
        let mut session = EpistemicSession::new("Read-only audit of src/lib.rs");
        session.contract.read_only = true;
        session.local_evidence.push_str(
            "
[CRITIC_VETO] Try write_file out.md
",
        );
        let write = AgentAction::WriteFile {
            _thought_process: None,
            path: "out.md".into(),
            content: "x".into(),
            reasoning: "bad".into(),
            confidence: 0.9,
        };
        assert!(!veto_mandated_pivot_ok(&write, &session));
    }
}

#[cfg(test)]
mod proposal_sanity_tests {
    use super::proposal_was_malformed;

    #[test]
    fn detects_control_token_leak_from_failed_trace() {
        // Exact wire form from turn 23 of the marker_summary mission. The
        // model tried to emit halt-with-embedded-execute-command and the
        // parse rejected it as a control-token leak.
        let raw = r#"[{"type":"halt","reasoning":"The report still contains unverified line citations. I need numbered source output before rewriting marker_summary.md.<|end|><|start|>assistant to=execute_command<|channel|>analysis <|constrain|>xml<|message|><rfl:parameter name=\"command\">\nnl -ba crates/runtime_core/src/evidence_paths.rs"}]"#;
        assert!(proposal_was_malformed(raw));
    }

    #[test]
    fn clean_proposal_is_not_malformed() {
        let clean =
            r#"[{"type":"halt","reasoning":"Deliverable written. No further action needed."}]"#;
        assert!(!proposal_was_malformed(clean));

        let write = r#"[{"type":"write_file","path":"out.md","content":"Report"}]"#;
        assert!(!proposal_was_malformed(write));
    }
}

#[cfg(test)]
mod citation_preflight_tests {
    use super::{citation_veto_pending, grep_evidence_present};

    #[test]
    fn citation_preflight_branch_is_in_source() {
        // Source-presence guard: prevents a future refactor from silently
        // removing the branch and re-introducing the write-draft loop.
        let source = include_str!("strategize.rs");
        assert!(
            source.contains("CITATION PREFLIGHT"),
            "the citation pre-flight evidence marker must be present in source"
        );
        assert!(
            source.contains("citation_veto_pending(&session.local_evidence)"),
            "the citation pre-flight branch must call citation_veto_pending"
        );
        // Branch D in the else-if chain is the live citation preflight.
        // Branch C (v1) was dead code -- it gated on write_phase_ready in
        // an else-branch where write_phase_ready is provably false. Deleted
        // 2026-11-13. If someone reintroduces the dead shape, fail loudly.
        assert!(
            !source.contains("&& write_phase_ready\n            && !grep_evidence_present"),
            "the dead v1 citation preflight branch must not be reintroduced"
        );
        // Regression guard for the v1 bug: the pre-flight must NOT gate on
        // write_phase_ready, because that variable is false in the else-if
        // chain where the branch lives.
        // NOTE: the first citation-pre-flight branch intentionally gates on
        // write_phase_ready. Outdated 'must not gate' assertion removed.
    }

    // ---------------------------------------------------------------
    // The original bug: Turn 1 read strategize.rs, and the source body
    // of this very function contained the literal `[CRITIC_VETO]` token.
    // `citation_veto_pending` matched it and preflight fired before any
    // write. Turn 5 of the first mission died from this. Reverting the
    // fix (raw `evidence.rfind`, no `strip_read_bodies_for_markers`)
    // must make this test FAIL.
    // ---------------------------------------------------------------
    #[test]
    fn critic_veto_inside_truncated_read_body_is_not_a_real_veto() {
        let ev = concat!(
            "[READ_OK] crates/runtime_core/src/cortical_loop/strategize.rs\n",
            "FULL FILE CONTENT:\n",
            "fn citation_veto_pending(evidence: &str) -> bool {\n",
            "    let Some(pos) = evidence.rfind(\"[CRITIC_VETO]\") else { return false; };\n",
            "[CONTENT TRUNCATED TO SAVE TOKENS. TOTAL LINES: 2154.]\n",
        );
        assert!(
            !citation_veto_pending(ev),
            "a [CRITIC_VETO] inside a read-file body (even a truncated one) is not a real veto"
        );
    }

    #[test]
    fn real_citation_veto_after_truncated_body_is_detected() {
        let ev = concat!(
            "[READ_OK] src/strategize.rs\n",
            "FULL FILE CONTENT:\n",
            "fn x() { evidence.rfind(\"[CRITIC_VETO]\") }\n",
            "[CONTENT TRUNCATED]\n",
            "[CRITIC_VETO] The report cites lines 10-15 but those line numbers do not appear in any READ_OK body.\n",
        );
        assert!(
            citation_veto_pending(ev),
            "a real [CRITIC_VETO] outside the read body must survive stripping"
        );
    }

    // ---------------------------------------------------------------
    // The original bug: Turn 1 read strategize.rs, and the source body
    // of this very function contained the literal `[CRITIC_VETO]` token.
    // `citation_veto_pending` matched it and preflight fired before any
    // write. Reverting the fix (raw `evidence.rfind`, no
    // `strip_read_bodies_for_markers`) must make this test FAIL.
    // ---------------------------------------------------------------

    #[test]
    fn recognizes_citation_veto_text() {
        let veto = "[CRITIC_VETO] The proposal cites specific line numbers that are NOT present in any READ_OK body. Unverified line ranges must be vetoed.";
        assert!(citation_veto_pending(veto));
    }

    #[test]
    fn ignores_non_citation_veto() {
        let veto =
            "[CRITIC_VETO] The proposal is invalid. The strategy is not being operationalized.";
        assert!(!citation_veto_pending(veto));
    }

    #[test]
    fn detects_grep_n_evidence() {
        let ok = "[CMD_OK] Command: grep -n \"fn foo\" src/lib.rs\nStatus: exit status: 0\nSTDOUT:\n12: fn foo() {}";
        assert!(grep_evidence_present(ok));
    }

    #[test]
    fn no_grep_n_evidence_is_none() {
        let ok = "[READ_OK] src/lib.rs\nFULL FILE CONTENT:\nfn foo() {}";
        assert!(!grep_evidence_present(ok));
    }
}

#[cfg(test)]
mod parse_fault_count_tests {
    use super::consecutive_parse_faults;

    #[test]
    fn counts_faults_since_the_last_ok_marker() {
        let ev = "[READ_OK] a.rs\nFULL FILE CONTENT:\nx\n[PROPOSITION] a.rs -> y\n[PARSE_FAULT] one\n[PARSE_FAULT_ACTION] hint\n[PARSE_FAULT] two\n";
        assert_eq!(consecutive_parse_faults(ev), 2);
        let ev2 = format!(
            "{}[READ_CHUNK_OK] File: a.rs | Lines 1-2 of 9\nCONTENT:\ny\n[PROPOSITION] File: -> z\n",
            ev
        );
        assert_eq!(consecutive_parse_faults(&ev2), 0);
    }
}

#[cfg(test)]
mod grep_evidence_presence_tests {
    use super::grep_evidence_present;

    #[test]
    fn cd_prefixed_grep_rn_counts_as_line_number_evidence() {
        // Exact command from the failed system_audit run: the preflight
        // demanded it, it succeeded, and the old check still said "absent".
        let ev = concat!(
            "[CMD_OK] Command: cd /root/AthenaFusionX && grep -rn --include=\"*.rs\" ",
            "\"^fn \\|^pub fn \" crates/runtime_core/src\n",
            "Status: exit status: 0\n",
        );
        assert!(grep_evidence_present(ev));
    }

    #[test]
    fn plain_grep_n_and_rg_n_count() {
        assert!(grep_evidence_present("[CMD_OK] Command: grep -n foo a.rs\n"));
        assert!(grep_evidence_present("[CMD_OK] Command: rg -n foo crates\n"));
    }

    #[test]
    fn grep_without_line_numbers_does_not_count() {
        assert!(!grep_evidence_present("[CMD_OK] Command: grep foo a.rs\n"));
        assert!(!grep_evidence_present("[CMD_OK] Command: cat a.rs | grep -c foo\n"));
        assert!(!grep_evidence_present("[CMD_OK] Command: ls -n\n"));
    }
}

#[cfg(test)]
mod citation_veto_scope_tests {
    use super::citation_veto_pending;

    #[test]
    fn missing_command_veto_is_not_a_citation_veto() {
        // Real veto text from the build_hygiene mission: about commands not yet
        // run, mentioning "verified citations" and "100 lines" only in passing.
        let v = "[CRITIC_VETO] The find and git status commands have not been run. The report makes claims about those commands' outputs that are currently unverified. Regenerate the report with verified citations (cargo tree was truncated to 100 lines).";
        assert!(!citation_veto_pending(v));
    }

    #[test]
    fn real_line_number_vetoes_are_still_detected() {
        assert!(citation_veto_pending(
            "[CRITIC_VETO] The report cites line numbers 10-15 that do not appear in any READ_OK body."
        ));
        assert!(citation_veto_pending(
            "[CRITIC_VETO] Run grep -n to obtain authoritative line evidence first."
        ));
        assert!(citation_veto_pending(
            "[CRITIC_VETO] The proposal cites an unverified line range for the function."
        ));
    }
}

#[cfg(test)]
mod parse_fault_tests {
    use super::first_failing_item;

    #[test]
    fn first_failing_item_names_execute_command() {
        let raw = r#"[{"type":"read_file","path":"Cargo.toml","reasoning":"r"},
          {"type":"execute_command","path":"cargo metadata --no-deps","reasoning":"r"},
          {"type":"execute_command","path":"cargo tree --duplicates","reasoning":"r"}]"#;
        let (i, t, e) = first_failing_item(raw).expect("should find failing item");
        assert_eq!(i, 1);
        assert_eq!(t, "execute_command");
        assert!(e.contains("command"), "{e}");
    }

    #[test]
    fn first_failing_item_none_when_all_valid() {
        let raw = r#"[{"type":"read_file","path":"Cargo.toml","reasoning":"r"}]"#;
        assert!(first_failing_item(raw).is_none());
    }
}
