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

        // Untargeted/generic markers must come from this node's own execution,
        // never from inherited DAG lineage blocks.
        let own_evidence = strip_inherited_lineage(local_evidence);

        // EVIDENCE SCOPE: marker-bound conditions require evidence from
        // THIS node's own execution (own_evidence). Path-bound conditions
        // may also accept inherited lineage evidence. Guarded by
        // incident_regressions::incident_2.

        // 2. Strictly verify that EVERY condition in the contract is met
        for cond in conditions {
            let met = match cond {
                CompletionCondition::ReadMarkerAcquired => own_evidence.lines().any(|l| {
                    let t = l.trim_start();
                    t.starts_with("[READ_OK]") || t.starts_with("[READ_CHUNK_OK]")
                }),
                CompletionCondition::ReadPathAcquired { path } => {
                    let expected = path.trim();
                    let typed_sc =
                        crate::evidence_adapter::assess_read_path_typed(local_evidence, expected)
                            == nexus_evidence_requirements::assessment::Assessment::SC;
                    typed_sc
                        || local_evidence.lines().any(|l| {
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
                    own_evidence.lines().any(|l| l.trim_start().starts_with("[SEARCH_CODE_OK]"))
                }
                CompletionCondition::SearchTermAcquired { term } => {
                    // Term matching lives in evidence_markers::search_term_satisfied --
                    // shared with orchestrator_v2/verifier.rs so the two
                    // verifiers cannot diverge again.
                    crate::evidence_markers::search_term_satisfied(local_evidence, term)
                }
                CompletionCondition::SearchCheckedAcquired { target } => {
                    crate::evidence_markers::search_checked_satisfied(local_evidence, target)
                }
                CompletionCondition::SearchMemoryMarkerAcquired => {
                    own_evidence.lines().any(|l| l.trim_start().starts_with("[SEARCH_MEMORY_OK]"))
                }
                CompletionCondition::WriteMarkerAcquired => {
                    own_evidence.lines().any(|l| l.trim_start().starts_with("[WRITE_OK]"))
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
                    crate::evidence_markers::write_path_satisfied(local_evidence, expected)
                }
                CompletionCondition::PatchMarkerAcquired => {
                    crate::evidence_markers::has_patch_satisfied_marker(&own_evidence)
                }
                CompletionCondition::PatchPathAcquired { kind, path, function_name } => {
                    crate::evidence_markers::patch_path_satisfied(
                        local_evidence,
                        kind,
                        path,
                        function_name.as_deref(),
                    )
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
                    //
                    // Two corrections layered on top of that scan:
                    //
                    // 1. Strip inherited evidence: everything before the last
                    //    [DAG LINEAGE COMPLETE sha256:...] marker belongs to
                    //    an ancestor node. A node that follows a command node
                    //    must not be trivially complete from inherited
                    //    [CMD_OK] blocks; the node's own command must run.
                    //
                    // 2. Reset on halt rejection: if [HALT_REJECTED] appears
                    //    after the last own [CMD_OK], the node has already
                    //    attempted a halt and been rejected at mission level.
                    //    The contract is no longer satisfied until the node
                    //    produces new command evidence. This is what stops
                    //    the auto-halt from overriding a freshly-recovered
                    //    ExecuteCommand action.
                    let own = local_evidence
                        .rfind("[DAG LINEAGE COMPLETE sha256:")
                        .and_then(|i| {
                            local_evidence[i..].find('\n').map(|j| &local_evidence[i + j + 1..])
                        })
                        .unwrap_or(local_evidence);
                    let reject_after_ok = match (own.rfind("[HALT_REJECTED"), own.rfind("[CMD_OK]"))
                    {
                        (Some(r), Some(o)) if r > o => true,
                        (Some(_), None) => true,
                        _ => false,
                    };
                    !reject_after_ok && crate::evidence_paths::cmd_ok_binds_command(own)
                }
                CompletionCondition::LineageEvidenceAcquired => {
                    verify_lineage_provenance(local_evidence)
                }
                CompletionCondition::HaltDelivered => {
                    own_evidence.lines().any(|l| l.trim_start().starts_with("[HALT]"))
                        && crate::halt_gate::halt_range_gate(&own_evidence).is_none()
                }
                CompletionCondition::ReadFunctionMarkerAcquired { path, function_name } => {
                    crate::evidence_markers::read_function_marker_satisfies(
                        local_evidence,
                        path,
                        function_name,
                    )
                }
                CompletionCondition::CommandExecutionVerified { command } => {
                    // Ordinary command contracts are exact: the evidence must
                    // bind to the requested command through CMD_OK/CMD_HASH.
                    crate::cmd_match::command_condition_met(command, local_evidence)
                }
                CompletionCondition::MathematicalExecutionVerified { command } => {
                    // Mathematical derivations deliberately permit the
                    // compatibility path required by their generated
                    // python3 -c command, but ONLY because the contract
                    // explicitly identifies this node as mathematical.
                    crate::cmd_match::command_condition_met(command, local_evidence)
                        || (crate::completion_engine::math_command_satisfied(local_evidence)
                            && crate::evidence_paths::cmd_ok_binds_command(local_evidence))
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
                    for l in own_evidence.lines() {
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

    /// Physical evidence authorizes completion; belief only ranks hypotheses.
    /// Returns true when every non-MinBeliefScore condition is met.
    pub fn physical_contract_satisfied(
        local_evidence: &str,
        belief_score: f32,
        conditions: &[CompletionCondition],
    ) -> bool {
        let unmet = Self::unmet_conditions(local_evidence, belief_score, conditions);
        unmet.is_empty() || unmet.iter().all(|s| s.contains("MinBeliefScore"))
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
        // Fail closed: unhashed lineage is no longer accepted here. Any
        // replay path that needs the fallback must call
        // `verify_lineage_provenance_legacy` explicitly.
        return false;
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
    if payload.is_empty() {
        return false;
    }

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

fn strip_inherited_lineage(evidence: &str) -> String {
    const START: &str = "[DAG LINEAGE INHERITED EVIDENCE";
    const END: &str = "[DAG LINEAGE COMPLETE";

    let mut in_lineage = false;
    let mut own = Vec::new();
    for line in evidence.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with(START) {
            in_lineage = true;
            continue;
        }
        if trimmed.starts_with(END) && in_lineage {
            in_lineage = false;
            continue;
        }
        if !in_lineage {
            own.push(line);
        }
    }
    own.join("\n")
}

/// Legacy compatibility: accepts unhashed boundary markers WITHOUT hash
/// validation. Use ONLY for replaying evidence produced before the sha256
/// boundary markers landed. New code MUST call `verify_lineage_provenance`.
pub fn verify_lineage_provenance_legacy(evidence: &str) -> bool {
    if verify_lineage_provenance(evidence) {
        return true;
    }
    evidence.contains("[DAG LINEAGE INHERITED EVIDENCE]")
        && evidence.contains("[DAG LINEAGE COMPLETE]")
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
    fn read_function_rejects_marker_and_target_from_different_records() {
        let condition = CompletionCondition::ReadFunctionMarkerAcquired {
            path: "crates/runtime_core/src/bar.rs".into(),
            function_name: "target_b".into(),
        };

        let evidence = concat!(
            "[READ_FUNCTION_OK] crates/runtime_core/src/foo.rs:actual_a\n",
            "File: crates/runtime_core/src/foo.rs | Function: actual_a\n",
            "EXACT AST SOURCE:\n",
            "fn actual_a() {}\n",
            "Observed reference to crates/runtime_core/src/bar.rs\n",
            "Discussion mentions target_b\n",
        );

        assert!(!OutcomeVerifier::mission_completion_authorized(evidence, 1.0, &[condition],));
    }

    #[test]
    fn read_function_accepts_exact_marker_record() {
        let condition = CompletionCondition::ReadFunctionMarkerAcquired {
            path: "crates/runtime_core/src/bar.rs".into(),
            function_name: "target_b".into(),
        };

        let evidence = concat!(
            "[READ_FUNCTION_OK] crates/runtime_core/src/bar.rs:target_b\n",
            "File: crates/runtime_core/src/bar.rs | Function: target_b\n",
            "EXACT AST SOURCE:\n",
            "fn target_b() {}\n",
        );

        assert!(OutcomeVerifier::mission_completion_authorized(evidence, 1.0, &[condition],));
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
    fn math_evidence_does_not_satisfy_ordinary_command_contract() {
        let condition =
            CompletionCondition::CommandExecutionVerified { command: "uptime".to_string() };

        let evidence = concat!(
            "ALL TABLE\n",
            "ANY TABLE\n",
            "ALL_COMMUTATIVE True\n",
            "ALL_IDEMPOTENT True\n",
            "RC_ABSORBING_ALL True\n",
            "ORDER_SC_GT_SP_GT_NP_GT_NN True\n",
            "ANY_COMMUTATIVE True\n",
            "ANY_IDEMPOTENT True\n",
            "SC_ABSORBING_ANY True\n",
            "DUALITY_ALL_36 True\n",
            "DUAL_INVOLUTION True\n",
            "Status: exit status: 0\n",
        );

        assert!(!OutcomeVerifier::mission_completion_authorized(evidence, 1.0, &[condition],));
    }

    #[test]
    fn mathematical_execution_contract_accepts_math_compatibility_evidence() {
        let condition = CompletionCondition::MathematicalExecutionVerified {
            command: "python3 -c 'print(math)'".to_string(),
        };

        let evidence = concat!(
            "[CMD_OK] Command: echo math_output\n",
            "ALL TABLE\n",
            "ANY TABLE\n",
            "ALL_COMMUTATIVE True\n",
            "ALL_IDEMPOTENT True\n",
            "RC_ABSORBING_ALL True\n",
            "ORDER_SC_GT_SP_GT_NP_GT_NN True\n",
            "ANY_COMMUTATIVE True\n",
            "ANY_IDEMPOTENT True\n",
            "SC_ABSORBING_ANY True\n",
            "DUALITY_ALL_36 True\n",
            "DUAL_INVOLUTION True\n",
            "Status: exit status: 0\n",
        );

        assert!(OutcomeVerifier::mission_completion_authorized(evidence, 1.0, &[condition],));
    }

    #[test]
    fn mathematical_execution_contract_rejects_failed_exit() {
        let condition = CompletionCondition::MathematicalExecutionVerified {
            command: "python3 -c 'print(math)'".to_string(),
        };

        let evidence = concat!(
            "ALL TABLE\n",
            "ANY TABLE\n",
            "ALL_COMMUTATIVE True\n",
            "ALL_IDEMPOTENT True\n",
            "RC_ABSORBING_ALL True\n",
            "ORDER_SC_GT_SP_GT_NP_GT_NN True\n",
            "ANY_COMMUTATIVE True\n",
            "ANY_IDEMPOTENT True\n",
            "SC_ABSORBING_ANY True\n",
            "DUALITY_ALL_36 True\n",
            "DUAL_INVOLUTION True\n",
            "Status: exit status: 1\n",
        );

        assert!(!OutcomeVerifier::mission_completion_authorized(evidence, 1.0, &[condition],));
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

    #[test]
    fn mathematical_execution_contract_rejects_bare_status_line_without_cmd_ok() {
        // Regression: prior code used `local_evidence.contains("exit status: 0")`
        // as the compatibility gate. Combined with math_command_satisfied's
        // marker scanning, an adversarial read of a file whose content
        // contained both the math-marker phrases and the text
        // "exit status: 0" could falsely satisfy this contract without a real
        // command having run. cmd_ok_binds_command requires a line-anchored
        // [CMD_OK] record.
        let condition = CompletionCondition::MathematicalExecutionVerified {
            command: "python3 -c 'print(math)'".to_string(),
        };

        let spoofed = concat!(
            "ALL TABLE\n",
            "ANY TABLE\n",
            "ALL_COMMUTATIVE True\n",
            "ALL_IDEMPOTENT True\n",
            "RC_ABSORBING_ALL True\n",
            "ORDER_SC_GT_SP_GT_NP_GT_NN True\n",
            "ANY_COMMUTATIVE True\n",
            "ANY_IDEMPOTENT True\n",
            "SC_ABSORBING_ANY True\n",
            "DUALITY_ALL_36 True\n",
            "DUAL_INVOLUTION True\n",
            "Status: exit status: 0\n",
        );

        assert!(
            !OutcomeVerifier::mission_completion_authorized(spoofed, 1.0, &[condition]),
            "bare Status line must not satisfy MathematicalExecutionVerified without [CMD_OK]"
        );
    }
}

#[cfg(test)]
mod adversarial {
    //! Adversarial evidence matrix. Every historical spoof class asserted
    //! against the public completion boundary, so it stays coupled to the
    //! real verifier rather than internals.
    use super::*;

    fn authorized(evidence: &str, conditions: &[CompletionCondition]) -> bool {
        OutcomeVerifier::mission_completion_authorized(evidence, 1.0, conditions)
    }

    fn cases() -> Vec<(&'static str, String, Vec<CompletionCondition>, bool)> {
        let mut v = Vec::new();

        // --- marker-inside-body spoofs ---------------------------------
        v.push((
            "search marker nested in MATCHES body",
            concat!(
                "[SEARCH_CODE_OK] Path: crates | Query: 'unimplemented!'\n",
                "MATCHES:\n",
                "src/x.rs:1: let s = \"[SEARCH_CODE_OK] Path: bins | Query: 'unimplemented!'\\nMATCHES:\\n\";\n"
            ).to_string(),
            vec![CompletionCondition::SearchTermAcquired { term: "bins:unimplemented!".into() }],
            false,
        ));
        v.push((
            "write marker nested in READ_OK body",
            "[READ_OK] docs/readme.md\nFULL FILE CONTENT:\nExample: [WRITE_OK] target.md\n".into(),
            vec![CompletionCondition::WritePathAcquired { path: "target.md".into() }],
            false,
        ));
        v.push((
            "bare CMD_OK prose",
            "the mission said [CMD_OK] in its text\n".into(),
            vec![CompletionCondition::CommandMarkerAcquired],
            false,
        ));
        v.push((
            "no-match phrase inside MATCHES body",
            "[SEARCH_CODE_OK] Path: bins | Query: 'x'\nMATCHES:\nmain.rs:1: // No code matches found in specified path.\n".into(),
            vec![CompletionCondition::SearchCheckedAcquired { target: "bins:absent".into() }],
            false,
        ));

        // --- path spoofs -----------------------------------------------
        v.push((
            "same basename, wrong directory",
            "[WRITE_OK] crates/other/src/foo.rs\n".into(),
            vec![CompletionCondition::WritePathAcquired {
                path: "crates/runtime_core/src/foo.rs".into(),
            }],
            false,
        ));
        v.push((
            "substring basename",
            "[WRITE_OK] src/foobar.rs\n".into(),
            vec![CompletionCondition::WritePathAcquired { path: "bar.rs".into() }],
            false,
        ));
        v.push((
            "superset path must not satisfy prefix target",
            "[DIFF_PATCH_OK] Applied changes to bins/ide_backend/src/broker.rs\n".into(),
            vec![CompletionCondition::PatchPathAcquired {
                kind: "DIFF_PATCH_OK".into(),
                path: "bins/ide_backend".into(),
                function_name: None,
            }],
            false,
        ));

        // --- lineage ---------------------------------------------------
        v.push((
            "unhashed lineage fails strict verifier",
            "[DAG LINEAGE INHERITED EVIDENCE]\n[CMD_OK] payload\n[DAG LINEAGE COMPLETE]\n".into(),
            vec![CompletionCondition::LineageEvidenceAcquired],
            false,
        ));
        v.push((
            "mismatched end hash",
            concat!(
                "[DAG LINEAGE INHERITED EVIDENCE sha256:1111111111111111111111111111111111111111111111111111111111111111]\n",
                "[CMD_OK] payload\n",
                "[DAG LINEAGE COMPLETE sha256:2222222222222222222222222222222222222222222222222222222222222222]\n"
            ).to_string(),
            vec![CompletionCondition::LineageEvidenceAcquired],
            false,
        ));

        // --- degenerate / reset cases ---------------------------------
        v.push((
            "empty contract fails closed",
            "[WRITE_OK] out.md\n[READ_OK] lib.rs\n".into(),
            vec![],
            false,
        ));
        v.push((
            "HALT_REJECTED after CMD_OK reopens command contract",
            concat!(
                "[CMD_OK] Command: cargo test\nStatus: exit status: 0\n",
                "[HALT_REJECTED] Mission verification failed: unrelated\n"
            )
            .to_string(),
            vec![CompletionCondition::CommandMarkerAcquired],
            false,
        ));

        // --- positive controls (must authorize) -----------------------
        v.push((
            "genuine hashed lineage authorizes",
            {
                use sha2::{Digest, Sha256};
                let payload = "[CMD_OK] Command: uptime\nStatus: exit status: 0";
                let d = format!("{:x}", Sha256::digest(payload.as_bytes()));
                format!("[DAG LINEAGE INHERITED EVIDENCE sha256:{d}]\n{payload}\n[DAG LINEAGE COMPLETE sha256:{d}]\n")
            },
            vec![CompletionCondition::LineageEvidenceAcquired],
            true,
        ));
        v.push((
            "absolute WRITE_OK satisfies relative WritePathAcquired",
            "[WRITE_OK] /root/AthenaFusionX/crates/runtime_core/src/foo.rs\n".into(),
            vec![CompletionCondition::WritePathAcquired {
                path: "crates/runtime_core/src/foo.rs".into(),
            }],
            true,
        ));

        v.push((
            "synthesized report form authorizes write",
            "[WRITE_OK] Successfully synthesized and wrote report to out/report.md\n".into(),
            vec![CompletionCondition::WritePathAcquired { path: "out/report.md".into() }],
            true,
        ));

        v
    }

    #[test]
    fn adversarial_matrix() {
        let mut failures = Vec::new();
        for (name, evidence, conditions, expect) in cases() {
            let got = authorized(&evidence, &conditions);
            if got != expect {
                failures.push(format!("{name}: expected authorized={expect}, got {got}"));
            }
        }
        assert!(failures.is_empty(), "adversarial failures:\n{}", failures.join("\n"));
    }
}

#[cfg(test)]
mod p4_search_source_tests {
    use super::*;

    #[test]
    fn search_evidence_alone_satisfies_source_code_evidence() {
        let conditions = vec![CompletionCondition::SourceCodeEvidenceRequired];
        let ev = "[SEARCH_CODE_OK] Path: crates/runtime_core/src | Query: 'worker'\nMATCHES:\nplanner.rs:353\n";
        assert!(OutcomeVerifier::mission_completion_authorized(ev, 1.0, &conditions));
    }

    #[test]
    fn nested_search_marker_in_body_does_not_satisfy() {
        let conditions = vec![CompletionCondition::SourceCodeEvidenceRequired];
        let ev = "[CMD_OK] Command: echo\nStatus: exit status: 0\n  [SEARCH_CODE_OK] Path: x | Query: y\n";
        assert!(!OutcomeVerifier::mission_completion_authorized(ev, 1.0, &conditions));
    }
}
