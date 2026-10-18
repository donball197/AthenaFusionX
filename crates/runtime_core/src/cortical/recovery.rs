//! Strategist recovery paths for the Cortex state machine.
//!
//! Extracted from `handle_strategize`. Each recovery observes a specific
//! failure mode recorded in session evidence and either pivots the active
//! strategy or injects a competing hypothesis. Behavior is unchanged.

use crate::cognition::epistemic_session::EpistemicSession;
use crate::cortical_loop::CorticalLoop;

impl CorticalLoop {
    /// AST-UNADDRESSABLE RECOVERY (Tree-sitter body missing).
    ///
    /// When PatchFunction fails because the symbol has no Tree-sitter body,
    /// demote the dead patch_function hypothesis and pivot to read_file of
    /// the exact path named in the strategy. Do not guess a fallback path.
    /// Do not synthesize replace_lines until READ_OK evidence exists.
    pub(crate) fn apply_ast_unaddressable_recovery(
        &self,
        session: &mut EpistemicSession,
        active_strategy: &mut String,
    ) {
        let patch_function_strategy = active_strategy.starts_with("patch_function:");
        // When PatchFunction fails because the symbol has no Tree-sitter body,
        // demote the dead patch_function hypothesis and pivot to read_file of
        // the exact path named in the strategy. Do not guess a fallback path.
        // Do not synthesize replace_lines until READ_OK evidence exists.
        let tree_sitter_unaddressable = session.local_evidence.contains("has no Tree-sitter body")
            || session.local_evidence.contains("has no Tree-sitter body in Rust");

        if patch_function_strategy && tree_sitter_unaddressable {
            if let Some(h_id) = session.best_hypothesis_id {
                if session.graph.content_of(h_id).starts_with("patch_function:") {
                    session.graph.demote_hypothesis(h_id, 0.05);
                }
            }

            let path = active_strategy
                .strip_prefix("patch_function:")
                .and_then(|rest| rest.split(':').next())
                .map(str::trim)
                .filter(|p| !p.is_empty());

            if let Some(path) = path {
                let pivot_marker = format!("[AST_UNADDRESSABLE_PIVOTED]:{}", path);
                let already_pivoted = session.local_evidence.contains(&pivot_marker);

                let recon = format!("read_file:{}", path);
                let hid = session
                    .graph
                    .nodes
                    .iter()
                    .find(|(_, n)| n.content == recon)
                    .map(|(id, _)| *id)
                    .unwrap_or_else(|| {
                        session.graph.add_node(crate::cognition_graph::NodeKind::Hypothesis, &recon, 0.58, 100)
                    });

                // Always keep strategy/hypothesis coherent; log/mark only once.
                session.best_hypothesis_id = Some(hid);
                *active_strategy = recon;

                if !already_pivoted {
                    session.local_evidence.push('\n');
                    session.local_evidence.push_str(&pivot_marker);
                    session.local_evidence.push('\n');
                    self.log(
                        "\x1b[1;36m>> [CORTEX AST-UNADDRESSABLE RECOVERY]\x1b[0m \
                         Symbol has no Tree-sitter body; pivoting to read_file."
                            .to_string(),
                    );
                }
            } else {
                self.log(
                    "\x1b[1;31m>> [CORTEX AST-UNADDRESSABLE RECOVERY]\x1b[0m \
                     Could not parse path from patch_function strategy; recovery aborted."
                        .to_string(),
                );
            }
        }
    }
}

impl CorticalLoop {
    /// POST-READ MUTATION PIVOT.
    ///
    /// After READ_OK on a mutation mission, inject patch_function /
    /// replace_lines so strict `action_matches_strategy` does not reject
    /// mutation proposals while `active_strategy` is still `read_file`.
    ///
    /// Extracted from `handle_strategize`; behavior is unchanged.
    pub(crate) fn apply_post_read_mutation_pivot(
        &self,
        session: &mut EpistemicSession,
        active_strategy: &mut String,
        mission: &str,
        write_phase_ready: bool,
    ) {
        // After READ_OK on a mutation mission, inject patch_function / replace_lines
        // so strict action_matches_strategy does not reject mutation proposals
        // while active_strategy is still read_file.
        {
            let read_ok_done = session.local_evidence.contains("[READ_OK]")
                || session.local_evidence.contains("[READ_FUNCTION_OK]")
                || session.local_evidence.contains("[READ_CHUNK_OK]");
            let patch_marker_needed = session.contract.is_mutation_task
                || session.contract.conditions.iter().any(|c| {
                    let s = format!("{:?}", c);
                    s.contains("PatchMarker") || s.contains("MutationMarker")
                })
                || {
                    let m = mission.to_lowercase();
                    m.contains("patch")
                        || m.contains("modify ")
                        || m.contains("remove the unconditional")
                };
            let already_mutated = session.local_evidence.contains("[PATCH_FUNCTION_OK]")
                || session.local_evidence.contains("[DIFF_PATCH_OK]")
                || session.local_evidence.contains("[REPLACE_LINES_OK]")
                || session.local_evidence.contains("[WRITE_OK]");

            let read_only_mission = session.contract.read_only || {
                let m = mission.to_lowercase();
                m.contains("read-only")
                    || m.contains("do not modify")
                    || m.contains("don't modify")
            };

            if read_ok_done
                && patch_marker_needed
                && !already_mutated
                && !write_phase_ready
                && !read_only_mission
            {
                let mut target_path: Option<String> = None;
                for line in session.local_evidence.lines().rev() {
                    let t = line.trim();
                    if let Some(rest) = t.strip_prefix("[READ_OK]") {
                        let p = rest.split_whitespace().next().unwrap_or("").trim();
                        if !p.is_empty() && p.ends_with(".rs") {
                            target_path = Some(p.to_string());
                            break;
                        }
                    } else if let Some(rest) = t.strip_prefix("[READ_FUNCTION_OK]") {
                        let p = rest.trim().split(':').next().unwrap_or("").trim();
                        if !p.is_empty() && p.ends_with(".rs") {
                            target_path = Some(p.to_string());
                            break;
                        }
                    } else if let Some(rest) = t.strip_prefix("[READ_CHUNK_OK]") {
                        let p = rest.split_whitespace().next().unwrap_or("").trim();
                        if !p.is_empty() && p.ends_with(".rs") {
                            target_path = Some(p.to_string());
                            break;
                        }
                    }
                }
                if target_path.is_none() {
                    if let Some(rest) = active_strategy.strip_prefix("read_file:") {
                        let p = rest.trim();
                        if !p.is_empty() {
                            target_path = Some(p.to_string());
                        }
                    } else if let Some(rest) = active_strategy.strip_prefix("read_function:") {
                        let p = rest.trim().split(':').next().unwrap_or("").trim();
                        if !p.is_empty() {
                            target_path = Some(p.to_string());
                        }
                    } else if let Some(rest) = active_strategy.strip_prefix("read_file_chunk:") {
                        let p = rest.trim().split(':').next().unwrap_or("").trim();
                        if !p.is_empty() {
                            target_path = Some(p.to_string());
                        }
                    }
                }

                if let Some(ref path_str) = target_path {
                    let fn_name: Option<String> = {
                        let mut func_from_strategy = None;
                        if let Some(rest) = active_strategy.strip_prefix("read_function:") {
                            let f = rest.trim().split(':').nth(1).unwrap_or("").trim();
                            if !f.is_empty() {
                                func_from_strategy = Some(f.to_string());
                            }
                        }
                        if func_from_strategy.is_none() {
                            for line in session.local_evidence.lines().rev() {
                                if let Some(rest) = line.trim().strip_prefix("[READ_FUNCTION_OK]") {
                                    let f = rest.trim().split(':').nth(1).unwrap_or("").trim();
                                    if !f.is_empty() {
                                        func_from_strategy = Some(f.to_string());
                                        break;
                                    }
                                }
                            }
                        }

                        func_from_strategy.or_else(|| {
                            let lower = mission.to_lowercase();
                            let dead = |n: &str| {
                                session
                                    .local_evidence
                                    .contains(&format!("Symbol `{}` has no Tree-sitter body", n))
                            };
                            // Prefer a symbol actually defined in the file read.
                            let mut found: Option<String> = None;
                            for line in session.local_evidence.lines() {
                                if line.contains("Defined symbols:")
                                    && line.contains(path_str.as_str())
                                {
                                    for part in line.split("pub fn ").skip(1) {
                                        let name: String = part
                                            .chars()
                                            .take_while(|c| c.is_alphanumeric() || *c == '_')
                                            .collect();
                                        if !name.is_empty() && !dead(&name) {
                                            found = Some(name);
                                            break;
                                        }
                                    }
                                }
                                if found.is_some() {
                                    break;
                                }
                            }
                            found.or_else(|| {
                                ["evaluate", "check_contract"]
                                    .iter()
                                    .find(|c| lower.contains(**c) && !dead(c))
                                    .map(|s| (*s).to_string())
                            })
                        })
                    };

                    let mutation_strats: Vec<String> = match &fn_name {
                        Some(fn_name) => vec![
                            format!("patch_function:{}:{}", path_str, fn_name),
                            format!("replace_lines:{}", path_str),
                        ],
                        None => {
                            self.log(
                                "\x1b[1;31m>> [CORTEX POST-READ MUTATION PIVOT]\x1b[0m \
                                 No function name grounded in evidence or mission text; \
                                 offering path-grounded replace_lines only."
                                    .to_string(),
                            );
                            vec![format!("replace_lines:{}", path_str)]
                        }
                    };

                    let mut injected = 0usize;
                    for strat in &mutation_strats {
                        let already = session.graph.find_by_content(strat).is_some();
                        if already {
                            if injected == 0 {
                                let existing_id = session.graph.find_by_content(strat);
                                if let Some(id) = existing_id {
                                    session.graph.promote_hypothesis(id, 0.56);
                                    session.best_hypothesis_id = Some(id);
                                    *active_strategy = strat.clone();
                                    injected += 1;
                                    self.log(format!(
                                        "\x1b[1;36m>> [CORTEX POST-READ MUTATION PIVOT]\x1b[0m Promoted existing `{}`.",
                                        strat
                                    ));
                                }
                            }
                            continue;
                        }
                        let belief = if injected == 0 { 0.58 } else { 0.54 };
                        let hid = session.graph.add_node(crate::cognition_graph::NodeKind::Hypothesis, strat, belief, 100);
                        if injected == 0 {
                            session.best_hypothesis_id = Some(hid);
                            *active_strategy = strat.clone();
                            self.log(format!(
                                "\x1b[1;36m>> [CORTEX POST-READ MUTATION PIVOT]\x1b[0m Prefer `{}` after READ_OK on mutation mission.",
                                strat
                            ));
                        } else {
                            self.log(format!("\x1b[90m  -> Injected strategy: {}\x1b[0m", strat));
                        }
                        injected += 1;
                    }

                    if injected > 0 {
                        session.graph.cap_matching(
                            |c| c.starts_with("read_file:") && c.contains(path_str.as_str()),
                            0.20,
                        );
                    }
                }
            }
        }
    }
}

impl CorticalLoop {
    /// HALT-VETO RECOVERY (HaltDelivered strategy-prefix lock fix).
    ///
    /// When the contract is solely HaltDelivered and the critic has just
    /// vetoed a halt for lack of inspection evidence, the original
    /// "halt: ..." hypothesis becomes sticky. Demote it and inject a
    /// competing reconnaissance hypothesis so the node can escape.
    ///
    /// Extracted from `handle_strategize`; behavior is unchanged.
    pub(crate) fn apply_halt_veto_recovery(&self, session: &mut EpistemicSession) {
        //
        // When the contract is solely HaltDelivered and the critic has just
        // vetoed a halt for lack of inspection evidence, the original
        // "halt: ..." hypothesis becomes sticky. Demote it and inject a
        // competing reconnaissance hypothesis so the node can escape.
        {
            let is_halt_delivered =
                session.contract.conditions.iter().any(|c| {
                    matches!(c, crate::completion_engine::CompletionCondition::HaltDelivered)
                });
            let critic_demands_inspection =
                session.local_evidence.lines().rev().take(60).any(|l| {
                    let lower = l.to_lowercase();
                    (lower.contains("critic")
                        || lower.contains("veto")
                        || lower.contains("dialectic"))
                        && (lower.contains("no source")
                            || lower.contains("not inspected")
                            || lower.contains("no [read_ok]")
                            || lower.contains("no [search")
                            || lower.contains("must search")
                            || lower.contains("mandatory first step")
                            || lower.contains("zero inspection")
                            || lower.contains("locate and read")
                            || lower.contains("first locate")
                            || lower.contains("hallucination")
                            || lower.contains("evidentiary support"))
                });
            if is_halt_delivered && critic_demands_inspection {
                if let Some(h_id) = session.best_hypothesis_id {
                    if session.graph.content_of(h_id).trim().starts_with("halt:") {
                        session.graph.demote_hypothesis(h_id, 0.05);
                    }
                }
                let recon = "search_code:crates/runtime_core/src evaluate_contract OR mission_completion_authorized OR HaltDelivered";
                let already = session.graph.find_by_content(recon).is_some();
                if !already {
                    let hid = session.graph.add_node(
                        crate::cognition_graph::NodeKind::Hypothesis,
                        recon,
                        0.56,
                        100,
                    );
                    session.best_hypothesis_id = Some(hid);
                    self.log(
                        "\x1b[1;36m>> [CORTEX HALT-VETO RECOVERY]\x1b[0m Injected reconnaissance hypothesis after critic veto on HaltDelivered contract."
                            .to_string(),
                    );
                }
            }
        }
    }
}

impl CorticalLoop {
    /// POST-SEARCH EXPANSION (recon stall fix).
    ///
    /// After a successful SEARCH_CODE with MATCHES, inject read_file
    /// hypotheses for un-read hit paths so reconnaissance does not stall.
    ///
    /// Extracted from `handle_strategize`; behavior is unchanged.
    pub(crate) fn apply_post_search_expansion(
        &self,
        session: &mut EpistemicSession,
        active_strategy: &mut String,
        current_belief: f32,
        write_phase_ready: bool,
    ) {
        //
        // The deterministic parsing of SEARCH_CODE MATCHES evidence is
        // delegated to the independent reconnaissance algorithm.
        // Cortex remains the executive coordinator: it evaluates the
        // contract, creates hypotheses, assigns belief, and selects the
        // active strategy.
        if !write_phase_ready
            && session.local_evidence.contains("[SEARCH_CODE_OK]")
            && session.local_evidence.contains("MATCHES:")
        {
            let contract_met = crate::completion_engine::SubgoalCompletionEngine::evaluate(
                &session.contract,
                &session.local_evidence,
                current_belief,
            );

            if !contract_met {
                let unread: Vec<String> = nexus_reconnaissance_algorithms::ReconnaissancePlanner::plan(&session.local_evidence, 4)
                    .into_iter()
                    .filter_map(|strategy| strategy.strip_prefix("read_file:").map(str::to_owned))
                    .collect();

                if !unread.is_empty() {
                    for (i, path) in unread.iter().enumerate() {
                        let strat = format!("read_file:{}", path);
                        let already = session.graph.find_by_content(&strat).is_some();

                        if already {
                            continue;
                        }

                        let belief = if i == 0 { 0.56 } else { 0.53 };
                        let hid = session.graph.add_node(crate::cognition_graph::NodeKind::Hypothesis, &strat, belief, 100);

                        if i == 0 {
                            session.best_hypothesis_id = Some(hid);
                            *active_strategy = strat.clone();
                            self.log(format!(
                                "\x1b[1;36m>> [CORTEX POST-SEARCH EXPANSION]\x1b[0m Prefer `{}` (search hits not yet read).",
                                strat
                            ));
                        } else {
                            self.log(format!("\x1b[90m  -> Injected strategy: {}\x1b[0m", strat));
                        }
                    }
                }
            }
        }
    }
}

impl CorticalLoop {
/// Prevent a single failed action from driving the active hypothesis
    /// below the selection threshold. If the hypothesis was selectable
    /// before this evaluation (`pre_eval_belief >= floor`), clamp its
    /// post-evaluation score back up to the floor.
    ///
    /// This preserves anti-loop behavior -- repeated failures still decay
    /// the hypothesis through subsequent turns -- while ensuring a single
    /// `PATH_NOT_FOUND`, `[ACTION FAILED]`, or strongly negative LLM
    /// evaluation does not remove the hypothesis from the candidate set
    /// on the same turn the failure occurred. `select_best_hypothesis`
    /// filters hypotheses with `belief_score < 0.15`; without this floor
    /// one failed action can exhaust a node before it can try a different
    /// approach.
    ///
    /// The floor is not applied when the hypothesis was already below the
    /// threshold before this evaluation: a hypothesis that is already
    /// dead should not be resurrected by a single turn's accounting.
    pub(crate) fn floor_hypothesis_belief_after_failure(
        session: &mut EpistemicSession,
        h_id: uuid::Uuid,
        pre_eval_belief: f32,
        floor: f32,
    ) {
        session.graph.restore_floor(h_id, pre_eval_belief, floor);
    }

    /// Apply a direct decay to the active hypothesis's belief.
    ///
    /// This is the chokepoint for direct belief subtraction
    /// (`belief - delta`) used by strategy-violation and anti-loop
    /// pivots.
    ///
    /// F4: no floor clamp is applied. The belief may cross SELECT_FLOOR
    /// and become unselectable, which is exactly what forced-pivot
    /// callers intend. Decay never resurrects: it only floors at 0.0.
    /// The evaluation-failure rescue path remains
    /// `floor_hypothesis_belief_after_failure`.
    pub(crate) fn decay_hypothesis_belief(session: &mut EpistemicSession, h_id: uuid::Uuid, delta: f32) {
        let _ = session.graph.decay(h_id, delta);
    }
}
