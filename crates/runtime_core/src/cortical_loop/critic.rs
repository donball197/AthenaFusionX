//! CorticalLoop::handle_critic (moved verbatim from cortical_loop.rs).

use super::*;

impl CorticalLoop {
    pub(super) async fn handle_critic(
        &mut self,
        turn: u32,
        session: &mut EpistemicSession,
        mission: &str,
        world_context: &str,
        input: CriticInput,
    ) -> Result<CortexState> {
        let CriticInput { actions, proposal, belief } = input;

        let status =
            ContractEvaluator::evaluate(&session.contract, &session.local_evidence, belief);

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

        let actions = match self.filter_anti_loop(session, actions) {
            AntiLoopOutcome::Proceed(acts) => acts,
            AntiLoopOutcome::NeedsStrategize => return Ok(CortexState::Strategize),
            AntiLoopOutcome::StuckNodeFailed { belief, evidence } => {
                return Ok(CortexState::Fail { belief, evidence });
            }
        };

        // NEXUS_FIX_LEDGER_BYPASS: next unread chunk is deterministic pagination; skip the critic
        if session.contract.read_only && actions.len() == 1 {
            if let Some((gap_start, _)) =
                session.coverage.largest_interior_gap().or_else(|| session.coverage.largest_gap())
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
            Some(AgentAction::Halt { reasoning, .. }) => {
                Some(crate::cortical::halt_claims::halt_defects(reasoning, &session.local_evidence))
            }
            _ => None,
        };
        if session.contract.read_only && actions.len() == 1 {
            if let Some(mut defects) = halt_check {
                // Truncation guard: a report cut off mid-sentence (output token cap)
                // must not pass on length alone.
                if let Some(AgentAction::Halt { reasoning, .. }) = actions.first() {
                    let tail = reasoning.trim_end();
                    if !tail.ends_with(['.', ')', '!', '`', '"']) {
                        defects.push(format!(
                            "report appears truncated (ends with {:?}); finish the final sentence",
                            tail.chars()
                                .rev()
                                .take(12)
                                .collect::<Vec<_>>()
                                .into_iter()
                                .rev()
                                .collect::<String>()
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
                self.log(format!(
                    "\x1b[1;31m>> [HALT PRE-GATE] Rejecting halt: {:?}\x1b[0m",
                    defects
                ));
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
        let current_action_keys =
            actions.iter().map(crate::cortical::action_identity::action_key).collect::<Vec<_>>();

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
            if let AgentAction::PatchFunction {
                _thought_process: None,
                path,
                function_name,
                new_body,
                ..
            } = a
            {
                let needle = patch_signature_needle(path, function_name.trim());
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
                AgentAction::ReadFile { _thought_process: None, path, .. }
                | AgentAction::ReadFileChunk { _thought_process: None, path, .. } => Some(format!(
                    "\n[FILESYSTEM FACT - AUTHORITATIVE] `{}` {} in the workspace.",
                    path,
                    if std::path::Path::new(path).is_dir() {
                        "EXISTS as a DIRECTORY (read_file on a directory is VALID: it returns a sorted listing of its entries; do NOT veto it for not being a file)"
                    } else if std::path::Path::new(path).exists() {
                        "EXISTS"
                    } else {
                        "does NOT exist"
                    }
                )),
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
                let action_is_search = matches!(
                    actions.first(),
                    Some(AgentAction::SearchCode { _thought_process: None, .. })
                );
                let strategy_is_search = h_content.starts_with("search_code:");
                let critic_veto_is_redundancy = critic_text.contains("redundant")
                    || critic_text.contains("already read")
                    || critic_text.contains("already contains")
                    || critic_text.contains("proceed immediately to writing")
                    || critic_text.contains("proceed immediately to patching");

                let action_is_write = matches!(
                    actions.first(),
                    Some(AgentAction::WriteFile { _thought_process: None, .. })
                );
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
                        session.graph.add_edge(
                            obs_id,
                            h_id,
                            EdgeRelation::Contradicts,
                            // A vetoed halt judges the completion proposal, not the
                            // strategy that produced the evidence.
                            if (action_is_write && strategy_is_write)
                                || matches!(actions.first(), Some(AgentAction::Halt { .. }))
                            {
                                0.10
                            } else {
                                0.40
                            },
                        );
                        session.graph.propagate_beliefs();
                    }
                    Ok(CortexState::Strategize)
                }
            }
        }
    }
}

/// Text a patch_function `new_body` must contain for the file's language: the
/// COMPLETE function item including its signature. Languages other than Rust and
/// Python are not checked (empty needle always matches).
fn patch_signature_needle(path: &str, name: &str) -> String {
    let ext = std::path::Path::new(path.trim())
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match ext.as_str() {
        "rs" => format!("fn {}", name),
        "py" => format!("def {}", name),
        _ => String::new(),
    }
}

#[cfg(test)]
mod patch_needle_tests {
    use super::patch_signature_needle;

    #[test]
    fn rust_requires_fn_item() {
        assert_eq!(patch_signature_needle("src/lib.rs", "add"), "fn add");
    }

    #[test]
    fn python_requires_def_item() {
        assert_eq!(patch_signature_needle("math_ops.py", "divide"), "def divide");
        assert!("def divide(a, b):\n    return 0".contains(&patch_signature_needle("math_ops.py", "divide")));
    }

    #[test]
    fn unknown_extensions_are_unchecked() {
        assert_eq!(patch_signature_needle("x.js", "f"), "");
        assert_eq!(patch_signature_needle("Makefile", "f"), "");
        assert!("anything".contains(&patch_signature_needle("x.js", "f")));
    }

    #[test]
    fn path_is_trimmed_and_case_insensitive() {
        assert_eq!(patch_signature_needle("  a/B.PY ", "g"), "def g");
    }
}
