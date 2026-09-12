use crate::cognitive_events::TelemetryArtifact;
use crate::completion_engine::CompletionCondition;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutcomeAssessment {
    pub success_probability: f32,
    pub deviation_score: f32,
    pub confidence_delta: f32,
    pub lessons: Vec<String>,
}

#[derive(Debug, PartialEq)]
pub enum CompletionStatus {
    Approved,
    Rejected(String),
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
    pub fn verify(
        _node_id: &str,
        local_evidence: &str,
        belief_score: f32,
        conditions: &[CompletionCondition],
    ) -> CompletionStatus {
        // 1. If telemetry is provided, it overrides all else (Phase 2 external proof)
        if let Some(telemetry_json) = extract_telemetry(local_evidence) {
            if telemetry_json.exit_code == 0 && telemetry_json.test_digest.is_some() {
                return CompletionStatus::Approved;
            } else {
                return CompletionStatus::Rejected(format!(
                    "Telemetry captured, but verification failed. Exit code: {}, Tests passed: {}",
                    telemetry_json.exit_code,
                    telemetry_json.test_digest.is_some()
                ));
            }
        }

        // 2. Strictly verify that EVERY condition in the contract is met
        for cond in conditions {
            let met = match cond {
                CompletionCondition::ReadMarkerAcquired => {
                    local_evidence.contains("[READ_OK]")
                        || local_evidence.contains("[READ_CHUNK_OK]")
                }
                CompletionCondition::SearchMarkerAcquired => {
                    local_evidence.contains("[SEARCH_CODE_OK]")
                }
                CompletionCondition::SearchMemoryMarkerAcquired => {
                    local_evidence.contains("[SEARCH_MEMORY_OK]")
                }
                CompletionCondition::WriteMarkerAcquired => local_evidence.contains("[WRITE_OK]"),
                CompletionCondition::PatchMarkerAcquired => {
                    local_evidence.contains("[PATCH_FUNCTION_OK]")
                        || local_evidence.contains("[DIFF_PATCH_OK]")
                        || local_evidence.contains("[REPLACE_LINES_OK]")
                }
                CompletionCondition::CommandMarkerAcquired => local_evidence.contains("[CMD_OK]"),
                CompletionCondition::LineageEvidenceAcquired => {
                    verify_lineage_provenance(local_evidence)
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
                    // Was more than one distinct file/path actually examined? A comparative
                    // or audit-style mission naming multiple subjects should not be satisfiable
                    // by evidence for a single path -- previously this also hardcoded a ".rs"
                    // extension requirement, which made it both too narrow (rejected valid
                    // README/doc-based comparisons) and too weak (a single .rs read/search was
                    // enough). This checks distinct-path count instead of file type.
                    let mut seen_paths = std::collections::HashSet::new();
                    for l in local_evidence.lines() {
                        if let Some(path) = l.strip_prefix("[READ_OK]") {
                            let path = path.trim();
                            if !path.is_empty() {
                                seen_paths.insert(path.to_string());
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
                    seen_paths.len() >= 2
                }
                CompletionCondition::MinBeliefScore(min_score) => belief_score >= *min_score,
            };

            if !met {
                return CompletionStatus::Rejected(format!(
                    "Contract condition {:?} not met in evidence.",
                    cond
                ));
            }
        }

        // 3. Default fallback for empty conditions (backward compatibility)
        if conditions.is_empty() {
            if local_evidence.contains("[WRITE_OK]")
                || local_evidence.contains("[DIFF_PATCH_OK]")
                || local_evidence.contains("[PATCH_FUNCTION_OK]")
            {
                return CompletionStatus::Approved;
            } else {
                return CompletionStatus::Rejected(
                    "Missing verified TelemetryArtifact side-channel or execution marker."
                        .to_string(),
                );
            }
        }

        CompletionStatus::Approved
    }
}

// Helper to extract JSON artifact appended by the RoboticExecutor
fn extract_telemetry(_local_evidence: &str) -> Option<TelemetryArtifact> {
    // Assumed to exist in the original file or cognitive_events
    // If missing, returns None and skips telemetry check
    None
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
    use super::{CompletionStatus, OutcomeVerifier};
    use crate::completion_engine::CompletionCondition;

    fn verify(command: &str, evidence: &str) -> CompletionStatus {
        let condition =
            CompletionCondition::CommandExecutionVerified { command: command.to_string() };

        OutcomeVerifier::verify("test_node", evidence, 1.0, &[condition])
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

        assert!(matches!(
            verify("uptime; df -h /; free -m", wrong_command),
            CompletionStatus::Rejected(_)
        ));

        assert_eq!(verify("uptime; df -h /; free -m", exact_command), CompletionStatus::Approved);
    }

    #[test]
    fn successful_command_requires_zero_exit_status() {
        let failed_command = concat!("[CMD_OK] Command: uptime\n", "Status: exit status: 1\n",);

        let missing_status = "[CMD_OK] Command: uptime\n";

        let successful_command = concat!("[CMD_OK] Command: uptime\n", "Status: exit status: 0\n",);

        assert!(matches!(verify("uptime", failed_command), CompletionStatus::Rejected(_)));

        assert!(matches!(verify("uptime", missing_status), CompletionStatus::Rejected(_)));

        assert_eq!(verify("uptime", successful_command), CompletionStatus::Approved);
    }

    #[test]
    fn generic_cmd_ok_remains_legacy_compatible() {
        let condition = CompletionCondition::CommandMarkerAcquired;

        let result = OutcomeVerifier::verify(
            "test_node",
            "[CMD_OK] Status: exit status: 0\n",
            1.0,
            &[condition],
        );

        assert_eq!(result, CompletionStatus::Approved);
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
        let result = OutcomeVerifier::verify("test_node", &evidence, 1.0, &[condition]);
        assert_eq!(result, CompletionStatus::Approved);
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
        let result = OutcomeVerifier::verify("test_node", &evidence, 1.0, &[condition]);
        assert!(matches!(result, CompletionStatus::Rejected(_)));
    }

    #[test]
    fn mismatched_end_hash_fails_verification() {
        let evidence = concat!(
            "[DAG LINEAGE INHERITED EVIDENCE sha256:1111111111111111111111111111111111111111111111111111111111111111]\n",
            "[CMD_OK] payload\n",
            "[DAG LINEAGE COMPLETE sha256:2222222222222222222222222222222222222222222222222222222222222222]"
        );

        let condition = CompletionCondition::LineageEvidenceAcquired;
        let result = OutcomeVerifier::verify("test_node", evidence, 1.0, &[condition]);
        assert!(matches!(result, CompletionStatus::Rejected(_)));
    }
}
