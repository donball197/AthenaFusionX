//! Anti-loop subsystem for relevance gating.

pub fn score_search_relevance(query: &str, evidence: &str) -> f32 {
    let ev_lower = evidence.to_ascii_lowercase();
    if ev_lower.contains("[action failed]")
        || ev_lower.contains("[search_code_error]")
        || ev_lower.contains("os rejected action")
    {
        return 0.10;
    }

    if ev_lower.contains("no code matches found")
        || ev_lower.contains("no matches found")
        || ev_lower.contains("0 results")
    {
        return 0.40;
    }

    let body_lines: Vec<&str> = evidence
        .lines()
        .map(str::trim)
        .filter(|l| {
            !l.is_empty()
                && !l.starts_with("[SEARCH_CODE_OK]")
                && !l.starts_with("[SEARCH_MEMORY_OK]")
                && !l.starts_with("[SEARCH_CHECKED_OK]")
                && *l != "MATCHES:"
        })
        .collect();

    if body_lines.is_empty() {
        return 0.45;
    }

    let query_terms: Vec<String> = query
        .split(|c: char| !c.is_ascii_alphanumeric() && c != '_')
        .map(|s| s.trim().to_ascii_lowercase())
        .filter(|s| s.len() >= 2)
        .collect();

    let overlap_ratio = if query_terms.is_empty() {
        1.0
    } else {
        let hits = query_terms.iter().filter(|t| ev_lower.contains(t.as_str())).count();
        hits as f32 / query_terms.len() as f32
    };

    let density_bonus = (body_lines.len().min(6) as f32 / 6.0) * 0.15;
    (0.55 + 0.25 * overlap_ratio + density_bonus).clamp(0.45, 0.95)
}

pub fn should_raise_belief(score: f32, threshold: f32) -> bool {
    // Belief-raise floor: any score >= 0.30 raises belief regardless of
    // `threshold`; threshold only matters when set below 0.30. Pinned by
    // incident_regressions::incident_should_raise_belief_floor_is_030.
    score >= threshold.min(0.30)
}

// ---------------------------------------------------------------------
// classify_duplicate — replaces the single "duplicate -> filter + decay"
// outcome with a three-way handshake against the contract + evidence.
// ---------------------------------------------------------------------
use crate::agent_protocol::AgentAction;
use crate::cmd_match::command_condition_met;
use crate::completion_engine::{CompletionCondition, SubgoalContract};

/// Replaces the single "duplicate -> filter + decay" outcome.
pub enum FilterOutcome {
    /// True stale duplicate: filter and decay (current behavior).
    FilterAndDecay,
    /// Duplicate, but the contract needs exactly this command and it already
    /// executed: treat the marker as the satisfaction event, no decay.
    ConditionSatisfied,
    /// Duplicate, contract still needs the marker, and it is NOT in evidence
    /// (earlier attempt failed): the retry is legitimate, do not filter.
    AllowRetry,
}

pub fn classify_duplicate(
    action: &AgentAction,
    contract: &SubgoalContract,
    evidence: &str,
) -> FilterOutcome {
    if let AgentAction::ExecuteCommand { command, .. } = action {
        let required = contract.conditions.iter().any(|c| match c {
            CompletionCondition::CommandExecutionVerified { command: want } => {
                crate::cmd_match::cmd_hash(want) == crate::cmd_match::cmd_hash(command)
            }
            _ => false,
        });
        if required {
            return if command_condition_met(command, evidence) {
                FilterOutcome::ConditionSatisfied
            } else {
                FilterOutcome::AllowRetry
            };
        }
    }
    if let AgentAction::WriteFile { path, .. } = action {
        let required = contract.required_artifact_paths.iter().any(|a| a.path == *path);
        if required && !crate::evidence_markers::has_artifact_write(evidence, path) {
            return FilterOutcome::AllowRetry; // earlier write failed
        }
    }
    // RE-READ-AFTER-WRITE EXEMPTION: if the path was written more
    // recently than it was read, its content has changed and re-reading
    // is legitimate verification, not a stale duplicate. Applies to any
    // path the executor emits a [WRITE_OK] for.
    if let AgentAction::ReadFile { path, .. } = action {
        let last_read = evidence.rfind(&format!("[READ_OK] {}", path));
        let last_write = evidence.rfind(&format!("[WRITE_OK] {}", path));
        if let Some(w) = last_write {
            if last_read.map_or(true, |r| w > r) {
                return FilterOutcome::AllowRetry;
            }
        }
    }

    // CORTEXFIX: satisfied whole-file read -- never re-admit it.
    if let AgentAction::ReadFile { path, .. } = action {
        let already = crate::outcome_verifier::OutcomeVerifier::mission_completion_authorized(
            evidence,
            1.0,
            &[CompletionCondition::ReadPathAcquired { path: path.clone() }],
        );
        if already {
            return FilterOutcome::FilterAndDecay;
        }
    }

    // STALE-ADVANCE GUARD: search already satisfied and nothing written since.
    if let AgentAction::SearchCode { path, query, .. } = action {
        let target = format!("{}:{}", path.trim(), query.trim());
        let last_search = evidence.rfind(&format!("[SEARCH_CODE_OK] Path: {}", path.trim()));
        let last_write = ["[WRITE_OK]", "[PATCH_FUNCTION_OK]", "[DIFF_PATCH_OK]"]
            .iter()
            .filter_map(|m| evidence.rfind(*m))
            .max();
        let written_since = matches!((last_search, last_write), (Some(s), Some(w)) if w > s);
        if !written_since && crate::evidence_markers::search_checked_satisfied(evidence, &target) {
            return FilterOutcome::FilterAndDecay;
        }
    }

    // ATTEMPT CAP: a read/search path that has failed MAX_FAILED_ATTEMPTS times
    // is a loop, not a legitimate retry. Writes are excluded on purpose.
    const MAX_FAILED_ATTEMPTS: usize = 3;
    let failing_path = match action {
        AgentAction::ReadFile { path, .. }
        | AgentAction::ReadFileChunk { path, .. }
        | AgentAction::SearchCode { path, .. } => Some(path.trim()),
        _ => None,
    };
    if let Some(p) = failing_path {
        let fails = evidence.matches(&format!("[ACTION FAILED] path={} ", p)).count();
        if fails >= MAX_FAILED_ATTEMPTS {
            return FilterOutcome::FilterAndDecay;
        }
    }

    if matches!(
        action,
        AgentAction::ReadFileChunk { .. }
            | AgentAction::SearchCode { .. }
            | AgentAction::ReadFile { .. }
            | AgentAction::ReadFunction { .. }
    ) {
        return FilterOutcome::AllowRetry;
    }
    FilterOutcome::FilterAndDecay
}

/// Fingerprint a halt by its reasoning body so a *revised* halt is not treated
/// as a duplicate of a prior rejected halt. Call this from the anti-loop key
/// path when the action is Halt { reasoning }.
pub fn halt_reasoning_fingerprint(reasoning: &str) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut h = DefaultHasher::new();
    "halt".hash(&mut h);
    reasoning.hash(&mut h);
    h.finish()
}

/// Prefer this when recording a Halt into the anti-loop set so revised
/// halt bodies are not treated as duplicates of a prior rejected halt.
pub fn halt_action_key(reasoning: &str) -> String {
    format!("halt:{:x}", halt_reasoning_fingerprint(reasoning))
}

#[cfg(test)]
mod relevance_tests {
    use super::{score_search_relevance, should_raise_belief};

    #[test]
    fn high_overlap_search_hits_score_above_boost_threshold() {
        let ev = "[SEARCH_CODE_OK] Path: crates/runtime_core | Query: 'score_search_relevance'
MATCHES:
crates/runtime_core/src/anti_loop.rs:3: pub fn score_search_relevance
crates/runtime_core/src/cortical_loop/evaluate.rs:170: score_search_relevance
";
        let score = score_search_relevance("score_search_relevance", ev);
        assert!(score >= 0.70, "expected >= 0.70, got {}", score);
        assert!(should_raise_belief(score, 0.45));
    }

    #[test]
    fn reread_after_write_is_allowed_not_duplicate() {
        use crate::agent_protocol::AgentAction;
        use crate::completion_engine::{CompletionCondition, SubgoalContract};

        let contract = SubgoalContract {
            node_id: "t".into(),
            objective: "write report.md".into(),
            read_only: false,
            is_mutation_task: true,
            required_artifact_paths: vec![],
            conditions: vec![CompletionCondition::ReadPathAcquired { path: "report.md".into() }],
        };

        let read = AgentAction::ReadFile {
            _thought_process: None,
            path: "report.md".into(),
            reasoning: "verify".into(),
        };

        // Scenario A: read, then write, then re-read. The re-read must be
        // allowed because the file changed.
        let ev = "[READ_OK] report.md\nFULL FILE CONTENT:\nv1\n\
                  [WRITE_OK] report.md\n";
        assert!(matches!(
            super::classify_duplicate(&read, &contract, ev),
            super::FilterOutcome::AllowRetry,
        ));

        // Scenario B: read, no subsequent write. The re-read must still
        // be rejected as a stale duplicate (existing behavior preserved).
        let ev = "[READ_OK] report.md\nFULL FILE CONTENT:\nv1\n";
        assert!(matches!(
            super::classify_duplicate(&read, &contract, ev),
            super::FilterOutcome::FilterAndDecay,
        ));
    }

    #[test]
    fn no_match_and_failed_searches_do_not_boost() {
        let empty = "[SEARCH_CODE_OK] Path: crates | Query: 'missing'
No code matches found in specified path.
";
        assert!(score_search_relevance("missing", empty) < 0.70);

        let failed = "[ACTION FAILED] OS rejected action: bad regex";
        let fail_score = score_search_relevance("(", failed);
        assert!(!should_raise_belief(fail_score, 0.45));
    }
}

#[cfg(test)]
mod p2a_search_guard_tests {
    use super::*;

    fn contract() -> SubgoalContract {
        SubgoalContract {
            node_id: "t".into(), objective: "o".into(), read_only: false,
            is_mutation_task: false, required_artifact_paths: vec![], conditions: vec![],
        }
    }
    fn search() -> AgentAction {
        AgentAction::SearchCode {
            _thought_process: None, path: "crates".into(),
            query: "x".into(), reasoning: String::new(),
        }
    }

    #[test]
    fn search_after_write_is_retry_not_stale() {
        let ev = "[SEARCH_CODE_OK] Path: crates | Query: 'x'\nNo code matches found in specified path.\n[WRITE_OK] a.rs\n";
        assert!(matches!(classify_duplicate(&search(), &contract(), ev), FilterOutcome::AllowRetry));
    }

    #[test]
    fn satisfied_search_without_write_is_stale() {
        let ev = "[SEARCH_CODE_OK] Path: crates | Query: 'x'\nNo code matches found in specified path.\n";
        assert!(matches!(classify_duplicate(&search(), &contract(), ev), FilterOutcome::FilterAndDecay));
    }
}

#[cfg(test)]
mod p2a_patch_marker_tests {
    use super::*;

    fn contract() -> SubgoalContract {
        SubgoalContract {
            node_id: "t".into(), objective: "o".into(), read_only: false,
            is_mutation_task: false, required_artifact_paths: vec![], conditions: vec![],
        }
    }
    fn search() -> AgentAction {
        AgentAction::SearchCode {
            _thought_process: None, path: "crates".into(),
            query: "x".into(), reasoning: String::new(),
        }
    }
    const S: &str = "[SEARCH_CODE_OK] Path: crates | Query: 'x'\nNo code matches found in specified path.\n";

    #[test]
    fn search_after_diff_patch_is_retry() {
        let ev = format!("{S}[DIFF_PATCH_OK] Applied changes to crates\n");
        assert!(matches!(classify_duplicate(&search(), &contract(), &ev), FilterOutcome::AllowRetry));
    }

    #[test]
    fn search_after_patch_function_is_retry() {
        let ev = format!("{S}[PATCH_FUNCTION_OK] crates:f\nUpdated function 'f' in crates\n");
        assert!(matches!(classify_duplicate(&search(), &contract(), &ev), FilterOutcome::AllowRetry));
    }

    #[test]
    fn search_after_idempotent_patch_is_still_stale() {
        let ev = format!("{S}[PATCH_IDEMPOTENT_OK] crates:f\n");
        assert!(matches!(classify_duplicate(&search(), &contract(), &ev), FilterOutcome::FilterAndDecay));
    }
}

#[cfg(test)]
mod p2b_attempt_cap_tests {
    use super::*;

    fn contract() -> SubgoalContract {
        SubgoalContract {
            node_id: "t".into(), objective: "o".into(), read_only: false,
            is_mutation_task: false, required_artifact_paths: vec![], conditions: vec![],
        }
    }
    fn read() -> AgentAction {
        AgentAction::ReadFile {
            _thought_process: None, path: "a.rs".into(), reasoning: String::new(),
        }
    }
    const F: &str = "[ACTION FAILED] path=a.rs OS rejected action: sandbox denied\n";

    #[test]
    fn three_failed_reads_are_capped() {
        let ev = F.repeat(3);
        assert!(matches!(classify_duplicate(&read(), &contract(), &ev), FilterOutcome::FilterAndDecay));
    }

    #[test]
    fn one_failed_read_is_still_retry() {
        assert!(matches!(classify_duplicate(&read(), &contract(), F), FilterOutcome::AllowRetry));
    }
}
