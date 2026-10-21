//! CorticalLoop::handle_strategize (moved verbatim from cortical_loop.rs).

use super::*;

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
            .filter(|p| !crate::evidence_markers::has_artifact_write(&session.local_evidence, p))
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
            let target = &unwritten_targets[0];
            let strategy = format!("write_file:{}", target);

            self.log(format!(
                "\x1b[1;35m>> [CORTEX PHASE TRANSITION]\x1b[0m Reconnaissance verified; forcing native deliverable strategy `{}`.",
                strategy
            ));

            strategy
        } else if session.coverage.coverage_pct() >= 99.9
            && session.contract.read_only
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
            strategy
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
            if crate::cortical::util::should_synthesize_halt(&raw_proposal) {
                actions.push(AgentAction::Halt {
                    _thought_process: None,
                    reasoning: "Model declared task complete".to_string(),
                });
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

        if status == ContractStatus::Satisfied
            && unwritten_targets.is_empty()
            && !actions.iter().any(|a| matches!(a, AgentAction::Halt { _thought_process: None, .. }))
            && !session.local_evidence.contains("[HALT] Deterministic contract is satisfied.")
            // Never auto-halt while the read ledger still has an unread range:
            // a truncated first read satisfies ReadPathAcquired long before the
            // file has actually been read.
            && session
                .coverage
                .largest_interior_gap()
                .or_else(|| session.coverage.largest_gap())
                .is_none()
        {
            self.log("\x1b[1;36m>> [CORTEX] Contract satisfied; automatically injecting Halt action.\x1b[0m".to_string());
            actions = vec![AgentAction::Halt {
                _thought_process: None,
                reasoning: "Deterministic contract is satisfied.".to_string(),
            }];
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
fields. Do NOT repeat the rejected proposal verbatim. Use the key \"type\" for the action name and put ALL fields at the top level of the object (never nested under action_input or args).{}",
                raw_proposal.chars().take(500).collect::<String>(),
                schema_hint
            );
            session.local_evidence.push_str(&parse_fault_obs);
            session.local_evidence.push('\n');

            if let Some(h_id) = session.best_hypothesis_id {
                if let Some(new_belief) = session.graph.decay(h_id, 0.05) {
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
            let veto_recon_ok = session.local_evidence.contains("[CRITIC_VETO]")
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
            if !strategy_ok && !advances_unmet && !veto_recon_ok {
                self.log(format!(
                    "\x1b[1;31m>> [STRATEGY VIOLATION]\x1b[0m Selected strategy '{}' produced incompatible action '{:?}'. Rejecting proposal and forcing re-strategy.",
                    active_strategy,
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
}
