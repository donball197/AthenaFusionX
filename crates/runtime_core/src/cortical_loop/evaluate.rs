//! CorticalLoop::handle_evaluate (moved verbatim from cortical_loop.rs).

use super::*;

impl CorticalLoop {
    pub(super) async fn handle_evaluate(
        &mut self,
        session: &mut EpistemicSession,
        mission: &str,
        input: EvaluationInput,
    ) -> Result<CortexState> {
        let EvaluationInput { evidence, executed_action, belief, is_halt_action, mutation_path } =
            input;

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
                    return Ok(CortexState::Strategize);
                }

                // Search relevance must inspect the action that actually
                // executed, never the strategist's natural-language proposal.
                let executed_action_lower = executed_action.to_ascii_lowercase();
                let is_search_like = executed_action_lower.contains("search_code")
                    || executed_action_lower.contains("search_memory");
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
                        return Ok(CortexState::Strategize);
                    }

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

                    let sanitized_history = session
                        .local_evidence
                        .lines()
                        .filter(|line| {
                            !line.contains("[HALT_REJECTED")
                                && !line.contains("[LOOP_VETO]")
                                && !line.contains("[CRITIC_REPEAT_VETO]")
                        })
                        .collect::<Vec<_>>()
                        .join("\n");

                    let verification_evidence = format!(
                        "CURRENT-TURN PHYSICAL EVIDENCE:\n{}\n\n                         HISTORICAL TASK EVIDENCE (CONTEXT ONLY):\n{}",
                        evidence, sanitized_history
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
