use crate::mutation_evidence::MutationEvidence;

pub struct MutationEvidenceAudit;

impl MutationEvidenceAudit {
    pub fn summarize(evidence: &MutationEvidence) -> String {
        format!(
            "file={} function={} check={} tests={} similarity={} regions={}",
            evidence.target_file,
            evidence.target_function,
            evidence.cargo_check_passed,
            evidence.tests_passed,
            evidence.ast_similarity,
            evidence.changed_regions
        )
    }
}
