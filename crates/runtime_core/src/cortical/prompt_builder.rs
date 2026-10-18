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
        format!(
            "{}\n\n[COVERAGE MANDATE] Unread evidence range {}-{} of {}. Read this range before proposing any summary or halt. Use read_file_chunk with start_line={} and max_lines={}.",
            mission,
            gap_s,
            gap_e,
            session.coverage.total_lines().unwrap_or(0),
            gap_s,
            (gap_e as i64 - gap_s as i64 + 1)
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
        && session.coverage.largest_interior_gap().or_else(|| session.coverage.largest_gap()).is_none()
    {
        let outline = crate::cortical::halt_claims::function_outline(&session.local_evidence, 20);
        if outline.is_empty() { enhanced } else { format!("{}\n\n{}", enhanced, outline) }
    } else {
        enhanced
    };

    let recent_vetoes: Vec<_> = session.local_evidence.lines()
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
