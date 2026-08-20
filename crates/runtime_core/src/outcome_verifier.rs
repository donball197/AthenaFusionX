use crate::cognitive_events::TelemetryArtifact;
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

    pub fn verify(_node_id: &str, local_evidence: &str) -> CompletionStatus {
        // ⚡ STRICT EXTERNAL PROOF REQUIREMENT (PHASE 2)
        // Check if the executor captured and appended a serialized TelemetryArtifact
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

        // Fallback for non-mutation writes
        if local_evidence.contains("[WRITE_OK]")
            || local_evidence.contains("[DIFF_PATCH_OK]")
            || local_evidence.contains("[PATCH_FUNCTION_OK]")
        {
            CompletionStatus::Approved
        } else {
            CompletionStatus::Rejected(
                "Missing verified TelemetryArtifact side-channel or execution marker.".to_string(),
            )
        }
    }
}

// Helper to extract JSON artifact appended by the RoboticExecutor
fn extract_telemetry(evidence: &str) -> Option<TelemetryArtifact> {
    for line in evidence.lines() {
        if line.starts_with("[TELEMETRY_PAYLOAD]") {
            let raw_json = line.trim_start_matches("[TELEMETRY_PAYLOAD]").trim();
            if let Ok(artifact) = serde_json::from_str::<TelemetryArtifact>(raw_json) {
                return Some(artifact);
            }
        }
    }
    None
}
