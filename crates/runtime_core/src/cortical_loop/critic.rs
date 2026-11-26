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

        let (actions, substituted) =
            crate::cortical::recovery::substitute_forced_read(&session.local_evidence, actions);
        if let Some(note) = substituted {
            self.log(format!("\x1b[1;36m{}\x1b[0m", note));
        }
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
                // conformance_guard: never bypass critic after conformance failure
                if session.local_evidence.contains("[WRITE_CONFORMANCE_FAILED]") {
                    self.log("\x1b[1;33m>> [CRITIC FORCED] Previous write failed conformance; LLM critic review required.\x1b[0m".to_string());
                    // fall through to LLM critic
                } else if is_next_chunk {
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
                if !allow_obj
                    && crate::completion_engine::requires_semantic_verification(
                        &session.contract.objective,
                    )
                    && content.trim().len() < 1200
                    && content.matches('`').count() < 2
                {
                    self.log(format!(
                        ">> [CONTENT CONSISTENCY GATE] Rejecting contentless report stub `{}`: cites no backticked symbols or paths.",
                        path
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
                        "\n[CRITIC_VETO] CONTENT CONSISTENCY GATE: report `{}` cites no backticked source symbols, file paths, or commands (appears to be a restated objective stub). Gather source evidence first and write a full grounded report naming observed symbols/paths in backticks. [CRITIC_VETO_ACTION] Do not repeat the stub.\n",
                        path
                    ));
                    return Ok(CortexState::Strategize);
                }
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
                let absent = crate::cortical::halt_claims::write_report_unsupported_absences(
                    path,
                    content,
                    &session.local_evidence,
                );
                if !absent.is_empty() {
                    let names =
                        absent.iter().map(|n| format!("`{}`", n)).collect::<Vec<_>>().join(", ");
                    self.log(format!(
                        ">> [CONTENT CONSISTENCY GATE] Rejecting report `{}`: unsupported absence claim(s) {}",
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
                        "\n[CRITIC_VETO] CONTENT CONSISTENCY GATE: report `{}` claims {} do not exist, but no search_code was ever run for them. Run search_code for each name before asserting absence, or remove the claim. [CRITIC_VETO_ACTION] Do not repeat the unsupported claim.\n",
                        path, names
                    ));
                    return Ok(CortexState::Strategize);
                }
            }
        }

        // FORBIDDEN REMOVAL GATE (report artifacts): a report that recommends
        // deleting protected source, CI, vendored, config or test trees is
        // rejected deterministically, before the critic LLM and before any
        // CRITIC FACT OVERRIDE branch can preserve the write.
        if let Some(AgentAction::WriteFile { path, content, .. }) = actions.first() {
            if crate::cortical::halt_claims::is_report_artifact(path) {
                let removal_vetoes =
                    session.local_evidence.matches("[CRITIC_VETO] Forbidden removal check").count();
                let removal_hit =
                    crate::cortical::forbidden_removal::halt_recommends_forbidden_removal(content);
                if removal_hit.is_some() && removal_vetoes >= 2 {
                    crate::journal::append_critic_event(
                        "critic_gate:forbidden_removal_released",
                        path,
                        "",
                        0.0,
                        removal_hit.as_deref().unwrap_or(""),
                    );
                }
                if let Some(removal) = removal_hit.filter(|_| removal_vetoes < 2) {
                    session.last_critic_rejected_actions = Some(
                        actions
                            .iter()
                            .map(crate::cortical::action_identity::action_key)
                            .collect::<Vec<_>>(),
                    );
                    session.last_critic_rejected_at_evidence_len =
                        Some(session.local_evidence.len());
                    session.local_evidence.push_str(&format!(
                        "\n[CRITIC_VETO] Forbidden removal check: the report proposes deleting a protected path ({removal}). Remove every recommendation to delete source, CI, vendored, config or test trees and rewrite from evidence.\n"
                    ));
                    crate::journal::append_critic_event(
                        "critic_gate:forbidden_removal",
                        path,
                        &actions
                            .first()
                            .map(|a| {
                                format!("{:?}", crate::cortical::action_identity::action_key(a))
                            })
                            .unwrap_or_default(),
                        0.0,
                        &removal,
                    );
                    return Ok(CortexState::Strategize);
                }
            }
        }

        // PATH GROUNDING GATE (report artifacts): every workspace path a report
        // cites must exist on disk. Vetoes hallucinated crate/dir names. After two
        // vetoes it releases, so a report about a not-yet-created path cannot loop.
        if let Some(AgentAction::WriteFile { path, content, .. }) = actions.first() {
            if crate::cortical::halt_claims::is_report_artifact(path) {
                let missing = crate::cortical::path_grounding::ungrounded_paths_on_disk(content);
                if !missing.is_empty() {
                    let prior = session
                        .local_evidence
                        .matches("[CRITIC_VETO] Path grounding check")
                        .count();
                    let listed = missing.iter().take(8).cloned().collect::<Vec<_>>().join(", ");
                    let action_key = actions
                        .first()
                        .map(|a| format!("{:?}", crate::cortical::action_identity::action_key(a)))
                        .unwrap_or_default();
                    if prior < 2 {
                        session.last_critic_rejected_actions = Some(
                            actions
                                .iter()
                                .map(crate::cortical::action_identity::action_key)
                                .collect::<Vec<_>>(),
                        );
                        session.last_critic_rejected_at_evidence_len =
                            Some(session.local_evidence.len());
                        session.local_evidence.push_str(&format!(
                            "\n[CRITIC_VETO] Path grounding check: the report cites paths that do not exist in the workspace ({listed}). List the real directory contents with read_file on the directory, then cite only paths you observed.\n"
                        ));
                        crate::journal::append_critic_event(
                            "critic_gate:ungrounded_paths",
                            path,
                            &action_key,
                            0.0,
                            &listed,
                        );
                        return Ok(CortexState::Strategize);
                    }
                    crate::journal::append_critic_event(
                        "critic_gate:ungrounded_paths_released",
                        path,
                        &action_key,
                        0.0,
                        &listed,
                    );
                }
            }
        }

        // SEMANTIC SUPPORT GATE:
        // Run after deterministic report checks and before required-command bypass.
        if let Some(AgentAction::WriteFile { path, content, .. }) = actions.first() {
            if crate::cortical::halt_claims::is_report_artifact(path) {
                let defects = crate::cortical::semantic_support::semantic_support_defects(
                    content,
                    &session.local_evidence,
                )
                .await;

                if !defects.is_empty() {
                    let joined = defects.join("; ");
                    session.last_critic_rejected_actions = Some(
                        actions
                            .iter()
                            .map(crate::cortical::action_identity::action_key)
                            .collect::<Vec<_>>(),
                    );
                    session.last_critic_rejected_at_evidence_len =
                        Some(session.local_evidence.len());
                    session.local_evidence.push_str(&format!(
                        "\n[CRITIC_VETO] Semantic report support check: {}\n",
                        joined
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
                // must not pass on length alone, but raw reasoning-tag fragments at
                // the tail are normalized before checking sentence punctuation.
                if let Some(AgentAction::Halt { reasoning, .. }) = actions.first() {
                    let mut cleaned_reasoning = reasoning.clone();
                    while let Some(start) = cleaned_reasoning.find("<think>") {
                        if let Some(end_rel) = cleaned_reasoning[start..].find("</think>") {
                            let end = start + end_rel + "</think>".len();
                            cleaned_reasoning.replace_range(start..end, "");
                        } else {
                            cleaned_reasoning.truncate(start);
                            break;
                        }
                    }
                    let tail = cleaned_reasoning.trim_end();
                    let tail = tail
                        .trim_end_matches("{")
                        .trim_end_matches("</think>")
                        .trim_end();
                    // A trailing criterion token the report must carry is not prose.
                    let tail = tail
                        .strip_suffix("ANALYSIS_OK")
                        .map(str::trim_end)
                        .filter(|t| !t.is_empty())
                        .unwrap_or(tail);
                    let unclosed_backtick = cleaned_reasoning.matches('`').count() % 2 != 0;
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

                // FORBIDDEN REMOVAL GATE: a halt report that recommends
                // deleting protected source, CI, or vendored trees is
                // structurally unsafe regardless of how well the prose
                // matches the evidence. This runs before the semantic check
                // so a report can never trade "grounded prose" for
                // "catastrophic action plan."
                if let Some(AgentAction::Halt { reasoning, .. }) = actions.first() {
                    if let Some(removal_msg) =
                        crate::cortical::forbidden_removal::halt_recommends_forbidden_removal(
                            reasoning,
                        )
                    {
                        defects.push(format!("Forbidden removal recommendation: the report proposes deleting a protected path ({removal_msg}); remove that recommendation"));
                    }
                }

                // Tier 2 semantic support check for the Halt reasoning field.
                // Avoid an embedding request when deterministic checks already
                // require the report to be rewritten.
                if defects.is_empty() {
                    if let Some(AgentAction::Halt { reasoning, .. }) = actions.first() {
                        let semantic_defects =
                            crate::cortical::semantic_support::semantic_support_defects(
                                reasoning,
                                &session.local_evidence,
                            )
                            .await;
                        defects.extend(semantic_defects);
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
                    } else if joined.contains("Forbidden removal") {
                        "[HALT_PRE_GATE_HINT] The report recommends deleting protected source, CI, vendored, config or test trees. Remove every deletion recommendation; this audit must not propose removing repository structure.\n"
                    } else if joined.contains("Semantic support below threshold") {
                        "[HALT_PRE_GATE_HINT] The report prose is not semantically supported by your evidence corpus. Read the file(s) the report discusses with `read_file` and rewrite from that body, or remove claims that are not grounded in evidence you have actually acquired.\n"
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

        // Evidence-growth gate: a retried action batch is repeat-vetoed until
        // a physical tool execution marker arrives in local_evidence. Using a
        // raw byte floor breaks when [CRITIC_VETO] + [NODE PHASE] chatter
        // exceeds 500 bytes.
        let evidence_unchanged = session.last_critic_rejected_at_evidence_len.map_or(true, |len| {
            let start = len.min(session.local_evidence.len());
            let tail = &session.local_evidence[start..];
            !tail.lines().any(|l| {
                let t = l.trim_start();
                t.starts_with("[READ_OK]")
                    || t.starts_with("[READ_CHUNK_OK]")
                    || t.starts_with("[READ_FUNCTION_OK]")
                    || t.starts_with("[SEARCH_CODE_OK]")
                    || t.starts_with("[SEARCH_MEMORY_OK]")
                    || t.starts_with("[CMD_OK]")
                    || t.starts_with("[WRITE_OK]")
                    || t.starts_with("[WRITE_NOOP]")
                    || t.starts_with("[PATCH_FUNCTION_OK]")
                    || t.starts_with("[PATCH_IDEMPOTENT_OK]")
                    || t.starts_with("[DIFF_PATCH_OK]")
                    || t.starts_with("[REPLACE_LINES_OK]")
                    || t.starts_with("[RUN_TESTS_OK]")
                    || t.starts_with("[ACTION FAILED]")
            })
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
                    session.contract.required_artifact_paths.iter().any(|art| {
                        crate::evidence_paths::paths_equivalent(&art.path, path)
                            && verification_read_pending(&session.local_evidence, &art.path)
                    })
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
        let condensed_evidence = condense_critic_evidence(&session.local_evidence);
        let critic_evidence = match session
            .coverage
            .largest_interior_gap()
            .or_else(|| session.coverage.largest_gap())
        {
            Some((gap_s, gap_e)) => format!(
                "{}\n\n[COVERAGE LEDGER - AUTHORITATIVE] Deterministic read ledger: unread range {}-{} of {} (coverage {:.1}%). A read_file_chunk starting at line {} is the correct next step and MUST NOT be vetoed as skipping lines.",
                condensed_evidence,
                gap_s,
                gap_e,
                session.coverage.total_lines().unwrap_or(0),
                session.coverage.coverage_pct(),
                gap_s
            ),
            None => match session.coverage.total_lines() {
                Some(total) => format!(
                    "[COVERAGE LEDGER - AUTHORITATIVE] 100% of {} lines have been read; no unread range exists. Do NOT veto for unread lines.\n{}\n[COVERAGE LEDGER - AUTHORITATIVE] 100% of {} lines read.",
                    total, condensed_evidence, total
                ),
                None => condensed_evidence.clone(),
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
                let critic_veto_is_pre_recon = is_pre_recon_override(&critic_text);

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
                    crate::journal::append_critic_event(
                        "critic_override:search_code",
                        &h_content.to_string(),
                        &actions
                            .first()
                            .map(|a| {
                                format!("{:?}", crate::cortical::action_identity::action_key(a))
                            })
                            .unwrap_or_default(),
                        critic.confidence_modifier as f32,
                        &critic.critique,
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
                    crate::journal::append_critic_event(
                        "critic_override:write_file",
                        &h_content.to_string(),
                        &actions
                            .first()
                            .map(|a| {
                                format!("{:?}", crate::cortical::action_identity::action_key(a))
                            })
                            .unwrap_or_default(),
                        critic.confidence_modifier as f32,
                        &critic.critique,
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
                    crate::journal::append_critic_event(
                        "critic_override:ast_noop",
                        &h_content.to_string(),
                        &actions
                            .first()
                            .map(|a| {
                                format!("{:?}", crate::cortical::action_identity::action_key(a))
                            })
                            .unwrap_or_default(),
                        critic.confidence_modifier as f32,
                        &critic.critique,
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

/// True when a required artifact was written and not read back since that write.
/// Epoch-aware: an older `[READ_OK]` does not count once a later `[WRITE_OK]`
/// replaced the file, so every rewrite gets its own verification read without
/// asking the LLM critic (which vetoed these as anti-redundant reads).
fn verification_read_pending(evidence: &str, path: &str) -> bool {
    let mut last_write: Option<usize> = None;
    let mut last_read: Option<usize> = None;
    for (idx, line) in evidence.lines().enumerate() {
        let t = line.trim_start();
        if let Some(rest) = t.strip_prefix("[WRITE_OK]") {
            let p = rest.trim();
            let p = p
                .strip_prefix("Successfully synthesized and wrote report to ")
                .unwrap_or(p)
                .trim();
            if crate::evidence_paths::paths_equivalent(p, path) {
                last_write = Some(idx);
            }
        } else if let Some(rest) = t.strip_prefix("[READ_OK]") {
            if crate::evidence_paths::paths_equivalent(rest.trim(), path) {
                last_read = Some(idx);
            }
        }
    }
    match (last_write, last_read) {
        (Some(_), None) => true,
        (Some(w), Some(r)) => w > r,
        _ => false,
    }
}

/// Harness marker line: column 0, `[` + UPPERCASE/digit/underscore/space + `]`.
fn critic_marker_line(line: &str) -> bool {
    let Some(rest) = line.strip_prefix('[') else {
        return false;
    };
    let Some(end) = rest.find(']') else {
        return false;
    };
    let tag = &rest[..end];
    tag.chars().next().map_or(false, |c| c.is_ascii_uppercase())
        && tag.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_' || c == ' ')
}

/// Condense evidence for the LLM critic so it cannot anchor on stale text:
///  * a `[READ_OK] <path>` block is cut to one line when the same path was read
///    again later or written later (the newest read is kept);
///  * every `[CRITIC_VETO]` block except the newest is cut to one line;
///  * every adversarial-rejection `[NODE PHASE]` notice except the newest is cut.
/// Evidence the critic needs about other files is untouched.
pub(crate) fn condense_critic_evidence(evidence: &str) -> String {
    struct Span {
        start: usize,
        end: usize,
        key: String,
    }

    let lines: Vec<&str> = evidence.lines().collect();
    let mut reads: Vec<Span> = Vec::new();
    let mut vetoes: Vec<Span> = Vec::new();
    let mut notices: Vec<usize> = Vec::new();
    let mut last_write: std::collections::HashMap<String, usize> = std::collections::HashMap::new();

    let mut i = 0usize;
    while i < lines.len() {
        let line = lines[i];
        if let Some(rest) = line.strip_prefix("[WRITE_OK] ") {
            last_write.insert(rest.trim().to_string(), i);
        }
        if line.starts_with("[NODE PHASE] The artifact `")
            && line.contains("rejected by the Adversarial Critic")
        {
            notices.push(i);
        }
        let is_read = line.starts_with("[READ_OK] ");
        let is_veto = line.starts_with("[CRITIC_VETO]");
        if is_read || is_veto {
            let mut j = i + 1;
            while j < lines.len() && !critic_marker_line(lines[j]) {
                j += 1;
            }
            let key = line.trim_start_matches("[READ_OK] ").trim().to_string();
            let span = Span { start: i, end: j, key };
            if is_read {
                reads.push(span);
            } else {
                vetoes.push(span);
            }
            i = j;
        } else {
            i += 1;
        }
    }

    let mut elide: std::collections::HashMap<usize, (usize, String)> =
        std::collections::HashMap::new();

    for (idx, span) in reads.iter().enumerate() {
        let re_read = reads[idx + 1..].iter().any(|later| later.key == span.key);
        let rewritten = last_write.get(&span.key).map_or(false, |w| *w > span.start);
        if re_read || rewritten {
            elide.insert(
                span.start,
                (
                    span.end,
                    format!(
                        "[READ_OK] {} (older read elided: that file was re-read or rewritten later; judge the newest evidence)",
                        span.key
                    ),
                ),
            );
            if span.end < lines.len()
                && lines[span.end].starts_with(&format!("[PROPOSITION] {} ->", span.key))
            {
                elide.insert(
                    span.end,
                    (span.end + 1, "[PROPOSITION] (older read elided)".to_string()),
                );
            }
        }
    }
    for (idx, span) in vetoes.iter().enumerate() {
        if idx + 1 < vetoes.len() {
            let first: String = lines[span.start].chars().take(160).collect();
            elide.insert(span.start, (span.end, format!("{} ...(older veto elided)", first)));
        }
    }
    for (idx, line_no) in notices.iter().enumerate() {
        if idx + 1 < notices.len() {
            elide.insert(
                *line_no,
                (*line_no + 1, "[NODE PHASE] (older rejection notice elided)".to_string()),
            );
        }
    }

    let mut out = String::with_capacity(evidence.len());
    let mut k = 0usize;
    while k < lines.len() {
        if let Some((end, stub)) = elide.get(&k) {
            out.push_str(stub);
            out.push('\n');
            k = *end;
        } else {
            out.push_str(lines[k]);
            out.push('\n');
            k += 1;
        }
    }
    out
}

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

/// The pre-recon veto override must fire only when a critique makes BOTH
/// a state claim (the evidence already contains what the write needs) AND
/// a sufficiency claim (no further recon is required). Either alone is
/// insufficient: a critique that prescribes `read_file X before writing Y`
/// is a directive for MORE work, not a false claim that work is done.
///
/// Pre-fix, any mention of `read_file`, `search_code`, `prior evidence`,
/// `before writing`, `information gathering`, or `without prior evidence`
/// was sufficient to fire the override. Every observed firing across two
/// runs was a false positive.
fn is_pre_recon_override(critic_text: &str) -> bool {
    (critic_text.contains("already read")
        || critic_text.contains("already gathered")
        || critic_text.contains("already present")
        || critic_text.contains("evidence already contains"))
        && (critic_text.contains("no further")
            || critic_text.contains("not needed")
            || critic_text.contains("sufficient"))
}

#[cfg(test)]
mod pre_recon_override_tests {
    use super::is_pre_recon_override;

    #[test]
    fn prescription_for_more_reads_does_not_fire_override() {
        let critique = "without a complete grep -n output or read_file of \
                        those specific files, these citations are unverified.";
        assert!(!is_pre_recon_override(critique));
    }

    #[test]
    fn state_claim_alone_does_not_fire_override() {
        let critique = "the proposed write lacks prior evidence for its claims.";
        assert!(!is_pre_recon_override(critique));
    }

    #[test]
    fn sufficiency_claim_alone_does_not_fire_override() {
        let critique = "the report should be longer; it is not sufficient yet.";
        assert!(!is_pre_recon_override(critique));
    }

    #[test]
    fn contradictory_sufficiency_claim_fires_override() {
        let critique = "you have already read all the source files; \
                        no further evidence is needed.";
        assert!(is_pre_recon_override(critique));
    }

    #[test]
    fn both_claims_in_any_order_fire() {
        assert!(is_pre_recon_override(
            "sufficient evidence is present; you have already gathered it"
        ));
    }
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

#[cfg(test)]
mod critic_condense_tests {
    use super::{condense_critic_evidence, verification_read_pending};

    #[test]
    fn verification_read_is_pending_after_each_rewrite() {
        let p = "r.md";
        assert!(!verification_read_pending("", p));
        assert!(verification_read_pending("[WRITE_OK] r.md\n", p));
        assert!(!verification_read_pending("[WRITE_OK] r.md\n[READ_OK] r.md\n", p));
        // The last-run bug: rewrite after a read-back must need a new read.
        assert!(verification_read_pending("[WRITE_OK] r.md\n[READ_OK] r.md\n[WRITE_OK] r.md\n", p));
        assert!(!verification_read_pending("[READ_OK] r.md\n", p));
    }

    #[test]
    fn newest_read_of_a_path_survives_and_older_is_elided() {
        let ev = "[READ_OK] a.md\nFULL FILE CONTENT:\nold body\n[PROPOSITION] a.md -> File read (36 lines)\n[CMD_OK] Command: ls\n[READ_OK] a.md\nFULL FILE CONTENT:\nnew body\n";
        let out = condense_critic_evidence(ev);
        assert!(!out.contains("old body"), "{out}");
        assert!(!out.contains("36 lines"), "{out}");
        assert!(out.contains("new body"), "{out}");
        assert!(out.contains("[CMD_OK] Command: ls"), "{out}");
    }

    #[test]
    fn read_before_a_later_write_is_elided() {
        let ev = "[READ_OK] a.md\nFULL FILE CONTENT:\nstale draft\n[WRITE_OK] a.md\n";
        let out = condense_critic_evidence(ev);
        assert!(!out.contains("stale draft"), "{out}");
        assert!(out.contains("[WRITE_OK] a.md"), "{out}");
    }

    #[test]
    fn only_the_newest_veto_keeps_its_body() {
        let ev = "[CRITIC_VETO] first veto\nold detail\n[CMD_OK] Command: ls\n[CRITIC_VETO] second veto\nnew detail\n";
        let out = condense_critic_evidence(ev);
        assert!(!out.contains("old detail"), "{out}");
        assert!(out.contains("older veto elided"), "{out}");
        assert!(out.contains("new detail"), "{out}");
    }

    #[test]
    fn other_paths_and_lowercase_brackets_are_untouched() {
        let ev = "[READ_OK] a.md\nbody a\n[READ_OK] b.md\n[workspace]\nmembers = []\n";
        let out = condense_critic_evidence(ev);
        assert!(out.contains("body a"), "{out}");
        assert!(out.contains("[workspace]"), "{out}");
        assert!(out.contains("members = []"), "{out}");
    }
}
