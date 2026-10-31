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
        if status == ContractStatus::Satisfied
            && unwritten_targets.is_empty()
            && !actions.iter().any(|a| matches!(a, AgentAction::Halt { _thought_process: None, .. }))
            && !session.local_evidence.contains("[HALT] Deterministic contract is satisfied.")
            // Once the report gate has rejected a bare halt, re-injecting it is
            // futile and would replace every later strategist action.
            && !session.local_evidence.contains("[HALT_REJECTED_REPORT]")
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
                reasoning: auto_halt_report(&session.local_evidence),
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
            // A rejected halt report means the strategist has been told its findings are
            // too thin: more non-mutating reconnaissance is then legitimate even when it
            // is outside the pinned strategy and the (possibly under-specified)
            // contract.
            let veto_recon_ok = (session.local_evidence.contains("[CRITIC_VETO]")
                || session.local_evidence.contains("[HALT_REJECTED_REPORT]"))
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
            if !strategy_ok
                && !advances_unmet
                && !veto_recon_ok
                && !dir_child_read_ok
                && !mandatory_pivot_ok
                && !audit_read_pivot_ok
            {
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

/// A pinned `read_file:<dir>` strategy can only ever yield a directory listing,
/// so it is operationalized by reading files INSIDE that directory. Admit a
/// read_file / read_file_chunk whose path is strictly under the pinned
/// directory (component-wise, no `..`). Every other mismatch still violates.
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

/// Build the auto-injected halt report from the satisfied command evidence so
/// it carries real findings instead of a bare one-liner. No backticks and no
/// line-range text, so the symbol and range checks have nothing to verify.
fn auto_halt_report(evidence: &str) -> String {
    let mut status = String::from("unknown status");
    let mut hash = String::new();
    let mut stdout = String::new();
    let mut in_stdout = false;
    for line in evidence.lines() {
        let t = line.trim_start();
        if t.starts_with("[CMD_OK] Command:") {
            status = String::from("unknown status");
            hash.clear();
            stdout.clear();
            in_stdout = false;
        } else if let Some(s) = t.strip_prefix("Status:") {
            status = s.trim().to_string();
        } else if let Some(h) = t.strip_prefix("[CMD_HASH]") {
            hash = h.trim().to_string();
        } else if t == "STDOUT:" {
            in_stdout = true;
        } else if in_stdout && t.starts_with('[') && t.contains(']') {
            in_stdout = false;
        } else if in_stdout {
            stdout.push_str(line);
            stdout.push_str(" | ");
        }
    }
    let excerpt: String = stdout.replace('`', "").chars().take(1200).collect();
    format!(
        "Deterministic contract satisfied: the required read-only command executed with {}. Command hash {}. Captured stdout: {}. This report covers only that command's output and does not establish any other audit criterion.",
        status, hash, excerpt
    )
}

#[cfg(test)]
mod auto_halt_report_tests {
    use super::auto_halt_report;

    #[test]
    fn report_is_substantive_and_clean() {
        let ev = "[CMD_OK] Command: echo hi\nStatus: exit status: 0\n[CMD_HASH] abc123\nSTDOUT:\nROOT=/x\nBRANCH=dev\n";
        let r = auto_halt_report(ev);
        assert!(r.chars().count() >= 50);
        assert!(r.ends_with('.'));
        assert!(r.contains("ROOT=/x") && r.contains("abc123"));
        assert!(!r.contains('`'));
    }
}
