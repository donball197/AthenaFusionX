// crates/runtime_core/src/cortical/prompt_builder.rs

use crate::cognition::epistemic_session::EpistemicSession;
use crate::contract_status::ContractStatus;

/// Synthesizes the enhanced mission prompt: enforces phase mandates (write deliverable,
/// coverage reading, or halt synthesis), injects function outlines for read audits,
/// and prepends critical historical vetoes.
pub fn build_enhanced_mission(
    mission: &str,
    session: &EpistemicSession,
    status: &ContractStatus,
    write_phase_ready: bool,
    unwritten_targets: &[String],
    coverage_gap: Option<(u32, u32)>,
) -> String {
    let enhanced = if write_phase_ready {
        format!(
            "{}\n\n[PHASE MANDATE: RECONNAISSANCE VERIFIED]\nThe deterministic reconnaissance contract has been satisfied. The required deliverable MUST now be produced at '{}' using the native `write_file` action.",
            mission,
            unwritten_targets[0]
        )
    } else if let Some((gap_s, gap_e)) = coverage_gap {
        let gap_len = (gap_e as i64 - gap_s as i64 + 1).clamp(1, 400);
        let gap_path = session
            .coverage
            .current_path()
            .map(|p| p.to_string())
            .unwrap_or_else(|| "<the file being read>".to_string());
        format!(
            "{}\n\n[COVERAGE MANDATE] Unread evidence range {}-{} of {} in `{}`. Read this range before proposing any summary or halt.\nEmit exactly this action (a JSON array, key \"type\"):\n[{{\"type\":\"read_file_chunk\",\"path\":\"{}\",\"start_line\":{},\"max_lines\":{},\"reasoning\":\"Read the unread range to complete coverage.\"}}]",
            mission,
            gap_s,
            gap_e,
            session.coverage.total_lines().unwrap_or(0),
            gap_path,
            gap_path,
            gap_s,
            gap_len
        )
    } else if session.coverage.coverage_pct() >= 99.9 && session.contract.read_only {
        format!("{}\n\n[PHASE MANDATE: RECONNAISSANCE COMPLETE]\nYou have successfully read 100% of the target files. Do NOT propose any further read actions. You MUST now synthesize your findings and propose the `halt` action.", mission)
    } else if status == &ContractStatus::Satisfied && unwritten_targets.is_empty() {
        format!("{}\n\n[PHASE MANDATE: COMPLETION VERIFIED]\nThe deterministic contract has been satisfied. You MUST now propose the `halt` action. Do not propose any further reconnaissance.", mission)
    } else {
        mission.to_string()
    };

    // Deterministic outline for audit reports: read evidence carries no line numbers,
    // so models estimate them (and disagree run to run).
    let enhanced = if session.contract.read_only
        && session
            .coverage
            .largest_interior_gap()
            .or_else(|| session.coverage.largest_gap())
            .is_none()
    {
        let outline = crate::cortical::halt_claims::function_outline(&session.local_evidence, 20);
        if outline.is_empty() {
            enhanced
        } else {
            format!("{}\n\n{}", enhanced, outline)
        }
    } else {
        enhanced
    };

    let recent_vetoes: Vec<_> = session
        .local_evidence
        .lines()
        .rev()
        .take(30)
        .filter(|l| {
            l.contains("[LOOP_VETO]")
                || l.contains("[PARSE_FAULT]")
                || l.contains("[CRITIC_VETO")
                || l.contains("[CRITIC_REPEAT_VETO]")
                || l.contains("[STRATEGY_VIOLATION]")
                || l.contains("[HALT_REJECTED")
                || l.contains("[WRITE_CONFORMANCE_FAILED]")
        })
        .collect();

    if !recent_vetoes.is_empty() {
        let mut rev_vetoes = recent_vetoes.clone();
        rev_vetoes.reverse();
        format!("{}\n\n[SYSTEM OVERRIDES - CRITICAL]\n{}", enhanced, rev_vetoes.join("\n"))
    } else {
        enhanced
    }
}

/// Extract the most recent conformance failure and NOOP messages from
/// session evidence so the strategist prompt can include them. This closes
/// the P0 feedback gap: the model must see what went wrong before it can
/// generate a correction.
///
/// Returns a formatted block to prepend to the strategist prompt, or empty
/// if no failures are recorded.
pub fn conformance_feedback(evidence: &str) -> String {
    let clean = crate::cortical::util::strip_read_bodies_for_markers(evidence);
    let lines: Vec<&str> = clean.lines().map(str::trim_start).collect();

    let mut failures: Vec<String> = Vec::new();
    for l in lines.iter().rev() {
        if let Some(rest) = l.strip_prefix("[WRITE_CONFORMANCE_FAILED]") {
            let r = rest.trim().to_string();
            if !failures.contains(&r) {
                failures.push(r);
            }
            if failures.len() == 3 {
                break;
            }
        }
    }

    // NOOP counts only if newer than the last real write.
    let last_ok = lines.iter().rposition(|l| l.starts_with("[WRITE_OK]"));
    let noop = lines
        .iter()
        .rposition(|l| l.starts_with("[WRITE_NOOP]"))
        .filter(|&i| last_ok.map_or(true, |o| i > o))
        .map(|i| lines[i]);

    if failures.is_empty() && noop.is_none() {
        return String::new();
    }

    let mut out = String::from("\n===== CRITICAL: YOUR PREVIOUS WRITE(S) FAILED =====\n");
    if noop.is_some() {
        out.push_str("Your last write was byte-identical to the file on disk. You MUST produce DIFFERENT content.\n");
    }
    for (i, f) in failures.iter().enumerate() {
        out.push_str(&format!("Defect {}: {}\n", i + 1, f));
    }
    out.push_str("\nFIX REQUIREMENTS:\n- Write the COMPLETE file in ONE write_file call\n- Address EVERY defect listed above\n");
    let python = failures
        .iter()
        .any(|f| f.contains(".py") || f.contains("SyntaxError") || f.contains("def "));
    if python {
        out.push_str("- Use REAL newlines between Python statements\n- NEVER join `import`, `def`, `class`, or `if` with semicolons on one line\n- Each `def` and `class` starts on its own line\n");
    }
    out.push_str("=====================================================\n");
    out
}

#[cfg(test)]
mod conformance_feedback_tests {
    use super::conformance_feedback;

    #[test]
    fn failure_is_reported_and_deduped() {
        let ev = "[WRITE_OK] a.py\n[WRITE_CONFORMANCE_FAILED] a.py: missing def get_metrics\n[WRITE_CONFORMANCE_FAILED] a.py: missing def get_metrics\n";
        let f = conformance_feedback(ev);
        assert!(f.contains("missing def get_metrics"));
        assert_eq!(f.matches("Defect ").count(), 1);
        assert!(f.contains("semicolons"));
    }

    #[test]
    fn noop_before_last_write_is_stale() {
        assert!(conformance_feedback("[WRITE_NOOP] a.py\n[WRITE_OK] a.py\n").is_empty());
    }

    #[test]
    fn marker_inside_read_body_is_ignored() {
        let ev = "[READ_OK] prompt_builder.rs\nFULL FILE CONTENT:\n[WRITE_CONFORMANCE_FAILED] x\n[PROPOSITION] p\n";
        assert!(conformance_feedback(ev).is_empty());
    }

    #[test]
    fn markdown_failure_gets_no_python_rules() {
        let f = conformance_feedback("[WRITE_CONFORMANCE_FAILED] report.md: too short\n");
        assert!(!f.contains("semicolons"));
    }
}
