//! CorticalLoop::handle_evaluate (moved verbatim from cortical_loop.rs).

use super::*;
use uuid::Uuid;

impl CorticalLoop {
    pub(super) async fn handle_evaluate(
        &mut self,
        session: &mut EpistemicSession,
        _mission: &str,
        input: EvaluationInput,
    ) -> Result<CortexState> {
        let EvaluationInput {
            evidence,
            executed_action,
            belief,
            is_halt_action,
            mutation_path,
            typed_evidence,
        } = input;

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
        let is_read_evidence =
            evidence.contains("[READ_CHUNK_OK]") || evidence.contains("[READ_OK]");
        let is_search_evidence = evidence.contains("[SEARCH_CODE_OK]");
        let h_content = if is_halt_action && session.contract.read_only {
            "halt: deliver the complete read-only audit findings in the halt reasoning".to_string()
        } else if !is_halt_action && is_read_evidence && graph_h_content.starts_with("read_file:") {
            "read_file / read_file_chunk: acquire complete source evidence for the contract targets"
                .to_string()
        } else if !is_halt_action
            && is_search_evidence
            && (graph_h_content.starts_with("read_file:")
                || graph_h_content.starts_with("search_code:"))
        {
            "search_code: verify contract search targets".to_string()
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

        // HALT TURNS ONLY: a halt's own evidence is just the [HALT] line, so the
        // evaluator cannot see the success markers that justify it, and the HALT
        // RULE in its prompt ("this turn or earlier in context") never applies.
        // Append the recent success-marker lines from the session. Every other
        // turn keeps the strict current-turn-only boundary above.
        let eval_evidence_context = if is_halt_action {
            let support = crate::halt_support::success_marker_lines(&session.local_evidence, 6);
            if support.is_empty() {
                eval_evidence_context
            } else {
                format!(
                    "{}\nSESSION SUCCESS MARKERS (earlier in this node):\n{}",
                    eval_evidence_context, support
                )
            }
        } else {
            eval_evidence_context
        };

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
                        critique:
                            "Evaluator unavailable; recon evidence recorded with neutral modifier."
                                .to_string(),
                        confidence_modifier: 0.0,
                    };
                    session.record_evidence(&evidence, &neutral, h_id);
                }

                let typed_max_rel =
                    typed_evidence.iter().map(|r| r.reliability).fold(0.0_f32, f32::max);
                let (tag, mod_val) = if evidence.contains("[COMPILE_OK]")
                    || evidence.contains("[PATCH_FUNCTION_OK]")
                {
                    ("eval_err_fallback:compile_or_patch", 0.35)
                } else if ["[READ_CHUNK_OK]", "[READ_OK]", "[READ_FUNCTION_OK]", "[SEARCH_CODE_OK]"]
                    .iter()
                    .any(|m| evidence.contains(m))
                {
                    ("eval_err_fallback:recon", 0.0)
                } else if typed_max_rel >= 0.85 {
                    let fallback_eval = crate::epistemic_engine::CriticResponse {
                        critique: "Typed physical evidence verified; evaluator fallback applied."
                            .to_string(),
                        confidence_modifier: 0.20,
                    };
                    session.record_evidence(&evidence, &fallback_eval, h_id);
                    ("eval_err_fallback:typed_verified", 0.20)
                } else {
                    ("eval_err_fallback:unrecorded", 0.0)
                };
                let after =
                    session.graph.nodes.get(&h_id).map(|n| n.belief_score).unwrap_or(belief);
                append_eval_journal(
                    session.best_hypothesis_id,
                    &h_content,
                    tag.into(),
                    &evidence,
                    belief,
                    after,
                    mod_val,
                );
                Ok(CortexState::Strategize)
            }
            Ok(eval) => {
                if is_halt_action
                    && session
                        .local_evidence
                        .lines()
                        .any(|line| line.starts_with("[WRITE_CONFORMANCE_FAILED]"))
                {
                    self.log(
                        "\x1b[1;33m>> [CORTEX] HALT rejected: unresolved Python write-conformance failure.\x1b[0m"
                            .to_string(),
                    );
                    session.local_evidence.push_str(
                        "[HALT_REJECTED] A Python deliverable failed conformance. Correct the file and pass conformance verification before halting.\n",
                    );
                    append_eval_journal(
                        session.best_hypothesis_id,
                        &h_content,
                        "halt_rejected:write_conformance".into(),
                        &evidence,
                        belief,
                        belief,
                        eval.confidence_modifier,
                    );
                    return Ok(CortexState::Strategize);
                }

                // Search relevance must inspect the action that actually
                // executed, never the strategist's natural-language proposal.
                let executed_action_lower = executed_action.to_ascii_lowercase();
                let mut search_relevance_score: Option<f32> = None;
                let is_search_like = !executed_action_lower.contains("halt")
                    && (executed_action_lower.contains("search_code")
                        || executed_action_lower.contains("search_memory"));
                if is_search_like {
                    let query_hint = executed_action
                        .lines()
                        .filter_map(|line| serde_json::from_str::<AgentAction>(line).ok())
                        .filter_map(|action| match action {
                            AgentAction::SearchCode { query, .. }
                            | AgentAction::SearchMemory { query, .. } => Some(query),
                            _ => None,
                        })
                        .collect::<Vec<_>>()
                        .join("\n");

                    if query_hint.trim().is_empty() {
                        self.log(
                            "[SEARCH RELEVANCE] No query could be extracted from executed action JSON."
                                .to_string(),
                        );
                        session.record_evidence(&evidence, &eval, h_id);
                        let after = session
                            .graph
                            .nodes
                            .get(&h_id)
                            .map(|n| n.belief_score)
                            .unwrap_or(belief);
                        append_eval_journal(
                            session.best_hypothesis_id,
                            &h_content,
                            "search_empty_query".into(),
                            &evidence,
                            belief,
                            after,
                            eval.confidence_modifier,
                        );
                        return Ok(CortexState::Strategize);
                    }

                    let rel = score_search_relevance(&query_hint, &evidence);
                    search_relevance_score = Some(rel);
                    if !should_raise_belief(rel, 0.45) {
                        let mut eval = eval;
                        if eval.confidence_modifier > 0.0 {
                            eval.confidence_modifier = 0.0;
                        }
                        session.record_evidence(&evidence, &eval, h_id);
                        Self::floor_hypothesis_belief_after_failure(session, h_id, belief, 0.15);
                        let after = session
                            .graph
                            .nodes
                            .get(&h_id)
                            .map(|n| n.belief_score)
                            .unwrap_or(belief);
                        append_eval_journal(
                            session.best_hypothesis_id,
                            &h_content,
                            format!("low_relevance:{:.2}", rel),
                            &evidence,
                            belief,
                            after,
                            eval.confidence_modifier,
                        );
                        return Ok(CortexState::Strategize);
                    }
                }
                // PROGRESS FLOOR & SEARCH RELEVANCE BOOST:
                // 1) High-relevance search hits raise belief rather than only clamping down.
                // 2) If this action's evidence reduced unmet non-belief conditions, it is
                //    progress and may not be penalized.
                let mut eval = eval;
                if let Some(rel) = search_relevance_score {
                    if rel >= 0.70 {
                        let boosted = ((rel - 0.50) * 0.50).clamp(0.10, 0.25);
                        if boosted > eval.confidence_modifier {
                            self.log(format!(
                                "\x1b[1;36m>> [SEARCH RELEVANCE] High relevance ({:.2}); raising modifier {:.2} -> {:.2}\x1b[0m",
                                rel, eval.confidence_modifier, boosted
                            ));
                            eval.confidence_modifier = boosted;
                        }
                    }
                }
                {
                    use crate::completion_engine::CompletionCondition as CC;
                    let conds: Vec<CC> = session
                        .contract
                        .conditions
                        .iter()
                        .filter(|c| !matches!(c, CC::MinBeliefScore(_)))
                        .cloned()
                        .collect();
                    // Evidence may already have been appended by execute.rs.
                    // Recover a pre-action view by stripping one trailing copy of
                    // `evidence` when present; otherwise use local_evidence as-is
                    // and synthesize the after view.
                    let pre_view = {
                        let ev = evidence.as_str();
                        if let Some(pos) = session.local_evidence.rfind(ev) {
                            let mut s = session.local_evidence.clone();
                            s.replace_range(pos..pos + ev.len(), "");
                            s
                        } else {
                            session.local_evidence.clone()
                        }
                    };
                    let before = crate::outcome_verifier::OutcomeVerifier::unmet_conditions(
                        &pre_view, 1.0, &conds,
                    )
                    .len();
                    let after_current = crate::outcome_verifier::OutcomeVerifier::unmet_conditions(
                        &session.local_evidence,
                        1.0,
                        &conds,
                    )
                    .len();
                    let after_synth = if session.local_evidence.contains(evidence.as_str()) {
                        after_current
                    } else {
                        let after_ev = format!("{}\n{}", session.local_evidence, evidence);
                        crate::outcome_verifier::OutcomeVerifier::unmet_conditions(
                            &after_ev, 1.0, &conds,
                        )
                        .len()
                    };
                    let after = after_current.min(after_synth);
                    if after < before && eval.confidence_modifier < 0.10 {
                        self.log(format!(
                            "\x1b[1;36m>> [PROGRESS FLOOR] unmet {} -> {}; modifier {:.2} -> 0.10\x1b[0m",
                            before, after, eval.confidence_modifier
                        ));
                        eval.confidence_modifier = 0.10;
                    }
                }
                self.log(format!(
                    "\x1b[33m>> [EVALUATION]\x1b[0m {} (Modifier: {:.2})",
                    eval.critique, eval.confidence_modifier
                ));
                session.record_evidence(&evidence, &eval, h_id);
                let eval_after = session
                    .graph
                    .nodes
                    .get(&h_id)
                    .map(|n| n.belief_score)
                    .unwrap_or((belief + eval.confidence_modifier).clamp(0.0, 1.0));

                // Do NOT floor on the success path. restore_floor is reserved for
                // genuine failures (low-relevance branch above, parse faults, etc.).
                // Unconditional flooring here kept belief pinned near 0.15 despite
                // positive confidence modifiers in live runs.

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
                    if let Some(pos) = session.local_evidence.rfind(evidence.as_str()) {
                        session.local_evidence.replace_range(pos..pos + evidence.len(), "");
                    }
                    session.local_evidence.push_str("\n[HALT_REJECTED] You cannot halt. The required output deliverable has not been written yet. You MUST write the final file now.\n");
                    append_eval_journal(
                        session.best_hypothesis_id,
                        &h_content,
                        "halt_rejected:unwritten_artifacts".into(),
                        &evidence,
                        belief,
                        eval_after,
                        eval.confidence_modifier,
                    );
                    return Ok(CortexState::Strategize);
                }

                // DETERMINISTIC HALT-CLAIM GATE (read-only audits): every backticked
                // symbol in the report must exist in evidence actually read.
                if is_halt_action && session.contract.read_only {
                    let corpus = crate::cortical::halt_claims::verification_corpus(
                        &session.local_evidence,
                        &evidence,
                    );
                    let bad =
                        crate::cortical::halt_claims::unverified_symbols(&executed_action, &corpus);
                    if !bad.is_empty() {
                        self.log(format!(
                            "\x1b[1;31m>> [HALT CLAIM GATE] Unverified symbols in report: {:?}. Rejecting halt.\x1b[0m",
                            bad
                        ));
                        if let Some(pos) = session.local_evidence.rfind(evidence.as_str()) {
                            session.local_evidence.replace_range(pos..pos + evidence.len(), "");
                        }
                        session.local_evidence.push_str(&format!(
                            "\n[HALT_REJECTED_SYMBOLS] Report named symbols not found in any file content read: {}. Re-read the relevant ranges and cite only functions that appear in the evidence.\n",
                            bad.join(", ")
                        ));
                        append_eval_journal(
                            session.best_hypothesis_id,
                            &h_content,
                            "halt_rejected:unverified_symbols".into(),
                            &evidence,
                            belief,
                            eval_after,
                            eval.confidence_modifier,
                        );
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
                        if let Some(pos) = session.local_evidence.rfind(evidence.as_str()) {
                            session.local_evidence.replace_range(pos..pos + evidence.len(), "");
                        }
                        session.local_evidence.push_str(&format!(
                            "\n[HALT_REJECTED_REPORT] The halt `reasoning` field IS the deliverable. Put the full audit there: each large function by name with its line range, plus concrete findings. Defects: {}\n",
                            defects.join("; ")
                        ));
                        append_eval_journal(
                            session.best_hypothesis_id,
                            &h_content,
                            "halt_rejected:report_defects".into(),
                            &evidence,
                            belief,
                            eval_after,
                            eval.confidence_modifier,
                        );
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

                // Physical-only completion: allow halt when all non-belief conditions
                // are met even if MinBeliefScore is unmet.
                let physical_completion_ready = is_halt_action
                    && !deterministic_contract_satisfied
                    && crate::outcome_verifier::OutcomeVerifier::physical_contract_satisfied(
                        &session.local_evidence,
                        current_belief,
                        &session.contract.conditions,
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
                    append_eval_journal(
                        session.best_hypothesis_id,
                        &h_content,
                        executed_action.chars().take(80).collect(),
                        &evidence,
                        belief,
                        current_belief.max(0.95),
                        eval.confidence_modifier,
                    );

                    return Ok(CortexState::Complete {
                        belief: current_belief.max(0.95),
                        evidence: session.collect_evidence(h_id),
                    });
                }

                if is_halt_action
                    && (deterministic_contract_satisfied || physical_completion_ready)
                    && (eval.confidence_modifier >= 0.0 || session.contract.read_only)
                {
                    let strategy_for_verification = format!(
                        "MISSION OBJECTIVE:\n{}\n\nCURRENT STRATEGY:\n{}",
                        session.contract.objective, h_content
                    );

                    self.log(
                        "\x1b[1;35m>> [CORTEX] HALT proposed. Running node-level epistemic verification...\x1b[0m"
                            .to_string(),
                    );

                    let sanitized_history = {
                        let mut kept = Vec::new();
                        let mut in_halt = false;
                        for line in session.local_evidence.lines() {
                            let t = line.trim_start();
                            if t.starts_with("[HALT]") {
                                in_halt = true;
                                continue;
                            }
                            if in_halt {
                                if t.starts_with('[')
                                    && t.find(']').is_some_and(|i| {
                                        i >= 3
                                            && t[1..i].chars().all(|c| {
                                                c.is_ascii_uppercase()
                                                    || c.is_ascii_digit()
                                                    || matches!(c, '_' | ' ' | '-' | ':')
                                            })
                                    })
                                {
                                    in_halt = false;
                                } else {
                                    continue;
                                }
                            }
                            if !line.contains("[HALT_REJECTED")
                                && !line.contains("[LOOP_VETO]")
                                && !line.contains("[CRITIC_REPEAT_VETO]")
                            {
                                kept.push(line);
                            }
                        }
                        kept.join("\n")
                    };

                    let verification_evidence = format!(
                        "CURRENT-TURN PHYSICAL EVIDENCE:\n{}\n\n                         HISTORICAL TASK EVIDENCE (CONTEXT ONLY):\n{}",
                        evidence, sanitized_history
                    );
                    // A write-artifact mission is judged on the artifact itself: the evidence
                    // otherwise holds only `[WRITE_OK] <path>`, never the content.
                    let verification_evidence = {
                        let mut v = verification_evidence;
                        for p in &session.contract.required_artifact_paths {
                            if let Ok(body) = std::fs::read_to_string(&p.path) {
                                let body: String = body.chars().take(12_000).collect();
                                v.push_str(&format!(
                                    "\n\nARTIFACT CONTENT ({}):\n{}",
                                    p.path, body
                                ));
                            }
                        }
                        v
                    };

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
                            let final_belief = belief.max(report.evidence_confidence).max(0.95);
                            append_eval_journal(
                                session.best_hypothesis_id,
                                &h_content,
                                executed_action.chars().take(80).collect(),
                                &evidence,
                                belief,
                                final_belief,
                                eval.confidence_modifier,
                            );

                            return Ok(CortexState::Complete {
                                belief: final_belief,
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
                            if let Some(pos) = session.local_evidence.rfind(evidence.as_str()) {
                                session.local_evidence.replace_range(pos..pos + evidence.len(), "");
                            }

                            session.local_evidence.push_str(&format!(
                                "\n[HALT_REJECTED] Mission verification failed: {}\n",
                                report.rationale
                            ));
                            append_eval_journal(
                                session.best_hypothesis_id,
                                &h_content,
                                "halt_rejected:verify_outcome".into(),
                                &evidence,
                                belief,
                                eval_after,
                                eval.confidence_modifier,
                            );

                            return Ok(CortexState::Strategize);
                        }

                        Err(e) => {
                            self.log(format!(
                                "\x1b[1;33m>> [CORTEX] HALT verification unavailable: {}. Continuing mission instead of terminating.\x1b[0m",
                                e
                            ));
                            if let Some(pos) = session.local_evidence.rfind(evidence.as_str()) {
                                session.local_evidence.replace_range(pos..pos + evidence.len(), "");
                            }
                            append_eval_journal(
                                session.best_hypothesis_id,
                                &h_content,
                                "halt_rejected:verify_err".into(),
                                &evidence,
                                belief,
                                eval_after,
                                eval.confidence_modifier,
                            );

                            return Ok(CortexState::Strategize);
                        }
                    }
                }

                if is_halt_action && !deterministic_contract_satisfied && !physical_completion_ready
                {
                    let unmet = crate::outcome_verifier::OutcomeVerifier::unmet_conditions(
                        &session.local_evidence,
                        current_belief,
                        &session.contract.conditions,
                    );
                    self.log(format!(
                        "\x1b[1;33m>> [CORTEX] HALT rejected: deterministic completion contract is not satisfied. Unmet: {:?}\x1b[0m",
                        unmet
                    ));
                    if let Some(pos) = session.local_evidence.rfind(evidence.as_str()) {
                        session.local_evidence.replace_range(pos..pos + evidence.len(), "");
                    }
                    session.local_evidence.push_str(&format!(
                        "\n[HALT_REJECTED] Deterministic completion contract not satisfied. Unmet: {:?}\n",
                        unmet
                    ));
                    append_eval_journal(
                        session.best_hypothesis_id,
                        &h_content,
                        "halt_rejected:contract_unmet".into(),
                        &evidence,
                        belief,
                        eval_after,
                        eval.confidence_modifier,
                    );
                    return Ok(CortexState::Strategize);
                }

                if eval.confidence_modifier < 0.0 {
                    if let Some(path) = &mutation_path {
                        // argv form: a path containing shell metacharacters
                        // cannot be interpreted as a second command.
                        let revert_ok = crate::revert::revert_mutation_path(path).is_ok();

                        if revert_ok {
                            // Only on a confirmed successful revert may the
                            // evidence and the active mutation-path entry be
                            // discarded. A failed revert leaves the source
                            // mutated on disk; removing the evidence would
                            // erase the only record that a mutation occurred.
                            session.active_mutation_paths.retain(|p| p != path);
                            if let Some(pos) = session.local_evidence.rfind(evidence.as_str()) {
                                session.local_evidence.replace_range(pos..pos + evidence.len(), "");
                            }
                        } else {
                            session.local_evidence.push_str(
                                "[NEGATIVE_CONFIDENCE_REVERT_FAILED] Mutation revert did not succeed. Evidence preserved and active mutation path retained.\n",
                            );
                        }
                    }
                }

                let final_after =
                    session.graph.nodes.get(&h_id).map(|n| n.belief_score).unwrap_or(eval_after);
                append_eval_journal(
                    session.best_hypothesis_id,
                    &h_content,
                    executed_action.chars().take(80).collect(),
                    &evidence,
                    belief,
                    final_after,
                    eval.confidence_modifier,
                );

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

fn append_eval_journal(
    h_id: Option<Uuid>,
    strategy: &str,
    action_type: String,
    evidence: &str,
    belief_before: f32,
    belief_after: f32,
    eval_modifier: f32,
) {
    crate::journal::append_global(crate::journal::TurnRecord {
        node_id: journal_node_id(h_id),
        turn: 0,
        strategy: strategy.to_string(),
        action_type,
        evidence_hash: crate::journal::hash_bytes(evidence.as_bytes()),
        evidence_len: evidence.len(),
        belief_before,
        belief_after,
        eval_modifier,
        unmet_before: Vec::new(),
        unmet_after: Vec::new(),
        provider: None,
        latency_ms: None,
        prev_hash: String::new(),
        record_hash: String::new(),
    });
}

fn journal_node_id(h_id: Option<Uuid>) -> String {
    h_id.map(|id| format!("{:?}", id)).unwrap_or_else(|| "none".to_string())
}

#[cfg(test)]
mod eval_journal_tests {
    use super::journal_node_id;
    use uuid::Uuid;

    #[test]
    fn none_and_nil_uuid_produce_distinct_journal_node_ids() {
        assert_ne!(journal_node_id(None), journal_node_id(Some(Uuid::nil())));
        assert_eq!(journal_node_id(None), "none");
    }
}
