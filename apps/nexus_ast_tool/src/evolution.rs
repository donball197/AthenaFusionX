//! Evolution Contract V1.
//!
//! This module defines the typed evidence and decision boundary for controlled
//! self-evolution. It does not execute mutations or security audits.
//!
//! Security evidence is deliberately tri-state:
//! PASS / FAIL / UNKNOWN.
//! UNKNOWN can never be interpreted as a successful security validation.

use serde::{Deserialize, Serialize};

pub const EVOLUTION_SCHEMA_VERSION: u32 = 1;

/// A persisted candidate produced during the report-only optimization phase.
///
/// This is descriptive evidence only. It does not authorize execution.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OptimizationCandidate {
    pub candidate_id: String,
    pub target: String,
    pub target_symbol: String,
    pub strategy: String,
    pub proposed_payload: String,
    pub evolution: EvolutionRecord,
}

/// Persisted report for Gate 11.6/11.7.
///
/// The report is the immutable handoff between report generation,
/// human approval, and isolated dry-run validation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OptimizationReport {
    pub schema_version: u32,
    pub report_identity: String,
    pub baseline: BuildIdentity,
    pub baseline_security_findings: usize,
    pub candidates: Vec<OptimizationCandidate>,
}

impl OptimizationReport {
    pub fn new(
        baseline: BuildIdentity,
        baseline_security_findings: usize,
        candidates: Vec<OptimizationCandidate>,
    ) -> Self {
        let mut report = Self {
            schema_version: EVOLUTION_SCHEMA_VERSION,
            report_identity: String::new(),
            baseline,
            baseline_security_findings,
            candidates,
        };
        report.report_identity = report.recompute_identity();
        report
    }

    /// Deterministic report identity over the report's substantive content.
    ///
    /// The identity intentionally excludes itself.
    pub fn recompute_identity(&self) -> String {
        let mut clone = self.clone();
        clone.report_identity.clear();

        let bytes = serde_json::to_vec(&clone)
            .expect("OptimizationReport contains only serializable fields");

        format!("report:v1|fnv1a={:016x}", stable_fingerprint(&bytes))
    }

    pub fn find_candidate(&self, candidate_id: &str) -> Option<&OptimizationCandidate> {
        self.candidates.iter().find(|candidate| candidate.candidate_id == candidate_id)
    }
}

/// Result of executing exactly one approved candidate inside an isolated copy.
///
/// This artifact never represents promotion. It is evidence from the dry-run
/// environment only.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DryRunReport {
    pub schema_version: u32,
    pub report_identity: String,
    pub approval_id: String,
    pub candidate_id: String,
    pub isolated_workspace: String,
    pub build_passed: bool,
    pub build_identity: String,
    pub tests_passed: Option<bool>,
    pub test_identity: Option<String>,
    pub baseline_security_findings: usize,
    pub candidate_security_findings: usize,
    pub security_regression: bool,
    pub authoritative_source_unchanged: bool,
    pub target_mutated: bool,
    pub promotion_performed: bool,
}

impl DryRunReport {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        report_identity: String,
        approval_id: String,
        candidate_id: String,
        isolated_workspace: String,
        build_passed: bool,
        build_identity: String,
        tests_passed: Option<bool>,
        test_identity: Option<String>,
        baseline_security_findings: usize,
        candidate_security_findings: usize,
        security_regression: bool,
        authoritative_source_unchanged: bool,
        target_mutated: bool,
        promotion_performed: bool,
    ) -> Self {
        Self {
            schema_version: EVOLUTION_SCHEMA_VERSION,
            report_identity,
            approval_id,
            candidate_id,
            isolated_workspace,
            build_passed,
            build_identity,
            tests_passed,
            test_identity,
            baseline_security_findings,
            candidate_security_findings,
            security_regression,
            authoritative_source_unchanged,
            target_mutated,
            promotion_performed,
        }
    }
}

/// Dependency-free deterministic fingerprint used for provenance identities.
///
/// This is an identity mechanism, not a cryptographic security primitive.
pub fn stable_fingerprint(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;

    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }

    hash
}

/// Calculate a deterministic source identity.
pub fn source_fingerprint(source: &str) -> String {
    format!("fnv1a:{:016x}", stable_fingerprint(source.as_bytes()))
}

/// Explicit human authorization for entering the isolated dry-run phase.
///
/// This is intentionally separate from `EvolutionDecision::Accept`.
/// Machine acceptance means only that the candidate satisfied the machine
/// evidence contract. It does NOT authorize execution.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HumanApproval {
    pub schema_version: u32,
    pub approval_id: String,
    pub candidate_id: String,
    pub report_identity: String,
    pub approved: bool,
    pub confirmation: String,
}

impl HumanApproval {
    pub const CONFIRMATION_TOKEN: &'static str = "APPROVE-DRY-RUN-V1";

    /// Create an approval only when the caller supplies the exact explicit
    /// confirmation token.
    pub fn authorize(
        candidate_id: impl Into<String>,
        report_identity: impl Into<String>,
        confirmation: &str,
    ) -> anyhow::Result<Self> {
        if confirmation != Self::CONFIRMATION_TOKEN {
            return Err(anyhow::anyhow!(
                "Human approval requires explicit confirmation token '{}'.",
                Self::CONFIRMATION_TOKEN
            ));
        }

        let candidate_id = candidate_id.into();
        let report_identity = report_identity.into();

        if candidate_id.trim().is_empty() {
            return Err(anyhow::anyhow!("Human approval requires a non-empty candidate identity."));
        }

        if report_identity.trim().is_empty() {
            return Err(anyhow::anyhow!("Human approval requires a non-empty report identity."));
        }

        let approval_id =
            format!("approval:v1|candidate={}|report={}", candidate_id, report_identity);

        Ok(Self {
            schema_version: EVOLUTION_SCHEMA_VERSION,
            approval_id,
            candidate_id,
            report_identity,
            approved: true,
            confirmation: Self::CONFIRMATION_TOKEN.into(),
        })
    }

    /// Verify that this artifact authorizes exactly the requested candidate
    /// and report. A valid artifact can later be consumed by the dry-run gate.
    pub fn authorizes(&self, candidate_id: &str, report_identity: &str) -> bool {
        self.schema_version == EVOLUTION_SCHEMA_VERSION
            && self.approved
            && self.candidate_id == candidate_id
            && self.report_identity == report_identity
            && self.confirmation == Self::CONFIRMATION_TOKEN
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StudyEvidenceSnapshot {
    pub source_files: usize,
    pub function_count: usize,
    pub entity_count: usize,
    pub dependency_count: usize,
    pub workspace_dependency_count: usize,
    pub resolved_workspace_dependency_count: usize,
    pub parse_health_percent: f64,
    pub clean_files: usize,
    pub read_failures: usize,
    pub tree_sitter_diagnostics: usize,
}

impl StudyEvidenceSnapshot {
    /// Adapt the already-canonical deterministic study evidence into the
    /// evolution contract. No repository metrics are recalculated here.
    pub fn from_evidence(evidence: &crate::evidence::EvidencePacketSet) -> Self {
        Self {
            source_files: evidence.overview.source_files,
            function_count: evidence.overview.functions,
            entity_count: evidence.overview.structural_entities,
            dependency_count: evidence.dependencies.total_dependencies,
            workspace_dependency_count: evidence.dependencies.workspace_dependencies,
            resolved_workspace_dependency_count: evidence
                .dependencies
                .resolved_workspace_dependencies,
            parse_health_percent: evidence.parse_health.parse_health_percent,
            clean_files: evidence.parse_health.clean_files,
            read_failures: evidence.parse_health.read_failures,
            tree_sitter_diagnostics: evidence.parse_health.diagnostics.len(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StudyComparison {
    pub baseline: StudyEvidenceSnapshot,
    pub candidate: StudyEvidenceSnapshot,
    pub delta: StructuralDelta,
}

impl StudyComparison {
    pub fn between(baseline: StudyEvidenceSnapshot, candidate: StudyEvidenceSnapshot) -> Self {
        let delta = StructuralDelta {
            source_files_delta: candidate.source_files as i64 - baseline.source_files as i64,
            function_count_delta: candidate.function_count as i64 - baseline.function_count as i64,
            entity_count_delta: candidate.entity_count as i64 - baseline.entity_count as i64,
            dependency_count_delta: candidate.dependency_count as i64
                - baseline.dependency_count as i64,
            parse_health_improved: Some(
                candidate.parse_health_percent > baseline.parse_health_percent,
            ),
        };

        Self { baseline, candidate, delta }
    }
}

/// Convert a complete study report into the evolution layer's canonical
/// structural snapshot.
pub fn study_snapshot(report: &crate::study::StudyReport) -> StudyEvidenceSnapshot {
    let evidence = crate::evidence::build_evidence_packets(report);
    StudyEvidenceSnapshot::from_evidence(&evidence)
}

/// Conservative read-only security comparison for Evolution V1.
///
/// The existing security scanner reports raw findings, not exploitability.
/// Therefore this adapter deliberately does not invent `actionable_findings`.
///
/// V1 semantics:
/// - candidate findings < baseline -> PASS
/// - candidate findings == baseline == 0 -> PASS
/// - candidate findings > baseline -> FAIL
/// - candidate findings == baseline > 0 -> UNKNOWN
///
/// UNKNOWN is never accepted by `evaluate_v1`.
pub fn compare_security_findings(
    baseline: &[crate::polyglot::SecurityFinding],
    candidate: &[crate::polyglot::SecurityFinding],
) -> SecurityEvidence {
    let baseline_count = baseline.len();
    let candidate_count = candidate.len();

    let regression = candidate_count > baseline_count;

    let status = if regression {
        SecurityStatus::Fail
    } else if candidate_count < baseline_count || candidate_count == 0 {
        SecurityStatus::Pass
    } else {
        SecurityStatus::Unknown
    };

    let audit_identity = Some(format!(
        "security-scan:v1|baseline-findings={}|candidate-findings={}|regression={}",
        baseline_count, candidate_count, regression
    ));

    SecurityEvidence {
        status,
        findings: candidate_count,
        actionable_findings: 0,
        regression,
        audit_identity,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BuildIdentity {
    pub target: String,
    pub source_fingerprint: String,
    pub session_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Hypothesis {
    pub target_symbol: String,
    pub strategy: String,
    pub expected_capability_improvement: String,
    pub expected_structural_effect: String,
    pub expected_security_effect: String,
    pub proposed_payload: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CandidateIdentity {
    pub candidate_id: String,
    pub baseline_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FunctionalEvidence {
    pub build_passed: bool,
    pub tests_passed: Option<bool>,
    pub test_identity: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SecurityStatus {
    Pass,
    Fail,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SecurityEvidence {
    pub status: SecurityStatus,
    pub findings: usize,
    pub actionable_findings: usize,
    pub regression: bool,
    pub audit_identity: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct StructuralDelta {
    pub source_files_delta: i64,
    pub function_count_delta: i64,
    pub entity_count_delta: i64,
    pub dependency_count_delta: i64,
    pub parse_health_improved: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CapabilityEvidence {
    pub measurable_improvement: bool,
    pub description: String,
    /// Stable identity for the evidence supporting the capability claim.
    pub evidence_identity: Option<String>,
    /// Provenance describing how the capability claim was established.
    pub source: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Comparison {
    pub structural: StructuralDelta,
    pub capability: CapabilityEvidence,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum EvolutionDecision {
    Accept { rationale: String },
    Reject { reason: String },
    Abstain { reason: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EvolutionRecord {
    pub schema_version: u32,
    pub baseline: BuildIdentity,
    pub hypothesis: Hypothesis,
    pub candidate: CandidateIdentity,
    pub functional: FunctionalEvidence,
    pub security: SecurityEvidence,
    pub comparison: Comparison,
    pub decision: EvolutionDecision,
}

impl EvolutionRecord {
    pub fn new(
        baseline: BuildIdentity,
        hypothesis: Hypothesis,
        candidate: CandidateIdentity,
        functional: FunctionalEvidence,
        security: SecurityEvidence,
        comparison: Comparison,
        decision: EvolutionDecision,
    ) -> Self {
        Self {
            schema_version: EVOLUTION_SCHEMA_VERSION,
            baseline,
            hypothesis,
            candidate,
            functional,
            security,
            comparison,
            decision,
        }
    }
}

/// Conservative V1 acceptance gate.
///
/// Security failure always rejects.
/// Security uncertainty abstains.
/// Functional failure rejects.
/// No measurable capability improvement rejects.
///
/// This function intentionally does not calculate an opaque aggregate score.
pub fn evaluate_v1(
    functional: &FunctionalEvidence,
    security: &SecurityEvidence,
    comparison: &Comparison,
) -> EvolutionDecision {
    if security.regression || matches!(security.status, SecurityStatus::Fail) {
        return EvolutionDecision::Reject {
            reason: "Security regression or failed security validation.".into(),
        };
    }

    if matches!(security.status, SecurityStatus::Unknown) {
        return EvolutionDecision::Abstain {
            reason: "Security validation is insufficient for acceptance.".into(),
        };
    }

    if !functional.build_passed {
        return EvolutionDecision::Reject { reason: "Candidate failed build validation.".into() };
    }

    if functional.tests_passed == Some(false) {
        return EvolutionDecision::Reject {
            reason: "Candidate failed required functional tests.".into(),
        };
    }

    if !comparison.capability.measurable_improvement {
        return EvolutionDecision::Reject {
            reason: "Candidate provides no measurable capability improvement.".into(),
        };
    }

    if comparison.capability.evidence_identity.is_none() || comparison.capability.source.is_none() {
        return EvolutionDecision::Abstain {
            reason: "Capability improvement lacks sufficient evidence provenance.".into(),
        };
    }

    if functional.tests_passed != Some(true)
        || functional.test_identity.is_none()
        || comparison.capability.evidence_identity != functional.test_identity
    {
        return EvolutionDecision::Abstain {
            reason: "Capability evidence is not bound to successful functional validation.".into(),
        };
    }

    EvolutionDecision::Accept {
        rationale: "Candidate passes functional and security gates and provides measurable capability improvement bound to successful functional validation.".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn functional_ok() -> FunctionalEvidence {
        FunctionalEvidence {
            build_passed: true,
            tests_passed: Some(true),
            test_identity: Some("test-digest".into()),
        }
    }

    fn security_ok() -> SecurityEvidence {
        SecurityEvidence {
            status: SecurityStatus::Pass,
            findings: 0,
            actionable_findings: 0,
            regression: false,
            audit_identity: Some("audit-1".into()),
        }
    }

    fn comparison_ok() -> Comparison {
        Comparison {
            structural: StructuralDelta::default(),
            capability: CapabilityEvidence {
                measurable_improvement: true,
                description: "Improved capability".into(),
                evidence_identity: Some("test-digest".into()),
                source: Some("functional validation".into()),
            },
        }
    }

    #[test]
    fn human_approval_requires_explicit_confirmation() {
        let result =
            HumanApproval::authorize("candidate:v1|symbol=test|index=0", "report:v1|test", "yes");

        assert!(result.is_err());
    }

    #[test]
    fn human_approval_binds_candidate_and_report() {
        let approval = HumanApproval::authorize(
            "candidate:v1|symbol=test|index=0",
            "report:v1|test",
            HumanApproval::CONFIRMATION_TOKEN,
        )
        .expect("explicit approval should succeed");

        assert!(approval.approved);
        assert!(approval.authorizes("candidate:v1|symbol=test|index=0", "report:v1|test",));

        assert!(!approval.authorizes("candidate:v1|symbol=other|index=0", "report:v1|test",));

        assert!(!approval.authorizes("candidate:v1|symbol=test|index=0", "report:v1|other",));
    }

    #[test]
    fn security_regression_always_rejects() {
        let mut security = security_ok();
        security.regression = true;

        let decision = evaluate_v1(&functional_ok(), &security, &comparison_ok());

        assert!(matches!(decision, EvolutionDecision::Reject { .. }));
    }

    #[test]
    fn unknown_security_abstains() {
        let mut security = security_ok();
        security.status = SecurityStatus::Unknown;

        let decision = evaluate_v1(&functional_ok(), &security, &comparison_ok());

        assert!(matches!(decision, EvolutionDecision::Abstain { .. }));
    }

    #[test]
    fn failed_build_rejects() {
        let mut functional = functional_ok();
        functional.build_passed = false;

        let decision = evaluate_v1(&functional, &security_ok(), &comparison_ok());

        assert!(matches!(decision, EvolutionDecision::Reject { .. }));
    }

    #[test]
    fn failed_tests_reject() {
        let mut functional = functional_ok();
        functional.tests_passed = Some(false);

        let decision = evaluate_v1(&functional, &security_ok(), &comparison_ok());

        assert!(matches!(decision, EvolutionDecision::Reject { .. }));
    }

    #[test]
    fn no_capability_gain_rejects() {
        let mut comparison = comparison_ok();
        comparison.capability.measurable_improvement = false;

        let decision = evaluate_v1(&functional_ok(), &security_ok(), &comparison);

        assert!(matches!(decision, EvolutionDecision::Reject { .. }));
    }

    #[test]
    fn capability_without_evidence_identity_abstains() {
        let mut comparison = comparison_ok();
        comparison.capability.evidence_identity = None;

        let decision = evaluate_v1(&functional_ok(), &security_ok(), &comparison);

        assert!(matches!(decision, EvolutionDecision::Abstain { .. }));
    }

    #[test]
    fn capability_without_source_abstains() {
        let mut comparison = comparison_ok();
        comparison.capability.source = None;

        let decision = evaluate_v1(&functional_ok(), &security_ok(), &comparison);

        assert!(matches!(decision, EvolutionDecision::Abstain { .. }));
    }

    #[test]
    fn capability_without_provenance_cannot_be_accepted() {
        let comparison = Comparison {
            structural: StructuralDelta::default(),
            capability: CapabilityEvidence {
                measurable_improvement: true,
                description: "Model claims improvement".into(),
                evidence_identity: None,
                source: None,
            },
        };

        let decision = evaluate_v1(&functional_ok(), &security_ok(), &comparison);

        assert!(matches!(decision, EvolutionDecision::Abstain { .. }));
    }

    #[test]
    fn capability_evidence_must_match_functional_test_identity() {
        let mut comparison = comparison_ok();
        comparison.capability.evidence_identity = Some("different-test-digest".into());

        let decision = evaluate_v1(&functional_ok(), &security_ok(), &comparison);

        assert!(matches!(decision, EvolutionDecision::Abstain { .. }));
    }

    #[test]
    fn valid_candidate_is_accepted() {
        let decision = evaluate_v1(&functional_ok(), &security_ok(), &comparison_ok());

        assert!(matches!(decision, EvolutionDecision::Accept { .. }));
    }

    #[test]
    fn study_comparison_calculates_structural_deltas() {
        let baseline = StudyEvidenceSnapshot {
            source_files: 147,
            function_count: 496,
            entity_count: 531,
            dependency_count: 27,
            workspace_dependency_count: 13,
            resolved_workspace_dependency_count: 13,
            parse_health_percent: 99.0,
            clean_files: 146,
            read_failures: 0,
            tree_sitter_diagnostics: 1,
        };

        let candidate = StudyEvidenceSnapshot {
            source_files: 148,
            function_count: 500,
            entity_count: 536,
            dependency_count: 26,
            workspace_dependency_count: 12,
            resolved_workspace_dependency_count: 12,
            parse_health_percent: 100.0,
            clean_files: 148,
            read_failures: 0,
            tree_sitter_diagnostics: 0,
        };

        let comparison = StudyComparison::between(baseline, candidate);

        assert_eq!(comparison.delta.source_files_delta, 1);
        assert_eq!(comparison.delta.function_count_delta, 4);
        assert_eq!(comparison.delta.entity_count_delta, 5);
        assert_eq!(comparison.delta.dependency_count_delta, -1);
        assert_eq!(comparison.delta.parse_health_improved, Some(true));
    }

    #[test]
    fn study_comparison_detects_no_parse_health_improvement() {
        let baseline = StudyEvidenceSnapshot {
            source_files: 10,
            function_count: 20,
            entity_count: 25,
            dependency_count: 4,
            workspace_dependency_count: 2,
            resolved_workspace_dependency_count: 2,
            parse_health_percent: 100.0,
            clean_files: 10,
            read_failures: 0,
            tree_sitter_diagnostics: 0,
        };

        let candidate = baseline.clone();

        let comparison = StudyComparison::between(baseline, candidate);

        assert_eq!(comparison.delta.parse_health_improved, Some(false));
        assert_eq!(comparison.delta.function_count_delta, 0);
        assert_eq!(comparison.delta.dependency_count_delta, 0);
    }

    #[test]
    fn evolution_record_uses_schema_version_one() {
        let record = EvolutionRecord::new(
            BuildIdentity {
                target: "target.rs".into(),
                source_fingerprint: "sha256:test".into(),
                session_id: "session-1".into(),
            },
            Hypothesis {
                target_symbol: "example".into(),
                strategy: "ReplaceFunctionBody".into(),
                expected_capability_improvement: "improve".into(),
                expected_structural_effect: "none".into(),
                expected_security_effect: "preserve".into(),
                proposed_payload: "return 1;".into(),
            },
            CandidateIdentity {
                candidate_id: "candidate-1".into(),
                baseline_id: "baseline-1".into(),
            },
            functional_ok(),
            security_ok(),
            comparison_ok(),
            EvolutionDecision::Accept { rationale: "test".into() },
        );

        assert_eq!(record.schema_version, EVOLUTION_SCHEMA_VERSION);
    }
}
