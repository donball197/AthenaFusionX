use nexus_evidence_requirements::{
    evaluate, ActionIdentity, Assessment, Evidence, Scope, SuccessfulExecution, TargetId,
};

fn evidence(success: bool) -> Evidence {
    Evidence::new(
        ActionIdentity("test-action".to_string()),
        Some(TargetId("test-target".to_string())),
        Some(Scope("test-scope".to_string())),
        success,
        Some("test observation".to_string()),
    )
}

#[test]
fn absence_is_nn_and_predicate_is_not_called() {
    let predicate = SuccessfulExecution;
    assert_eq!(evaluate(&predicate, None), Assessment::NN);
}

#[test]
fn successful_evidence_is_sc() {
    let predicate = SuccessfulExecution;
    let ev = evidence(true);

    assert_eq!(evaluate(&predicate, Some(&ev)), Assessment::SC);
}

#[test]
fn failed_evidence_is_rc() {
    let predicate = SuccessfulExecution;
    let ev = evidence(false);

    assert_eq!(evaluate(&predicate, Some(&ev)), Assessment::RC);
}
