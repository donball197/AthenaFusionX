use crate::completion_engine::CompletionCondition;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutcomeAssessment {
    pub success_probability: f32,
    pub deviation_score: f32,
    pub confidence_delta: f32,
    pub lessons: Vec<String>,
}

pub struct OutcomeVerifier;

impl OutcomeVerifier {
    pub fn evaluate(expected: &str, observed: &str) -> OutcomeAssessment {
        let success = if expected == observed { 1.0 } else { 0.5 };
        OutcomeAssessment {
            success_probability: success,
            deviation_score: 1.0 - success,
            confidence_delta: success - 0.5,
            lessons: vec![format!("Expected: {} Observed: {}", expected, observed)],
        }
    }

    /// ⚡ STRICT TYPED CONTRACT VERIFICATION
    /// Replaces the blind-scan string matching with actual condition enforcement.
    fn evaluate_contract(
        local_evidence: &str,
        belief_score: f32,
        conditions: &[CompletionCondition],
    ) -> Result<(), String> {
        // FAIL CLOSED: an empty contract authorizes nothing. Universal
        // quantification over zero conditions is vacuously true and would let
        // an unrecognized mission authorize itself.
        if conditions.is_empty() {
            return Err("empty completion contract is never satisfied".to_string());
        }

        // Former "telemetry overrides all else" branch removed. External
        // proof, when implemented, must be an ADDITIONAL contract condition
        // rather than a substitution for the declared conditions.

        // 2. Strictly verify that EVERY condition in the contract is met
        for cond in conditions {
            let met = match cond {
                CompletionCondition::ReadMarkerAcquired => local_evidence.lines().any(|l| {
                    let t = l.trim_start();
                    t.starts_with("[READ_OK]") || t.starts_with("[READ_CHUNK_OK]")
                }),
                CompletionCondition::ReadPathAcquired { path } => {
                    let expected = path.trim();
                    local_evidence.lines().any(|l| {
                        if let Some(rest) = l.strip_prefix("[READ_OK]") {
                            crate::evidence_paths::paths_equivalent(rest.trim(), expected)
                        } else if let Some(rest) = l.strip_prefix("[READ_CHUNK_OK] File: ") {
                            rest.split(" | ")
                                .next()
                                .map(|p| crate::evidence_paths::paths_equivalent(p, expected))
                                .unwrap_or(false)
                        } else if let Some(rest) = l.strip_prefix("[ACTION FAILED] path=") {
                            // CONFIRMED-ABSENCE ONLY. ENOENT proves the target
                            // was inspected and found absent; sandbox denials
                            // inspected nothing. See
                            // evidence_paths::is_confirmed_absence.
                            rest.split(" OS rejected action")
                                .next()
                                .map(|p| crate::evidence_paths::paths_equivalent(p, expected))
                                .unwrap_or(false)
                                && crate::evidence_paths::is_confirmed_absence(rest)
                        } else if let Some(rest) = l.strip_prefix("[SEARCH_CODE_OK] Path: ") {
                            // "Inspect this location" is satisfied by a
                            // documented, path-scoped search attempt even
                            // when it finds nothing -- e.g. confirming a
                            // target file does not yet exist before
                            // creating it. Without this, a ReadPathAcquired
                            // condition on a not-yet-created path is
                            // permanently unsatisfiable: every ReadFile
                            // attempt fails with ENOENT, and no other
                            // action can ever produce a [READ_OK] for a
                            // path with nothing there to read. Confirmed
                            // root cause of the reflection_test.rs
                            // mission's task_1 stall (0/3 nodes, belief
                            // decayed to 0.00 across 11 turns). Path
                            // comparison uses the shared paths_equivalent.
                            rest.split(" | ")
                                .next()
                                .map(|p| crate::evidence_paths::paths_equivalent(p, expected))
                                .unwrap_or(false)
                        } else {
                            false
                        }
                    })
                }
                CompletionCondition::SearchMarkerAcquired => {
                    local_evidence.lines().any(|l| l.trim_start().starts_with("[SEARCH_CODE_OK]"))
                }
                CompletionCondition::SearchTermAcquired { term } => {
                    // Term matching lives in evidence_markers::search_term_satisfied --
                    // shared with orchestrator_v2/verifier.rs so the two
                    // verifiers cannot diverge again.
                    crate::evidence_markers::search_term_satisfied(local_evidence, term)
                }
                CompletionCondition::SearchMemoryMarkerAcquired => {
                    local_evidence.lines().any(|l| l.trim_start().starts_with("[SEARCH_MEMORY_OK]"))
                }
                CompletionCondition::WriteMarkerAcquired => {
                    local_evidence.lines().any(|l| l.trim_start().starts_with("[WRITE_OK]"))
                }
                CompletionCondition::WritePathAcquired { path } => {
                    let expected = path.trim();
                    // Two emission shapes exist in the executor:
                    //   [WRITE_OK] <path>                                  (WriteFile)
                    //   [WRITE_OK] Successfully synthesized and wrote report to <path>  (SynthesizeReport)
                    // Accept both. The SynthesizeReport form is a full
                    // sentence, not a bare path, so exact-equality against
                    // the remainder of the line would reject a write that
                    // physically succeeded.
                    let synthesized =
                        format!("Successfully synthesized and wrote report to {}", expected);
                    local_evidence.lines().any(|l| {
                        if let Some(rest) = l.strip_prefix("[WRITE_OK]") {
                            let rest = rest.trim();
                            rest == expected || rest == synthesized
                        } else {
                            false
                        }
                    })
                }
                CompletionCondition::PatchMarkerAcquired => {
                    crate::evidence_markers::has_patch_satisfied_marker(local_evidence)
                }
                CompletionCondition::CommandMarkerAcquired => {
                    // A bare "[CMD_OK]" substring is not proof: mission text,
                    // quoted source, and tool output can all contain the
                    // literal marker (see the spoof warning in
                    // orchestrator_v2's verifier.rs). Authorize only a real
                    // execution record: a line-anchored [CMD_OK] whose record
                    // carries a Status of exit status: 0 -- the format the
                    // executor emits. The legacy short form
                    // "[CMD_OK] Status: exit status: 0" parses through the
                    // same path as the long
                    // "[CMD_OK] Command: ...\nStatus: exit status: 0" form.
                    // Delegates to the shared binder in evidence_paths. The
                    // former 1-line-forward scan mishandled multi-line
                    // commands and bracketed output.
                    crate::evidence_paths::cmd_ok_binds_command(local_evidence)
                }
                CompletionCondition::LineageEvidenceAcquired => {
                    verify_lineage_provenance(local_evidence)
                }
                CompletionCondition::HaltDelivered => {
                    local_evidence.lines().any(|l| l.trim_start().starts_with("[HALT]"))
                }
                CompletionCondition::ReadFunctionMarkerAcquired { path, function_name } => {
                    local_evidence.contains("[READ_FUNCTION_OK]")
                        && (local_evidence.contains(&format!("{}:{}", path, function_name))
                            || (local_evidence.contains(path.as_str())
                                && local_evidence.contains(function_name.as_str())))
                }
                CompletionCondition::CommandExecutionVerified { command } => {
                    let expected = command.trim();

                    local_evidence.lines().any(|line| {
                        let Some(actual) = line.strip_prefix("[CMD_OK] Command:") else {
                            return false;
                        };

                        actual.trim() == expected
                            && local_evidence
                                .lines()
                                .any(|status_line| status_line.trim() == "Status: exit status: 0")
                    })
                }
                CompletionCondition::SourceCodeEvidenceRequired => {
                    // Has at least one distinct source path actually been examined?
                    //
                    // Counts every evidence form that names a physical path:
                    //
                    //   [READ_OK] <path>
                    //   [READ_CHUNK_OK] File: <path> | Lines N-M of T
                    //   [SEARCH_CODE_OK] Path: <path> | Query: ...
                    //
                    // [READ_CHUNK_OK] must be counted: a file large enough to
                    // require chunked reads (e.g. a 2300-line source file)
                    // can only produce [READ_CHUNK_OK] markers, never a bare
                    // [READ_OK]. Excluding it made this condition unreachable
                    // for any single-file audit.
                    //
                    // Threshold is >= 1, not >= 2. Multi-file comparative
                    // missions are already enforced by required_artifact_paths
                    // and the mission-contract inference in completion_engine;
                    // the >= 2 check only made single-file missions
                    // unsatisfiable by construction. Confirmed root cause of
                    // the 1-node audit abort: 15 chunks of one file counted
                    // as 1 path, recon_ready stayed false, write_phase_ready
                    // stayed false for all 60 turns.
                    let mut seen_paths = std::collections::HashSet::new();
                    for l in local_evidence.lines() {
                        if let Some(path) = l.strip_prefix("[READ_OK]") {
                            let path = path.trim();
                            if !path.is_empty() {
                                seen_paths.insert(path.to_string());
                            }
                        } else if let Some(rest) = l.strip_prefix("[READ_CHUNK_OK] File: ") {
                            if let Some(path) = rest.split(" | ").next() {
                                let path = path.trim();
                                if !path.is_empty() {
                                    seen_paths.insert(path.to_string());
                                }
                            }
                        } else if let Some(rest) = l.strip_prefix("[SEARCH_CODE_OK] Path: ") {
                            if let Some(path) = rest.split(" | ").next() {
                                let path = path.trim();
                                if !path.is_empty() {
                                    seen_paths.insert(path.to_string());
                                }
                            }
                        }
                    }
                    !seen_paths.is_empty()
                }
                CompletionCondition::MinBeliefScore(min_score) => belief_score >= *min_score,
            };

            if !met {
                return Err(format!("Contract condition {:?} not met in evidence.", cond));
            }
        }

        Ok(())
    }

    /// Single boolean completion-authorization boundary.
    pub fn mission_completion_authorized(
        local_evidence: &str,
        belief_score: f32,
        conditions: &[CompletionCondition],
    ) -> bool {
        Self::evaluate_contract(local_evidence, belief_score, conditions).is_ok()
    }

    /// Diagnostic helper: names each contract condition that is NOT yet
    /// satisfied. Backs actionable HALT-rejection logging.
    pub fn unmet_conditions(
        local_evidence: &str,
        belief_score: f32,
        conditions: &[CompletionCondition],
    ) -> Vec<String> {
        conditions
            .iter()
            .filter(|cond| {
                !Self::mission_completion_authorized(
                    local_evidence,
                    belief_score,
                    std::slice::from_ref(*cond),
                )
            })
            .map(|cond| format!("{:?}", cond))
            .collect()
    }
}

/// Cryptographically verify the provenance of ancestor evidence.
///
/// Ensures the enclosed payload matches the declared SHA-256 digest
/// stamped on both boundary markers.
pub fn verify_lineage_provenance(evidence: &str) -> bool {
    use sha2::{Digest, Sha256};

    let start_prefix = "[DAG LINEAGE INHERITED EVIDENCE sha256:";
    let end_prefix = "[DAG LINEAGE COMPLETE sha256:";

    let Some(start_pos) = evidence.find(start_prefix) else {
        // Backward-compatible fallback for unhashed markers
        return evidence.contains("[DAG LINEAGE INHERITED EVIDENCE]")
            && evidence.contains("[DAG LINEAGE COMPLETE]");
    };

    let hash_start = start_pos + start_prefix.len();
    let Some(hash_end) = evidence[hash_start..].find(']') else {
        return false;
    };
    let declared_hash = evidence[hash_start..hash_start + hash_end].trim();

    let payload_start = hash_start + hash_end + 1;
    let Some(end_marker_rel) = evidence[payload_start..].find(end_prefix) else {
        return false;
    };
    let payload_end = payload_start + end_marker_rel;
    let payload = evidence[payload_start..payload_end].trim();

    let end_hash_start = payload_end + end_prefix.len();
    let Some(end_hash_end) = evidence[end_hash_start..].find(']') else {
        return false;
    };
    let end_declared_hash = evidence[end_hash_start..end_hash_start + end_hash_end].trim();

    if declared_hash != end_declared_hash {
        return false;
    }

    let computed_hash = format!("{:x}", Sha256::digest(payload.as_bytes()));
    computed_hash == declared_hash
}

#[cfg(test)]
mod tests {
    use super::OutcomeVerifier;
    use crate::completion_engine::CompletionCondition;

    fn verify(command: &str, evidence: &str) -> bool {
        let condition =
            CompletionCondition::CommandExecutionVerified { command: command.to_string() };

        OutcomeVerifier::mission_completion_authorized(evidence, 1.0, &[condition])
    }

    #[test]
    fn case_8_read_path_acquired_rejects_wrong_path() {
        let conditions = vec![CompletionCondition::ReadPathAcquired { path: "wanted.rs".into() }];
        assert!(!OutcomeVerifier::mission_completion_authorized(
            "[READ_OK] other.rs\n",
            1.0,
            &conditions
        ));
        assert!(OutcomeVerifier::mission_completion_authorized(
            "[READ_OK] wanted.rs\n",
            1.0,
            &conditions
        ));
    }

    #[test]
    fn write_path_acquired_accepts_synthesize_report_form() {
        // SynthesizeReport emits `[WRITE_OK] Successfully synthesized and
        // wrote report to <path>`, not the bare `[WRITE_OK] <path>` form
        // produced by WriteFile. The path-bound condition must accept both.
        let conditions = vec![CompletionCondition::WritePathAcquired { path: "report.md".into() }];
        assert!(OutcomeVerifier::mission_completion_authorized(
            "[WRITE_OK] report.md\n",
            1.0,
            &conditions
        ));
        assert!(OutcomeVerifier::mission_completion_authorized(
            "[WRITE_OK] Successfully synthesized and wrote report to report.md\n",
            1.0,
            &conditions
        ));
        assert!(!OutcomeVerifier::mission_completion_authorized(
            "[WRITE_OK] Successfully synthesized and wrote report to other.md\n",
            1.0,
            &conditions
        ));
    }

    #[test]
    fn read_path_acquired_accepts_chunk_marker() {
        let conditions = vec![CompletionCondition::ReadPathAcquired { path: "big.rs".into() }];
        assert!(OutcomeVerifier::mission_completion_authorized(
            "[READ_CHUNK_OK] File: big.rs | Lines 1-50 of 2711\n",
            1.0,
            &conditions
        ));
    }

    #[test]
    fn exact_command_is_required_for_command_completion() {
        let wrong_command = concat!(
            "[CMD_OK] Command: uptime\n",
            "Status: exit status: 0\n",
            "STDOUT:\n",
            "up 6 days\n",
        );

        let exact_command = concat!(
            "[CMD_OK] Command: uptime; df -h /; free -m\n",
            "Status: exit status: 0\n",
            "STDOUT:\n",
            "up 6 days\n",
        );

        assert!(!verify("uptime; df -h /; free -m", wrong_command));

        assert!(verify("uptime; df -h /; free -m", exact_command));
    }

    #[test]
    fn successful_command_requires_zero_exit_status() {
        let failed_command = concat!("[CMD_OK] Command: uptime\n", "Status: exit status: 1\n",);

        let missing_status = "[CMD_OK] Command: uptime\n";

        let successful_command = concat!("[CMD_OK] Command: uptime\n", "Status: exit status: 0\n",);

        assert!(!verify("uptime", failed_command));

        assert!(!verify("uptime", missing_status));

        assert!(verify("uptime", successful_command));
    }

    #[test]
    fn patch_idempotent_marker_satisfies_patch_contract() {
        let condition = CompletionCondition::PatchMarkerAcquired;

        assert!(OutcomeVerifier::mission_completion_authorized(
            "[PATCH_IDEMPOTENT_OK] crates/runtime_core/src/example.rs:target\n",
            1.0,
            &[condition],
        ));
    }

    #[test]
    fn unrelated_evidence_does_not_satisfy_patch_contract() {
        let condition = CompletionCondition::PatchMarkerAcquired;

        assert!(!OutcomeVerifier::mission_completion_authorized(
            "[READ_OK] crates/runtime_core/src/example.rs\n",
            1.0,
            &[condition],
        ));
    }

    #[test]
    fn generic_cmd_ok_requires_zero_exit_status_record() {
        // Legacy short form still authorizes.
        assert!(OutcomeVerifier::mission_completion_authorized(
            "[CMD_OK] Status: exit status: 0\n",
            1.0,
            &[CompletionCondition::CommandMarkerAcquired],
        ));

        // Executor long form authorizes.
        assert!(OutcomeVerifier::mission_completion_authorized(
            "[CMD_OK] Command: cargo test\nStatus: exit status: 0\n",
            1.0,
            &[CompletionCondition::CommandMarkerAcquired],
        ));

        // A failed status must not authorize.
        assert!(!OutcomeVerifier::mission_completion_authorized(
            "[CMD_OK] Command: cargo test\nStatus: exit status: 101\n",
            1.0,
            &[CompletionCondition::CommandMarkerAcquired],
        ));

        // Literal "[CMD_OK]" prose must not authorize.
        assert!(!OutcomeVerifier::mission_completion_authorized(
            "quoting the marker [CMD_OK] in mission text\n",
            1.0,
            &[CompletionCondition::CommandMarkerAcquired],
        ));

        // A bare marker with no status record must not authorize.
        assert!(!OutcomeVerifier::mission_completion_authorized(
            "all done [CMD_OK]\n",
            1.0,
            &[CompletionCondition::CommandMarkerAcquired],
        ));
    }

    #[test]
    fn generic_cmd_ok_remains_legacy_compatible() {
        let condition = CompletionCondition::CommandMarkerAcquired;

        assert!(OutcomeVerifier::mission_completion_authorized(
            "[CMD_OK] Status: exit status: 0\n",
            1.0,
            &[condition],
        ));
    }

    #[test]
    fn valid_hashed_lineage_passes_verification() {
        use sha2::{Digest, Sha256};

        let payload = "[CMD_OK] Command: uptime\nStatus: exit status: 0\nSTDOUT: up 1 hour";
        let digest = format!("{:x}", Sha256::digest(payload.as_bytes()));
        let evidence = format!(
            "[DAG LINEAGE INHERITED EVIDENCE sha256:{}]\n{}\n[DAG LINEAGE COMPLETE sha256:{}]",
            digest, payload, digest
        );

        let condition = CompletionCondition::LineageEvidenceAcquired;
        assert!(OutcomeVerifier::mission_completion_authorized(&evidence, 1.0, &[condition]));
    }

    #[test]
    fn tampered_lineage_payload_fails_verification() {
        use sha2::{Digest, Sha256};

        let original_payload = "[CMD_OK] Command: uptime\nStatus: exit status: 0";
        let digest = format!("{:x}", Sha256::digest(original_payload.as_bytes()));

        // Attacker alters the payload but reuses the declared digest
        let tampered_payload = "[CMD_OK] Command: uptime\nStatus: exit status: 1";
        let evidence = format!(
            "[DAG LINEAGE INHERITED EVIDENCE sha256:{}]\n{}\n[DAG LINEAGE COMPLETE sha256:{}]",
            digest, tampered_payload, digest
        );

        let condition = CompletionCondition::LineageEvidenceAcquired;
        assert!(!OutcomeVerifier::mission_completion_authorized(&evidence, 1.0, &[condition]));
    }

    #[test]
    fn mismatched_end_hash_fails_verification() {
        let evidence = concat!(
            "[DAG LINEAGE INHERITED EVIDENCE sha256:1111111111111111111111111111111111111111111111111111111111111111]\n",
            "[CMD_OK] payload\n",
            "[DAG LINEAGE COMPLETE sha256:2222222222222222222222222222222222222222222222222222222222222222]"
        );

        let condition = CompletionCondition::LineageEvidenceAcquired;
        assert!(!OutcomeVerifier::mission_completion_authorized(evidence, 1.0, &[condition]));
    }
}

#[cfg(test)]
mod extraction_guards {
    use super::*;

    #[test]
    fn unmet_conditions_names_each_unsatisfied_condition() {
        let conditions =
            vec![CompletionCondition::ReadMarkerAcquired, CompletionCondition::WriteMarkerAcquired];
        let unmet = OutcomeVerifier::unmet_conditions(
            "[READ_OK] crates/runtime_core/src/lib.rs\n",
            1.0,
            &conditions,
        );
        assert_eq!(unmet.len(), 1);
        assert!(unmet[0].contains("WriteMarkerAcquired"), "got {:?}", unmet);
    }

    #[test]
    fn mission_completion_authorized_mirrors_approved_contract() {
        let conditions = vec![CompletionCondition::ReadMarkerAcquired];
        let evidence = "[READ_OK] crates/runtime_core/src/lib.rs";

        assert!(OutcomeVerifier::mission_completion_authorized(evidence, 1.0, &conditions));
    }

    #[test]
    fn mission_completion_authorized_is_false_on_unmet_condition() {
        let conditions = vec![CompletionCondition::WriteMarkerAcquired];
        let evidence = "[READ_OK] crates/runtime_core/src/lib.rs";

        assert!(!OutcomeVerifier::mission_completion_authorized(evidence, 1.0, &conditions));
    }

    #[test]
    fn belief_threshold_is_gated_exactly_at_boundary() {
        let conditions = vec![CompletionCondition::MinBeliefScore(0.8)];

        assert!(OutcomeVerifier::mission_completion_authorized("[READ_OK] x", 0.8, &conditions));

        assert!(!OutcomeVerifier::mission_completion_authorized("[READ_OK] x", 0.79, &conditions));
    }

    #[test]
    fn empty_contract_fails_closed() {
        assert!(!OutcomeVerifier::mission_completion_authorized("[WRITE_OK] out.md", 1.0, &[]));

        assert!(!OutcomeVerifier::mission_completion_authorized("[READ_OK] lib.rs", 1.0, &[]));
    }

    #[test]
    fn command_contract_still_requires_zero_exit_status() {
        let conditions = vec![CompletionCondition::CommandExecutionVerified {
            command: "cargo test".to_string(),
        }];

        let ok = "[CMD_OK] Command: cargo test\nStatus: exit status: 0";
        let bad = "[CMD_OK] Command: cargo test\nStatus: exit status: 101";

        assert!(OutcomeVerifier::mission_completion_authorized(ok, 1.0, &conditions));

        assert!(!OutcomeVerifier::mission_completion_authorized(bad, 1.0, &conditions));
    }

    #[test]
    fn path_scoped_search_satisfies_not_yet_created_path() {
        let conditions =
            vec![CompletionCondition::ReadPathAcquired { path: "reports/summary.md".into() }];

        let evidence = "[SEARCH_CODE_OK] Path: reports/summary.md | Query: todo | 0 matches";

        assert!(OutcomeVerifier::mission_completion_authorized(evidence, 1.0, &conditions));
    }
}
