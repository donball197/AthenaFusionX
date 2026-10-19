use crate::completion_engine::CompletionCondition;
use crate::evidence_markers::{has_mutation_marker, write_path_satisfied};

const NOOP: &str = "[WRITE_NOOP] data.json -- content identical to existing file\n";

#[test]
fn noop_for_the_matching_path_satisfies_the_write() {
    assert!(write_path_satisfied(NOOP, "data.json"));
}

#[test]
fn noop_for_another_path_does_not() {
    assert!(!write_path_satisfied(NOOP, "other.json"));
}

#[test]
fn noop_for_a_longer_file_name_does_not_match() {
    let ev = "[WRITE_NOOP] data.json.bak -- content identical to existing file\n";
    assert!(!write_path_satisfied(ev, "data.json"));
}

#[test]
fn existing_write_ok_forms_still_satisfy() {
    assert!(write_path_satisfied("[WRITE_OK] report.md\n", "report.md"));
    assert!(write_path_satisfied(
        "[WRITE_OK] Successfully synthesized and wrote report to report.md\n",
        "report.md"
    ));
    assert!(!write_path_satisfied("[WRITE_OK] other.md\n", "report.md"));
    assert!(!write_path_satisfied("", "report.md"));
}

#[test]
fn a_noop_is_not_a_mutation_marker() {
    assert!(!has_mutation_marker(NOOP));
}

#[test]
fn outcome_verifier_accepts_a_noop_for_write_path_acquired() {
    let conditions = vec![CompletionCondition::WritePathAcquired { path: "data.json".to_string() }];
    let unmet = crate::outcome_verifier::OutcomeVerifier::unmet_conditions(NOOP, 0.9, &conditions);
    assert!(unmet.is_empty(), "unmet: {:?}", unmet);
}
