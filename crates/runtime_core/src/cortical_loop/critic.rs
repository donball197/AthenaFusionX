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

        // PROTECTED_PATH_GATE (deterministic; runs before the LLM critic and
        // before every FACT_OVERRIDE branch). vendor/ and third_party/ are
        // read-only unless the contract names the exact path as its artifact.
        // This is the structural defence against the OpenFang failure class:
        // the LLM critic vetoes a vendor mutation, then FACT_OVERRIDE lands it
        // anyway. This gate cannot be overridden.
        {
            use crate::cortical::protected_path as pp;
            let allowed: Vec<String> =
                session.contract.required_artifact_paths.iter().map(|a| a.path.clone()).collect();
            if let Some(msg) = actions.iter().find_map(|a| pp::protected_violation(a, &allowed)) {
                self.log(format!(">> [PROTECTED PATH GATE] {}", msg));
                session.last_critic_rejected_actions = Some(
                    actions
                        .iter()
                        .map(crate::cortical::action_identity::action_key)
                        .collect::<Vec<_>>(),
                );
                session.last_critic_rejected_at_evidence_len = Some(session.local_evidence.len());
                session.local_evidence.push_str(&format!(
                    "\n[CRITIC_VETO] {} [CRITIC_VETO_ACTION] The rejected proposal MUST NOT be repeated. Select a materially different, mission-compliant action.\n",
                    msg
                ));
                return Ok(CortexState::Strategize);
            }
        }

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

        // NEXUS_FIX_LEDGER_BYPASS: next unread chunk is deterministic pagination; skip the critic.
        // Applies to both read_only and mutation contracts: a chunk read at the coverage
        // gap start is deterministic pagination in either case. Without this, mutation
        // missions call the critic 3-4x per file just to say "yes, read the next chunk".
        if actions.len() == 1 {
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

        // REPORT_GROUNDING_GATE (deterministic; runs before the LLM critic and
        // before every FACT_OVERRIDE branch, so it cannot be overridden).
        if let Some(AgentAction::WriteFile { path, content, .. }) = actions.first() {
            use crate::cortical::halt_claims as hc;
            if hc::is_report_artifact(path)
                && (hc::wants_pub_items(&session.contract.objective)
                    || hc::wants_pub_items(mission))
            {
                let (grounded, ungrounded) = hc::check_pub_claims(content, &session.local_evidence);
                if !ungrounded.is_empty() || grounded.is_empty() {
                    let names = ungrounded
                        .iter()
                        .map(|n| format!("`{}`", n))
                        .collect::<Vec<_>>()
                        .join(", ");
                    let msg = if ungrounded.is_empty() {
                        concat!(
                            "GROUNDING GATE: the report cites no public item that the evidence defines as public (placeholders are rejected). ",
                            "If the evidence already holds the search hits, write the COMPLETE report now from them, naming items as `pub fn name` or `pub struct Name`. ",
                            "Otherwise gather evidence first (search_code for 'pub ' in each named file, or read_file_chunk the unread ranges)."
                        )
                        .to_string()
                    } else {
                        format!(
                            concat!(
                                "GROUNDING GATE: the report names item(s) the evidence never shows as public definitions: {}. ",
                                "A call site, `use` import, `impl` header, pub(crate) or private item does not count. ",
                                "Read the defining file (search_code for 'pub ' in it, or read_file_chunk the unread ranges) ",
                                "or remove those names. Do not describe items you did not see."
                            ),
                            names
                        )
                    };
                    self.log(format!(
                        ">> [GROUNDING GATE] Report write rejected: {} ungrounded, {} grounded.",
                        ungrounded.len(),
                        grounded.len()
                    ));
                    session.last_critic_rejected_actions = Some(
                        actions
                            .iter()
                            .map(crate::cortical::action_identity::action_key)
                            .collect::<Vec<_>>(),
                    );
                    session.last_critic_rejected_at_evidence_len =
                        Some(session.local_evidence.len());
                    session.local_evidence.push_str(&format!(
                        "\n[CRITIC_VETO] {} [CRITIC_VETO_ACTION] The rejected proposal MUST NOT be repeated. Select a materially different, mission-compliant action.\n",
                        msg
                    ));
                    // No graph edge: content veto must not decay the write_file hypothesis.
                    return Ok(CortexState::Strategize);
                }
            }
        }

        // REPORT_CONTENT_CONSISTENCY_GATE (Phase 3): before a report write_file
        // crosses the executor or critic bypass, verify its backticked symbol
        // claims against READ_OK/evidence. Prevents fabricated reports from
        // landing and passing WritePathAcquired.
        if let Some(AgentAction::WriteFile { path, content, .. }) = actions.first() {
            use crate::completion_engine::CompletionCondition as CC;
            use crate::cortical::halt_claims as hc;
            if hc::is_report_artifact(path) {
                let allow_obj = !session.contract.conditions.iter().any(|c| {
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
                let bad = hc::write_report_unverified_symbols(
                    path,
                    content,
                    &session.local_evidence,
                    &session.contract.objective,
                    allow_obj,
                );
                if !bad.is_empty() {
                    let names =
                        bad.iter().map(|n| format!("`{}`", n)).collect::<Vec<_>>().join(", ");
                    self.log(format!(
                        ">> [CONTENT CONSISTENCY GATE] Rejecting report `{}`: unverified symbol(s) {}",
                        path, names
                    ));
                    session.last_critic_rejected_actions = Some(
                        actions
                            .iter()
                            .map(crate::cortical::action_identity::action_key)
                            .collect::<Vec<_>>(),
                    );
                    session.last_critic_rejected_at_evidence_len =
                        Some(session.local_evidence.len());
                    session.local_evidence.push_str(&format!(
                        "\n[CRITIC_VETO] CONTENT CONSISTENCY GATE: report `{}` cites symbol(s) never seen in READ_OK/evidence: {}. Read the defining source file(s) with read_file/search_code before citing them, or remove unverified claims. [CRITIC_VETO_ACTION] Do not repeat the fabricated write.\n",
                        path, names
                    ));
                    return Ok(CortexState::Strategize);
                }
            }
        }

        // REQUIRED-COMMAND BYPASS: the exact command the directive requires is
        // mission-authoritative. Once every other condition is met (e.g. the patch
        // landed) the LLM critic has nothing left to judge, and it has been observed
        // vetoing this command on a stale reading of the evidence. The repeat-veto
        // latch then makes the identical (correct) proposal unrecoverable.
        if actions.len() == 1
            && is_required_verification_command(
                &session.contract.conditions,
                &session.local_evidence,
                &actions[0],
            )
        {
            self.log(
                "\x1b[1;36m>> [CRITIC BYPASS] Contract-required command with all other conditions met; LLM critic skipped.\x1b[0m"
                    .to_string(),
            );
            session.last_critic_rejected_actions = None;
            return Ok(CortexState::Execute { actions, proposal, belief });
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
                    let unclosed_backtick = reasoning.matches('`').count() % 2 != 0;
                    if unclosed_backtick || !tail.ends_with(['.', ')', '!', '`', '"']) {
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
                // HALT PRE-GATE HINT: name the specific corrective action per
                // defect class. Without this, the Cortex re-proposes the same
                // malformed report for many turns, burning budget without
                // acquiring the evidence it needs.
                {
                    let joined = defects.join("; ");
                    let hint: &str = if joined
                        .contains("symbols not found in the file content read")
                    {
                        "[HALT_PRE_GATE_HINT] Your report backticked at least one name that does \
                         not appear as a whole word in any [READ_OK]/[READ_CHUNK_OK] block. Two \
                         remedies: (a) if the name is real, `read_file` the file that defines it \
                         and re-cite; (b) if the name is a tool or status marker (e.g. `read_file`, \
                         `search_code`, `[CMD_OK]`), remove the backticks -- tool names are not \
                         source symbols.\n"
                    } else if joined.contains("no verified source identifiers cited in backticks") {
                        "[HALT_PRE_GATE_HINT] Your report has no backticked symbol that appears \
                         in your evidence corpus. A single backticked identifier from any search \
                         result is enough -- e.g. if you searched for `router`, cite `router` or \
                         `AgentProfile`.\n"
                    } else if joined.contains("must cite at least") {
                        "[HALT_PRE_GATE_HINT] The report is thin for the evidence on hand. Read \
                         at least one file the searches already located before halting.\n"
                    } else if joined.contains("cited at lines") {
                        "[HALT_PRE_GATE_HINT] You cited a line range that does not appear in \
                         the coverage ledger. Use the exact ranges returned by `read_file_chunk`.\n"
                    } else if joined.contains("report appears truncated") {
                        "[HALT_PRE_GATE_HINT] The report was cut off mid-sentence. Finish the \
                         final sentence.\n"
                    } else if joined.contains("report is only")
                        || joined.contains("report is too short")
                    {
                        "[HALT_PRE_GATE_HINT] The report is below the minimum length for its \
                         evidence corpus. Deliver the full findings, not a summary.\n"
                    } else {
                        ""
                    };
                    if !hint.is_empty() {
                        session.local_evidence.push_str(hint);
                    }
                }
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

        // Evidence-growth gate: a retried action batch is only repeat-vetoed
        // while evidence has not meaningfully grown since the last rejection.
        // 500 bytes is larger than any single [CRITIC_VETO] + [CRITIC_REPEAT_VETO]
        // message pair, so ordinary veto churn does not trip the gate; any real
        // tool result (read, search, command) does.
        const REPEAT_VETO_EVIDENCE_GROWTH_FLOOR: usize = 500;
        let evidence_unchanged = session.last_critic_rejected_at_evidence_len.map_or(true, |len| {
            session.local_evidence.len().saturating_sub(len) < REPEAT_VETO_EVIDENCE_GROWTH_FLOOR
        });
        if session.last_critic_rejected_actions.as_ref() == Some(&current_action_keys)
            && evidence_unchanged
        {
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

        // BUG-19: a proposal that deterministically resolves at
        // least one unmet completion condition is contract-
        // progressing by construction. The LLM critic's role is
        // scope/safety review; "is this enough progress?" has a
        // deterministic answer (task 2: six parallel criteria
        // flattened into one node; each 1-of-6 search was approved
        // or vetoed at the LLM's whim).
        if crate::contract_status::proposal_advances_contract(
            &actions,
            &session.contract,
            &session.local_evidence,
        ) {
            self.log(
                "\x1b[1;36m>> [CRITIC BYPASS] Proposal advances contract deterministically; LLM critic skipped.\x1b[0m"
                    .to_string(),
            );
            session.last_critic_rejected_actions = None;
            return Ok(CortexState::Execute { actions, proposal, belief });
        }

        // VERIFICATION-READ BYPASS: a read_file on a required artifact that
        // has already been written ([WRITE_OK] in evidence) but not yet
        // read back ([READ_OK] absent) is the mandatory post-write
        // verification step. The action is strictly non-destructive, the
        // artifact exists, and executing it satisfies the pending
        // ReadPathAcquired for that path. This is deterministic — do not
        // ask the LLM critic to second-guess it. The critic has been
        // observed to misread the strategist's stale "write the report"
        // reasoning text as a write attempt, and to anchor on stale
        // PATH_NOT_FOUND events that pre-date the successful [WRITE_OK].
        // Confirmed blocker of plan 40cfaaba (2026-11-12): turns 33-60
        // exhausted on exactly this veto loop.
        {
            let verification_read = actions.iter().any(|a| {
                if let AgentAction::ReadFile { _thought_process: None, path, .. } = a {
                    session.contract.required_artifact_paths.iter().any(|art| &art.path == path)
                        && session.local_evidence.contains(&format!("[WRITE_OK] {}", path))
                        && !session.local_evidence.contains(&format!("[READ_OK] {}", path))
                } else {
                    false
                }
            });
            if verification_read {
                self.log(
                    "\x1b[1;36m>> [CRITIC BYPASS] Post-write verification read of required artifact; LLM critic skipped.\x1b[0m"
                        .to_string(),
                );
                session.last_critic_rejected_actions = None;
                return Ok(CortexState::Execute { actions, proposal, belief });
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
        let critic_evidence = format!(
            "{}{}",
            critic_evidence,
            contract_facts(&session.contract.conditions, &session.local_evidence)
        );
        // For write_file proposals, the proposal supersedes any prior version
        // of the same path still visible in local_evidence. Without this note,
        // the LLM critic compares the new content against the stale content
        // captured by an earlier [READ_OK] / earlier write and vetoes the
        // correction as a mismatch. The note is emitted unconditionally for
        // write_file actions; when no prior version exists the phrase "any
        // previous content" is vacuously satisfied and the note is harmless.
        let supersession_note: String = actions
            .iter()
            .filter_map(|a| match a {
                AgentAction::WriteFile { path, .. } => Some(format!(
                    "\n[CRITIC PROPOSAL AUTHORITY - {}] The proposed write_file \
                     REPLACES any previous content of this path visible in the \
                     evidence above. Judge the PROPOSAL CONTENT against the \
                     SOURCE-CODE evidence (READ_OK blocks for other paths, \
                     CMD_OK output, SEARCH_CODE_OK results). Do NOT reject a \
                     proposal because it differs from the previous version of \
                     the same file in local_evidence - that is the correct \
                     behavior for a correction after a prior rejection.\n",
                    path
                )),
                _ => None,
            })
            .collect();
        let critic_evidence = format!("{}{}", critic_evidence, supersession_note);
        let citation_note = concat!(
            "\n[CRITIC ACTION IDENTITY] The PROPOSED ACTION is the JSON object below. Judge its `type` field. The `reasoning` field is narrative — it may describe a prior action, a next action, or current rationale; do NOT infer type from reasoning. A `read_file` whose reasoning starts \"Write the report…\" is a read_file, not a write_file.\n",
            "\n[CRITIC CITATION DISCIPLINE] ",
            "Assert line numbers or symbol spans ONLY when those exact numbers ",
            "are visible in one of: (a) a READ_OK / READ_CHUNK_OK body in evidence; ",
            "(b) a SEARCH_CODE_OK / SEARCH_CHECKED_OK match list naming the file and line; ",
            "or (c) a [CMD_OK] stdout block from a `grep -n` or `rg -n` command that names the file and line. ",
            "READ_OK and READ_CHUNK_OK bodies carry a line-range header but do NOT number ",
            "individual body lines, so a `grep -n` block in [CMD_OK] IS the authoritative ",
            "source for source-file line numbers and its output IS valid pairing evidence. ",
            "Do NOT reject a citation whose line number appears in a [CMD_OK] `grep -n` block; ",
            "that is the only way to obtain a line number from an unnumbered READ_OK body. ",
            "If a span is not visible in any of those sources, say unverified. Do NOT invent lines N-M ",
            "and do NOT hand the strategist a full replacement report filled with ",
            "guessed line ranges. Demand re-derivation from the traced source body.\n",
        );
        let typed_ledger = if session.typed_evidence.is_empty() {
            String::new()
        } else {
            let lines: Vec<String> = session
                .typed_evidence
                .iter()
                .rev()
                .take(12)
                .rev()
                .map(|r| r.render_ledger_line())
                .collect();
            format!("\n[TYPED EVIDENCE LEDGER]\n{}\n", lines.join("\n"))
        };
        let critic_evidence = format!("{}{}{}", critic_evidence, citation_note, typed_ledger);
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
                    session.last_critic_rejected_at_evidence_len =
                        Some(session.local_evidence.len());

                    // Feed the critic's rejection back into the next
                    // Strategize -> Propose cycle. Graph-only feedback is not
                    // sufficient because the strategist receives local_evidence.
                    // Strip ungrounded line-span prescriptions so the strategist
                    // re-derives citations from READ_OK instead of obeying invented ranges.
                    let critique_flat = critic.critique.replace('\n', " ");
                    let guarded_critique =
                        sanitize_ungrounded_citation_veto(&critique_flat, &session.local_evidence);
                    let veto_feedback = format!(
                        "\n[CRITIC_VETO] {} [CRITIC_VETO_ACTION] The rejected proposal MUST NOT be repeated. Select a materially different, mission-compliant action.\n",
                        guarded_critique
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
/// COMPLETE function item including its signature (per-language keyword; bare
/// function name for unknown languages).

fn sanitize_ungrounded_citation_veto(verdict: &str, evidence: &str) -> String {
    let spans = regex_lite_spans(verdict);
    if spans.is_empty() {
        return verdict.to_string();
    }

    let mut out = verdict.to_string();
    let mut any_ungrounded = false;
    for span in &spans {
        if evidence.contains(span) {
            continue;
        }
        any_ungrounded = true;
        // Neutralize the token so the strategist cannot copy it as authority.
        out = out.replace(span, "unverified-span");
    }

    if any_ungrounded {
        out.push_str(
            "\n[CITATION_GUARD] One or more prescribed line ranges in this veto do not appear in local evidence. Ignore unverified-span placeholders. Re-derive symbol spans from the existing READ_OK / READ_CHUNK_OK body and rewrite the report from that body only. Do not trust invented line ranges.",
        );
    }
    out
}

fn regex_lite_spans(text: &str) -> Vec<String> {
    // Candidate line-range tokens of the form N-M.
    // If the critique mentions "line" anywhere, every N-M is a candidate
    // (covers "not 95-105, 111-132, and 140-165" after an earlier "lines ...").
    // Otherwise require a local "line" window so version ranges in unrelated
    // vetoes are left alone.
    let line_context_global = text.to_ascii_lowercase().contains("line");
    let mut spans = Vec::new();
    let bytes = text.as_bytes();
    let n = bytes.len();
    let mut i = 0;
    while i < n {
        if bytes[i].is_ascii_digit() {
            let start = i;
            while i < n && bytes[i].is_ascii_digit() {
                i += 1;
            }
            let a = &text[start..i];
            if i < n && bytes[i] == b'-' {
                let j = i + 1;
                let mut k = j;
                while k < n && bytes[k].is_ascii_digit() {
                    k += 1;
                }
                if k > j {
                    let accept = if line_context_global {
                        true
                    } else {
                        let ctx_from = start.saturating_sub(12);
                        text[ctx_from..k].to_ascii_lowercase().contains("line")
                    };
                    if accept {
                        spans.push(format!("{}-{}", a, &text[j..k]));
                    }
                    i = k;
                    continue;
                }
            }
        }
        i += 1;
    }
    spans
}

fn patch_signature_needle(path: &str, name: &str) -> String {
    let ext = std::path::Path::new(path.trim())
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match ext.as_str() {
        "rs" | "zig" => format!("fn {}", name),
        "py" => format!("def {}", name),
        "go" => format!("func {}", name),
        "php" => format!("function {}", name),
        // Unknown language: at least require the function's own name.
        _ => name.to_string(),
    }
}

#[cfg(test)]
mod patch_needle_tests {
    use super::patch_signature_needle;

    #[test]
    fn rust_and_zig_require_fn_item() {
        assert_eq!(patch_signature_needle("src/lib.rs", "add"), "fn add");
        assert_eq!(patch_signature_needle("a.zig", "add"), "fn add");
    }

    #[test]
    fn python_requires_def_item() {
        assert_eq!(patch_signature_needle("math_ops.py", "divide"), "def divide");
        assert!("def divide(a, b):\n    return 0"
            .contains(&patch_signature_needle("math_ops.py", "divide")));
    }

    #[test]
    fn go_and_php_have_their_own_keywords() {
        assert_eq!(patch_signature_needle("m.go", "f"), "func f");
        assert_eq!(patch_signature_needle("m.php", "f"), "function f");
    }

    #[test]
    fn unknown_extensions_require_the_function_name() {
        assert_eq!(patch_signature_needle("x.js", "f"), "f");
        assert_eq!(patch_signature_needle("Makefile", "f"), "f");
        assert!(!"unrelated body".contains(&patch_signature_needle("x.js", "build")));
    }

    #[test]
    fn path_is_trimmed_and_case_insensitive() {
        assert_eq!(patch_signature_needle("  a/B.PY ", "g"), "def g");
    }
}

// (Removed local normalize_cmd — see crate::cmd_match::normalize_cmd,
//  which strips directive suffixes like 'and report stdout' so this
//  gate agrees with the strategy-match gate on the same command.)
use crate::cmd_match::normalize_cmd;

/// True when `action` is exactly the command the contract requires, the
/// requirement is still unmet, and every other (non-belief) condition is already
/// met. Pure over (conditions, evidence, action) so it is unit-testable.
fn is_required_verification_command(
    conditions: &[crate::completion_engine::CompletionCondition],
    evidence: &str,
    action: &AgentAction,
) -> bool {
    use crate::completion_engine::CompletionCondition as CC;
    let AgentAction::ExecuteCommand { _thought_process: None, command, .. } = action else {
        return false;
    };
    let wanted = normalize_cmd(command);
    let mut command_unmet = false;
    for c in conditions {
        match c {
            CC::CommandExecutionVerified { command: required } => {
                if normalize_cmd(required) != wanted {
                    return false;
                }
                if !crate::cortical::contract_advance::condition_is_met(c, evidence) {
                    command_unmet = true;
                }
            }
            CC::MinBeliefScore(_) => {}
            other => {
                if !crate::cortical::contract_advance::condition_is_met(other, evidence) {
                    return false;
                }
            }
        }
    }
    command_unmet
}

/// Deterministic contract status handed to the LLM critic as authoritative fact,
/// so it cannot invent a required marker or claim a met condition is missing.
fn contract_facts(
    conditions: &[crate::completion_engine::CompletionCondition],
    evidence: &str,
) -> String {
    let mut met: Vec<String> = Vec::new();
    let mut unmet: Vec<String> = Vec::new();
    for c in conditions {
        if matches!(c, crate::completion_engine::CompletionCondition::MinBeliefScore(_)) {
            continue;
        }
        let label = format!("{:?}", c);
        if crate::cortical::contract_advance::condition_is_met(c, evidence) {
            met.push(label);
        } else {
            unmet.push(label);
        }
    }
    format!(
        "\n[CONTRACT FACT - AUTHORITATIVE] Deterministic contract status. MET: {:?}. UNMET: {:?}. Do not claim a MET condition is missing from the evidence, and do not require a marker that is not listed here (for example ANALYSIS_OK) unless it appears in UNMET.",
        met, unmet
    )
}

#[cfg(test)]
mod contract_fact_tests {
    use super::{contract_facts, is_required_verification_command};
    use crate::agent_protocol::AgentAction;
    use crate::completion_engine::CompletionCondition as CC;

    const CMD: &str = "python3 -c \"import math_ops; print(math_ops.divide(10, 0))\"";
    const PATCHED: &str =
        "[PATCH_FUNCTION_OK] math_ops.py:divide\nUpdated function 'divide' in math_ops.py\n";

    fn run(c: &str) -> AgentAction {
        serde_json::from_value(serde_json::json!({"type": "execute_command", "command": c}))
            .expect("execute_command JSON must deserialize as AgentAction")
    }

    fn compound() -> Vec<CC> {
        vec![CC::PatchMarkerAcquired, CC::CommandExecutionVerified { command: CMD.to_string() }]
    }

    #[test]
    fn required_command_is_bypassed_only_after_the_patch_landed() {
        assert!(!is_required_verification_command(&compound(), "", &run(CMD)));
        assert!(is_required_verification_command(&compound(), PATCHED, &run(CMD)));
    }

    #[test]
    fn whitespace_differences_do_not_matter() {
        let spaced = "python3  -c \"import math_ops; print(math_ops.divide(10, 0))\"";
        assert!(is_required_verification_command(&compound(), PATCHED, &run(spaced)));
    }

    #[test]
    fn a_different_command_is_never_bypassed() {
        assert!(!is_required_verification_command(&compound(), PATCHED, &run("rm -rf /tmp/x")));
    }

    #[test]
    fn command_only_contract_bypasses_its_exact_command() {
        let conds = vec![CC::CommandExecutionVerified { command: CMD.to_string() }];
        assert!(is_required_verification_command(&conds, "", &run(CMD)));
    }

    #[test]
    fn non_command_actions_are_never_bypassed() {
        let read: AgentAction =
            serde_json::from_value(serde_json::json!({"type": "read_file", "path": "math_ops.py"}))
                .expect("read_file JSON must deserialize as AgentAction");
        assert!(!is_required_verification_command(&compound(), PATCHED, &read));
    }

    #[test]
    fn contract_facts_list_met_and_unmet_and_skip_the_belief_threshold() {
        let conds = vec![CC::PatchMarkerAcquired, CC::MinBeliefScore(0.65)];
        let out = contract_facts(&conds, PATCHED);
        assert!(out.contains("MET: [\"PatchMarkerAcquired\"]"), "{out}");
        assert!(out.contains("UNMET: []"), "{out}");
        assert!(!out.contains("MinBeliefScore"), "{out}");
    }
}

#[cfg(test)]
mod critic_normalize_tests {
    use super::is_required_verification_command;
    use crate::agent_protocol::AgentAction;
    use crate::completion_engine::CompletionCondition as CC;

    #[test]
    fn normalize_strips_report_suffix_like_cmd_match() {
        assert_eq!(
            super::normalize_cmd("find X | wc -l and report stdout."),
            crate::cmd_match::normalize_cmd("find X | wc -l"),
        );
    }

    #[test]
    fn suffixed_contract_command_bypasses_critic_for_bare_action() {
        // Regression for the class of failure where the contract carries
        // "and report stdout." (from directive phrasing) but the action's
        // command is bare. The bypass must fire so the critic is skipped
        // and the contract-required command runs.
        let conditions = vec![CC::CommandExecutionVerified {
            command: "find X | wc -l and report stdout.".to_string(),
        }];
        let evidence = "";
        let action = AgentAction::ExecuteCommand {
            _thought_process: None,
            command: "find X | wc -l".to_string(),
            reasoning: "test".to_string(),
            confidence: 1.0,
        };
        assert!(
            is_required_verification_command(&conditions, evidence, &action),
            "critic bypass must accept the bare command when the contract \
             carries a directive suffix"
        );
    }
}

#[cfg(test)]
mod citation_sanitizer_tests {
    use super::*;

    #[test]
    fn sanitizer_strips_invented_line_ranges() {
        // Exact critique from turn 14 of the failed marker_summary trace.
        // Every line number is invented — real source has is_marker_line
        // at 95, cmd_ok_binds_command at 111, read_path_satisfied at 140.
        let bad = "the proposed halt is premature because marker_summary.md \
                   cites incorrect line ranges. Source shows `is_marker_line` \
                   at lines 80-90, `cmd_ok_binds_command` at lines 93-114, \
                   and `read_path_satisfied` at lines 117-142, not 95-105, \
                   111-132, and 140-165.";

        let clean = sanitize_ungrounded_citation_veto(bad, "");

        for forbidden in ["80-90", "93-114", "117-142", "95-105", "111-132", "140-165"] {
            assert!(
                !clean.contains(forbidden),
                "invented range {:?} survived sanitization: {}",
                forbidden,
                clean
            );
        }

        assert!(
            clean.contains("premature") || clean.contains("incorrect"),
            "sanitizer stripped the entire critique, leaving no guidance: {}",
            clean
        );
    }

    #[test]
    fn sanitizer_passes_through_critiques_without_line_numbers() {
        let clean_input = "the artifact does not cite the specific functions \
                           the objective requires.";
        let output = sanitize_ungrounded_citation_veto(clean_input, "");

        assert!(
            output.contains("does not cite the specific functions"),
            "sanitizer lost semantic content: {}",
            output
        );
        assert!(
            !output.contains("lines"),
            "sanitizer introduced \"lines\" where input had none: {}",
            output
        );
    }
}
