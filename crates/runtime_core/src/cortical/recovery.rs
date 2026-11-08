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
                        session.graph.add_node(
                            crate::cognition_graph::NodeKind::Hypothesis,
                            &recon,
                            0.58,
                            100,
                        )
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
        // GUARD: Do not override a strategy whose target is the mission's
        // required artifact. A mission that names an output file is a
        // CREATE, not a mutation of the read files.
        {
            let names_required_artifact = active_strategy
                .strip_prefix("write_file:")
                .map(str::trim)
                .is_some_and(|target| {
                    session
                        .contract
                        .required_artifact_paths
                        .iter()
                        .any(|a| a.path == target)
                });
            if names_required_artifact {
                return;
            }
        }

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
                    crate::cortical::read_only_gate::contains_word_start(&m, "patch")
                        || m.contains("modify ")
                        || m.contains("remove the unconditional")
                };
            let already_mutated = session.local_evidence.contains("[PATCH_FUNCTION_OK]")
                || session.local_evidence.contains("[DIFF_PATCH_OK]")
                || session.local_evidence.contains("[REPLACE_LINES_OK]")
                || session.local_evidence.contains("[WRITE_OK]");

            let read_only_mission = session.contract.read_only
                || crate::cortical::read_only_gate::mission_read_only_intent(mission);

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
                        let hid = session.graph.add_node(
                            crate::cognition_graph::NodeKind::Hypothesis,
                            strat,
                            belief,
                            100,
                        );
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
        // GUARD: Do not override a strategy whose target is the mission's
        // required artifact. A mission that names an output file is a
        // CREATE, not a mutation of the read files.
        {
            let names_required_artifact = active_strategy
                .strip_prefix("write_file:")
                .map(str::trim)
                .is_some_and(|target| {
                    session
                        .contract
                        .required_artifact_paths
                        .iter()
                        .any(|a| a.path == target)
                });
            if names_required_artifact {
                return;
            }
        }

        //
        // The deterministic parsing of SEARCH_CODE MATCHES evidence is
        // delegated to the independent reconnaissance algorithm.
        // Cortex remains the executive coordinator: it evaluates the
        // contract, creates hypotheses, assigns belief, and selects the
        // active strategy.
        // Do not let post-search expansion pivot to read_file while a
        // path-bound SearchTermAcquired/SearchMarkerAcquired condition is
        // still unmet. Otherwise a `crates` search injects a read_file
        // strategy that then blocks the required `bins` search with a
        // STRATEGY VIOLATION.
        let has_unmet_search = session.contract.conditions.iter().any(|c| {
            matches!(
                c,
                crate::completion_engine::CompletionCondition::SearchTermAcquired { .. }
                    | crate::completion_engine::CompletionCondition::SearchCheckedAcquired { .. }
                    | crate::completion_engine::CompletionCondition::SearchMarkerAcquired
            ) && !crate::cortical::contract_advance::condition_is_met(
                c,
                &session.local_evidence,
            )
        });
        if has_unmet_search {
            return;
        }

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
                let unread: Vec<String> =
                    nexus_reconnaissance_algorithms::ReconnaissancePlanner::plan(
                        &session.local_evidence,
                        4,
                    )
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
                        let hid = session.graph.add_node(
                            crate::cognition_graph::NodeKind::Hypothesis,
                            &strat,
                            belief,
                            100,
                        );

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
    pub(crate) fn decay_hypothesis_belief(
        session: &mut EpistemicSession,
        h_id: uuid::Uuid,
        delta: f32,
    ) {
        let _ = session.graph.decay(h_id, delta);
    }
}

// ---------------------------------------------------------------------------
// PATCH -> VERIFY phase for compound [PatchMarkerAcquired + CommandExecutionVerified]
// contracts ("run X, patch the file so X passes, re-run X").
//
// A pinned `execute_command:` strategy narrows the strategist to exactly one
// tool (see tools::schema::tool_declarations_for_strategy), so it can never emit
// the patch the contract also requires. The phase is derived from physical
// evidence only: the latest traceback grounds the patch target, a patch marker
// moves the node to verification, and a fresh failure after the patch moves it
// back to patching.
// ---------------------------------------------------------------------------

#[derive(Debug, PartialEq)]
pub(crate) enum PatchCommandPhase {
    /// Nothing grounded to patch yet: let the pinned command strategy reproduce.
    Reproduce,
    /// Patch the file named by the latest traceback.
    Patch { path: String, function: Option<String> },
    /// A patch is in place: (re-)run the required command.
    Verify,
}

fn marker_token(line: &str) -> Option<&str> {
    line.trim_start().strip_prefix('[').and_then(|r| r.split(']').next())
}

fn relativize_to_cwd(p: &str) -> String {
    std::env::current_dir()
        .ok()
        .and_then(|cwd| {
            std::path::Path::new(p).strip_prefix(&cwd).ok().map(|r| r.to_string_lossy().to_string())
        })
        .unwrap_or_else(|| p.to_string())
}

/// Innermost non-stdlib frame of the latest Python-style traceback:
/// `File "<path>", line N, in <function>`. The function is optional
/// (module-level frames and SyntaxErrors have none).
pub(crate) fn grounded_patch_target(evidence: &str) -> Option<(String, Option<String>)> {
    for line in evidence.lines().rev() {
        let Some(rest) = line.trim_start().strip_prefix("File \"") else {
            continue;
        };
        let Some((path, tail)) = rest.split_once('"') else {
            continue;
        };
        if path.starts_with('<')
            || path.contains("site-packages")
            || path.contains("dist-packages")
            || path.starts_with("/usr/lib")
            || path.starts_with("/usr/local/lib")
        {
            continue;
        }
        let function = tail
            .rsplit_once(", in ")
            .map(|(_, n)| n.trim())
            .filter(|n| !n.is_empty() && n.chars().all(|c| c.is_alphanumeric() || c == '_'))
            .map(|n| n.to_string());
        return Some((relativize_to_cwd(path), function));
    }
    None
}

pub(crate) fn patch_command_phase(evidence: &str, patch_done: bool) -> PatchCommandPhase {
    let lines: Vec<&str> = evidence.lines().collect();
    let last_patch = lines.iter().rposition(|l| {
        matches!(marker_token(l), Some("PATCH_FUNCTION_OK" | "DIFF_PATCH_OK" | "REPLACE_LINES_OK"))
    });
    let last_fail = lines.iter().rposition(|l| l.trim_start().starts_with("[ACTION FAILED]"));
    let failed_after_patch = match (last_patch, last_fail) {
        (Some(p), Some(f)) => f > p,
        _ => false,
    };
    let target = grounded_patch_target(evidence);
    if !patch_done {
        return match target {
            Some((path, function)) => PatchCommandPhase::Patch { path, function },
            None => PatchCommandPhase::Reproduce,
        };
    }
    if failed_after_patch {
        if let Some((path, function)) = target {
            return PatchCommandPhase::Patch { path, function };
        }
    }
    PatchCommandPhase::Verify
}

impl crate::cortical_loop::CorticalLoop {
    pub(crate) fn apply_patch_command_phase(
        &self,
        session: &mut crate::cognition::epistemic_session::EpistemicSession,
        active_strategy: &mut String,
        mission: &str,
        write_phase_ready: bool,
    ) {
        use crate::completion_engine::CompletionCondition as CC;

        if write_phase_ready
            || session.contract.read_only
            || session.compile_remediation.is_active()
        {
            return;
        }
        let m = mission.to_lowercase();
        if m.contains("read-only") || m.contains("do not modify") || m.contains("don't modify") {
            return;
        }
        if !session.contract.conditions.iter().any(|c| matches!(c, CC::PatchMarkerAcquired)) {
            return;
        }
        let Some(command) = session.contract.conditions.iter().find_map(|c| match c {
            CC::CommandExecutionVerified { command } => Some(command.clone()),
            _ => None,
        }) else {
            return;
        };
        let command_cond = CC::CommandExecutionVerified { command: command.clone() };
        if crate::cortical::contract_advance::condition_is_met(
            &command_cond,
            &session.local_evidence,
        ) {
            return;
        }
        let patch_done = crate::cortical::contract_advance::condition_is_met(
            &CC::PatchMarkerAcquired,
            &session.local_evidence,
        );

        let strategies: Vec<String> = match patch_command_phase(&session.local_evidence, patch_done)
        {
            PatchCommandPhase::Reproduce => return,
            PatchCommandPhase::Patch { path, function } => {
                let forced = session
                    .local_evidence
                    .lines()
                    .filter(|l| l.starts_with("[PATCH_PHASE_FORCED]"))
                    .count();
                if forced >= 4 {
                    return;
                }
                let mut v: Vec<String> = Vec::new();
                // patch_function is AST-based and works for every language the AST
                // tool handles (Python included); fall back to a line-range patch
                // only when the traceback names no function.
                if let Some(f) = &function {
                    v.push(format!("patch_function:{}:{}", path, f));
                }
                v.push(format!("replace_lines:{}", path));
                session.local_evidence.push_str(&format!(
                    "[PATCH_PHASE_FORCED] {} (target grounded in the latest traceback)\n",
                    v[0]
                ));
                v
            }
            PatchCommandPhase::Verify => vec![format!("execute_command:{}", command)],
        };

        let mut injected = 0usize;
        for strat in &strategies {
            if let Some(id) = session.graph.find_by_content(strat) {
                if injected == 0 {
                    session.graph.promote_hypothesis(id, 0.58);
                    session.best_hypothesis_id = Some(id);
                    *active_strategy = strat.clone();
                    injected += 1;
                }
                continue;
            }
            let belief = if injected == 0 { 0.58 } else { 0.54 };
            let hid = session.graph.add_node(
                crate::cognition_graph::NodeKind::Hypothesis,
                strat,
                belief,
                100,
            );
            if injected == 0 {
                session.best_hypothesis_id = Some(hid);
                *active_strategy = strat.clone();
            }
            injected += 1;
        }
        if injected > 0 {
            self.log(format!(
                "\x1b[1;36m>> [CORTEX PATCH/VERIFY PHASE]\x1b[0m Forcing `{}`.",
                active_strategy
            ));
        }
    }
}

#[cfg(test)]
mod patch_command_phase_tests {
    use super::{grounded_patch_target, patch_command_phase, PatchCommandPhase};

    const TRACE: &str = "[ACTION FAILED] OS rejected action: Command failed with status: exit status: 1\nSTDERR:\nTraceback (most recent call last):\n  File \"<string>\", line 1, in <module>\n    import math_ops; print(math_ops.divide(10, 0))\n  File \"/elsewhere/math_ops.py\", line 2, in divide\n    return a / b\nZeroDivisionError: division by zero\n";

    fn divide_target() -> Option<(String, Option<String>)> {
        Some(("/elsewhere/math_ops.py".to_string(), Some("divide".to_string())))
    }

    #[test]
    fn innermost_workspace_frame_is_the_target() {
        assert_eq!(grounded_patch_target(TRACE), divide_target());
    }

    #[test]
    fn stdlib_frames_are_skipped() {
        let ev = format!(
            "{}  File \"/usr/lib/python3.13/json/__init__.py\", line 9, in dumps\n    x\n",
            TRACE
        );
        assert_eq!(grounded_patch_target(&ev), divide_target());
    }

    #[test]
    fn module_level_frame_has_no_function() {
        let ev = "Traceback:\n  File \"/elsewhere/x.py\", line 3, in <module>\n    boom()\n";
        assert_eq!(grounded_patch_target(ev), Some(("/elsewhere/x.py".to_string(), None)));
    }

    #[test]
    fn path_under_cwd_is_relativized() {
        let abs = std::env::current_dir().unwrap().join("math_ops.py");
        let ev = format!("  File \"{}\", line 2, in divide\n", abs.display());
        assert_eq!(
            grounded_patch_target(&ev),
            Some(("math_ops.py".to_string(), Some("divide".to_string())))
        );
    }

    #[test]
    fn no_traceback_means_no_target() {
        assert_eq!(grounded_patch_target("[CMD_OK] ls\nStatus: exit status: 0\n"), None);
    }

    #[test]
    fn phases_follow_patch_and_failure_order() {
        // nothing grounded yet: reproduce first
        assert_eq!(patch_command_phase("", false), PatchCommandPhase::Reproduce);
        // failing run, no patch yet
        assert!(matches!(patch_command_phase(TRACE, false), PatchCommandPhase::Patch { .. }));
        // patch landed, nothing failed since
        let patched = format!("{}[REPLACE_LINES_OK] math_ops.py\n", TRACE);
        assert_eq!(patch_command_phase(&patched, true), PatchCommandPhase::Verify);
        // patch landed, the re-run failed again with a fresh traceback
        let refailed = format!("{}{}", patched, TRACE);
        assert!(matches!(patch_command_phase(&refailed, true), PatchCommandPhase::Patch { .. }));
    }
}

// ---------------------------------------------------------------------------
// CONTRACT-FIRST RECON: outstanding path-bound reads outrank belief.
//
// Strategy selection ranks hypotheses by belief, so a post-search expansion
// (`read_file:<search hit>`) or an already-satisfied search strategy can keep the
// node pinned while the contract's mandatory reads stay unread; the strategist
// then cannot act (reading anything else is a STRATEGY VIOLATION) and can only
// halt. While a `ReadPathAcquired` condition is unmet, pin the read for it.
// ---------------------------------------------------------------------------

/// First `ReadPathAcquired` path, in contract order, that the evidence has not
/// yet satisfied.
pub(crate) fn next_unmet_read(
    conditions: &[crate::completion_engine::CompletionCondition],
    evidence: &str,
) -> Option<String> {
    use crate::completion_engine::CompletionCondition as CC;
    conditions.iter().find_map(|c| match c {
        CC::ReadPathAcquired { path }
            if !crate::cortical::contract_advance::condition_is_met(c, evidence) =>
        {
            Some(path.clone())
        }
        _ => None,
    })
}

impl crate::cortical_loop::CorticalLoop {
    pub(crate) fn apply_contract_first_recon(
        &self,
        session: &mut crate::cognition::epistemic_session::EpistemicSession,
        active_strategy: &mut String,
        write_phase_ready: bool,
    ) {
        use crate::completion_engine::CompletionCondition as CC;

        // Patch/verify contracts have their own phase logic; the write phase and
        // compile remediation own the strategy when they are active.
        if write_phase_ready
            || session.compile_remediation.is_active()
            || session.contract.conditions.iter().any(|c| matches!(c, CC::PatchMarkerAcquired))
        {
            return;
        }
        // An unread range in the file being read outranks this: the coverage
        // mandate finishes that file first, then this resumes.
        if session
            .coverage
            .largest_interior_gap()
            .or_else(|| session.coverage.largest_gap())
            .is_some()
        {
            return;
        }
        let Some(path) = next_unmet_read(&session.contract.conditions, &session.local_evidence)
        else {
            return;
        };
        let strat = format!("read_file:{}", path);
        // A read that keeps failing must not pin the node forever.
        let forced = session
            .local_evidence
            .lines()
            .filter(|l| l.starts_with("[CONTRACT_FIRST_FORCED]") && l.contains(strat.as_str()))
            .count();
        if forced >= 3 {
            return;
        }
        session.local_evidence.push_str(&format!(
            "[CONTRACT_FIRST_FORCED] {} (issued by the cortex; satisfied once its [READ_OK] appears; do not re-read it)\n",
            strat
        ));

        if let Some(id) = session.graph.find_by_content(&strat) {
            session.graph.promote_hypothesis(id, 0.58);
            session.best_hypothesis_id = Some(id);
        } else {
            let hid = session.graph.add_node(
                crate::cognition_graph::NodeKind::Hypothesis,
                &strat,
                0.58,
                100,
            );
            session.best_hypothesis_id = Some(hid);
        }
        *active_strategy = strat;
        self.log(format!(
            "\x1b[1;36m>> [CORTEX CONTRACT-FIRST RECON]\x1b[0m Mandatory read outstanding; forcing `{}`.",
            active_strategy
        ));
    }
}

#[cfg(test)]
mod contract_first_recon_tests {
    use super::next_unmet_read;
    use crate::completion_engine::CompletionCondition as CC;

    fn reads() -> Vec<CC> {
        vec![
            CC::ReadPathAcquired { path: "Cargo.toml".to_string() },
            CC::ReadPathAcquired { path: "crates/runtime_core/Cargo.toml".to_string() },
            CC::MinBeliefScore(0.65),
        ]
    }

    const FIRST: &str = "[READ_OK] Cargo.toml\nFULL FILE CONTENT:\n[workspace]\n";
    const SECOND: &str =
        "[READ_OK] crates/runtime_core/Cargo.toml\nFULL FILE CONTENT:\n[package]\n";

    #[test]
    fn first_unmet_read_is_returned_in_contract_order() {
        assert_eq!(next_unmet_read(&reads(), ""), Some("Cargo.toml".to_string()));
    }

    #[test]
    fn satisfied_reads_are_skipped() {
        assert_eq!(
            next_unmet_read(&reads(), FIRST),
            Some("crates/runtime_core/Cargo.toml".to_string())
        );
    }

    #[test]
    fn nothing_is_returned_once_every_read_is_evidenced() {
        let both = format!("{}{}", FIRST, SECOND);
        assert_eq!(next_unmet_read(&reads(), &both), None);
    }
}
