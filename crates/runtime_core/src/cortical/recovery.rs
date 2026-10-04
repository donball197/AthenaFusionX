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
