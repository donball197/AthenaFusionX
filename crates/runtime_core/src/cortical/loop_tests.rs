//! Unit tests for `CorticalLoop`. Declared in cortical_loop.rs via
//! `#[path]` so this stays a child module (private items remain visible).

use super::CorticalLoop;
use crate::agent_protocol::AgentAction;
use crate::cognition::epistemic_session::EpistemicSession;

#[test]
fn patch_function_key_is_body_sensitive() {
    let base = AgentAction::PatchFunction {
        path: "apps/math_utils/src/lib.rs".into(),
        function_name: "add".into(),
        new_body: "pub fn add(a: i32, b: i32) -> i32 { a + b }".into(),
        reasoning: "first".into(),
    };
    let same_body = AgentAction::PatchFunction {
        path: "apps/math_utils/src/lib.rs".into(),
        function_name: "add".into(),
        new_body: "pub fn add(a: i32, b: i32) -> i32 { a + b }".into(),
        reasoning: "retry with different reasoning".into(),
    };
    let diff_body = AgentAction::PatchFunction {
        path: "apps/math_utils/src/lib.rs".into(),
        function_name: "add".into(),
        new_body: "pub fn add(a: i32, b: i32) -> i32 {\n    a + b\n}".into(),
        reasoning: "first".into(),
    };

    let k_base = crate::cortical::action_identity::action_key(&base);
    let k_same = crate::cortical::action_identity::action_key(&same_body);
    let k_diff = crate::cortical::action_identity::action_key(&diff_body);

    assert_eq!(k_base, k_same, "reasoning must not affect the anti-loop key");
    assert_ne!(k_base, k_diff, "different new_body must produce a different key");
    assert!(
        k_base.starts_with("patch_function:apps/math_utils/src/lib.rs:add:"),
        "key must carry the patch_function prefix, path, and function"
    );
}

#[tokio::test]
async fn classify_patch_action_detects_real_mutation() {
    let dir = std::env::temp_dir().join(format!("nexus_test_mutation_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("lib.rs");
    std::fs::write(&path, "pub fn add(a: i32, b: i32) -> i32 {\n    a + 0 + b\n}\n").unwrap();

    let action = AgentAction::PatchFunction {
        path: path.to_string_lossy().into_owned(),
        function_name: "add".into(),
        new_body: "pub fn add(a: i32, b: i32) -> i32 { a + b }".into(),
        reasoning: "test".into(),
    };

    match super::classify_patch_action(&action).await.unwrap() {
        super::PatchClass::Mutation { function_name, .. } => {
            assert_eq!(function_name, "add");
        }
        other => panic!("expected Mutation, got {:?}", other),
    }

    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn classify_patch_action_detects_idempotent_body() {
    let dir =
        std::env::temp_dir().join(format!("nexus_test_idempotent_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("lib.rs");
    std::fs::write(&path, "pub fn add(a: i32, b: i32) -> i32 {\n    a + b\n}\n").unwrap();

    let action = AgentAction::PatchFunction {
        path: path.to_string_lossy().into_owned(),
        function_name: "add".into(),
        new_body: "pub fn add(a: i32, b: i32) -> i32 { a + b }".into(),
        reasoning: "test".into(),
    };

    match super::classify_patch_action(&action).await.unwrap() {
        super::PatchClass::Idempotent { function_name, .. } => {
            assert_eq!(function_name, "add");
        }
        other => panic!("expected Idempotent, got {:?}", other),
    }

    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn classify_patch_action_ignores_non_patch_actions() {
    let action = AgentAction::ReadFile { path: "whatever.rs".into(), reasoning: "test".into() };
    assert!(matches!(
        super::classify_patch_action(&action).await.unwrap(),
        super::PatchClass::NotPatch
    ));
}

#[test]
fn search_memory_strategy_requires_matching_search_memory_action() {
    let query =
        "host observations involving uptime, filesystem, available memory, and load average";
    let strategy = format!("search_memory:{}", query);

    let matching = AgentAction::SearchMemory {
        query: query.to_string(),
        reasoning: "regression test".to_string(),
    };

    let wrong_action = AgentAction::ReadFile {
        path: query.to_string(),
        reasoning: "regression test".to_string(),
    };

    assert!(crate::cortical::strategy_match::action_matches_strategy(&strategy, &matching));
    assert!(!crate::cortical::strategy_match::action_matches_strategy(
        &strategy,
        &wrong_action
    ));
}

#[test]
fn parse_failure_never_manufactures_a_typed_halt() {
    assert!(crate::cortical::util::should_synthesize_halt("halt"));
    assert!(crate::cortical::util::should_synthesize_halt("The task is done, halt."));
    assert!(!crate::cortical::util::should_synthesize_halt(r#"[{"type":"halt","reasoning":{"a":1}}]"#));
    assert!(!crate::cortical::util::should_synthesize_halt("no stop word here"));
}

#[test]
fn extract_named_target_read_ok() {
    // Reproduces the 2026-09-14 audit run's turn-1 mission text.
    let mission = "OBJECTIVE: inspect\n\nMANDATORY NODE SUCCESS CRITERIA:\nREAD_OK:crates/runtime_core/src/cortical_loop.rs\n\nNODE COMPLETION RULE: ...";
    assert_eq!(
        crate::cortical::util::extract_named_target(mission, "READ_OK:").as_deref(),
        Some("crates/runtime_core/src/cortical_loop.rs")
    );
}

#[test]
fn read_only_contract_never_enters_write_phase() {
    // Exercise the production helper rather than duplicating its
    // predicate here. If !contract.read_only is ever removed from
    // CorticalLoop::write_phase_ready, this test fails.
    let mission = "READ-ONLY audit.";
    let contract =
        crate::completion_engine::SubgoalCompletionEngine::infer_contract("t", mission);

    let unwritten: Vec<&str> =
        contract.required_artifact_paths.iter().map(|a| a.path.as_str()).collect();

    assert!(!CorticalLoop::write_phase_ready(&contract, &unwritten, true));
}

#[test]
fn floor_hypothesis_belief_after_failure_restores_threshold() {
    // Hypothesis was selectable before the evaluation (0.50),
    // collapsed to 0.05 after a single failed action, and must be
    // floored back to the selection threshold.
    let mut session = EpistemicSession::new("test");
    let h_id = session.graph.add_node(
        crate::cognition_graph::NodeKind::Hypothesis,
        "test hypothesis",
        0.05,
        100,
    );
    CorticalLoop::floor_hypothesis_belief_after_failure(&mut session, h_id, 0.50, 0.15);
    assert_eq!(session.graph.belief_of(h_id), Some(0.15));
}

#[test]
fn floor_hypothesis_belief_after_failure_leaves_dead_hypotheses_alone() {
    // Hypothesis was already below the threshold before the
    // evaluation (0.10 < 0.15). The floor must not resurrect it.
    let mut session = EpistemicSession::new("test");
    let h_id = session.graph.add_node(
        crate::cognition_graph::NodeKind::Hypothesis,
        "test hypothesis",
        0.05,
        100,
    );
    CorticalLoop::floor_hypothesis_belief_after_failure(&mut session, h_id, 0.10, 0.15);
    assert_eq!(session.graph.belief_of(h_id), Some(0.05));
}

#[test]
fn decay_hypothesis_belief_crosses_selection_floor() {
    // Pre-decay 0.30, delta 0.20 -> 0.10: the belief CROSSES
    // SELECT_FLOOR and becomes unselectable (F4: no clamp).
    let mut session = EpistemicSession::new("test");
    let h_id =
        session.graph.add_node(crate::cognition_graph::NodeKind::Hypothesis, "h", 0.30, 100);
    CorticalLoop::decay_hypothesis_belief(&mut session, h_id, 0.20);
    assert_eq!(session.graph.belief_of(h_id), Some((0.30_f32 - 0.20_f32).max(0.0)));
}

#[test]
fn decay_hypothesis_belief_leaves_dead_hypotheses_alone() {
    // Pre-decay 0.10 is already below the floor. F4: no clamp exists
    // to raise it back; a dead hypothesis stays dead.
    let mut session = EpistemicSession::new("test");
    let h_id =
        session.graph.add_node(crate::cognition_graph::NodeKind::Hypothesis, "h", 0.10, 100);
    CorticalLoop::decay_hypothesis_belief(&mut session, h_id, 0.20);
    assert_eq!(session.graph.belief_of(h_id), Some(0.0));
}

#[test]
fn extract_named_target_returns_none_when_absent() {
    let mission = "OBJECTIVE: inspect the code and report.";
    assert_eq!(crate::cortical::util::extract_named_target(mission, "READ_OK:"), None);
}

#[test]
fn extract_named_target_strips_backticks_and_quotes() {
    let mission = "READ_OK:`crates/runtime_core/src/cortical_loop.rs`\n";
    assert_eq!(
        crate::cortical::util::extract_named_target(mission, "READ_OK:").as_deref(),
        Some("crates/runtime_core/src/cortical_loop.rs")
    );
    let mission2 = "READ_OK:\"crates/runtime_core/src/router.rs\"\n";
    assert_eq!(
        crate::cortical::util::extract_named_target(mission2, "READ_OK:").as_deref(),
        Some("crates/runtime_core/src/router.rs")
    );
}
