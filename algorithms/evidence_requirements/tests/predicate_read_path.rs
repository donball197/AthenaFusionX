use nexus_evidence_requirements::{
    evaluate, ActionIdentity, Assessment, Evidence, ReadPathEvidenceEstablished, Scope, TargetId,
};

fn pred() -> ReadPathEvidenceEstablished {
    ReadPathEvidenceEstablished {
        target: TargetId("src/main.rs".into()),
        scope: Scope("crate".into()),
    }
}

fn ev(target: Option<&str>, scope: Option<&str>, success: bool) -> Evidence {
    Evidence::new(
        ActionIdentity("read_file:src/main.rs".into()),
        target.map(|t| TargetId(t.into())),
        scope.map(|s| Scope(s.into())),
        success,
        Some("obs".into()),
    )
}

#[test]
fn absence_is_nn() {
    assert_eq!(evaluate(&pred(), None), Assessment::NN);
}

#[test]
fn matching_success_is_sc() {
    let e = ev(Some("src/main.rs"), Some("crate"), true);
    assert_eq!(evaluate(&pred(), Some(&e)), Assessment::SC);
}

#[test]
fn matching_failure_is_rc() {
    let e = ev(Some("src/main.rs"), Some("crate"), false);
    assert_eq!(evaluate(&pred(), Some(&e)), Assessment::RC);
}

#[test]
fn wrong_target_is_nn() {
    let e = ev(Some("other.rs"), Some("crate"), true);
    assert_eq!(evaluate(&pred(), Some(&e)), Assessment::NN);
}

#[test]
fn missing_scope_is_nn() {
    let e = ev(Some("src/main.rs"), None, true);
    assert_eq!(evaluate(&pred(), Some(&e)), Assessment::NN);
}
