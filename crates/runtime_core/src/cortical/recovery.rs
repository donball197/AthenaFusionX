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
                    session.local_evidence.push_str("\n");
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

            if read_ok_done && patch_marker_needed && !already_mutated && !write_phase_ready {
                let mut target_path: Option<String> = None;
                for line in session.local_evidence.lines().rev() {
                    let t = line.trim();
                    if let Some(rest) = t.strip_prefix("[READ_OK]") {
                        let p = rest.trim().split_whitespace().next().unwrap_or("").trim();
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
                        let p = rest.trim().split_whitespace().next().unwrap_or("").trim();
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
