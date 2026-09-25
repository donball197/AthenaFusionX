use nexus_runtime_core::compile_remediation::{
    format_compile_failure_evidence, CompileDecision, CompileRemediationState, COMPILE_RETRY_BUDGET,
};

#[test]
fn test_compile_remediation_lifecycle() {
    let mut state = CompileRemediationState::default();
    let packages = vec!["test_pkg".to_string()];
    let diagnostic = "error[E0308]: mismatched types";

    assert_eq!(COMPILE_RETRY_BUDGET, 3);

    // Iteration 1: failed compile does NOT revert, retains for retry
    let d1 = state.classify(false);
    assert_eq!(d1, CompileDecision::RetainForRetry { iteration: 1 });
    assert!(state.is_active());
    let ev1 = format_compile_failure_evidence(&d1, &packages, diagnostic).unwrap();
    assert!(ev1.contains("[COMPILE_FAIL_ITERATION 1]"));
    assert!(ev1.contains(diagnostic));

    // Iteration 2: counter increments, diagnostic retained
    let d2 = state.classify(false);
    assert_eq!(d2, CompileDecision::RetainForRetry { iteration: 2 });
    let ev2 = format_compile_failure_evidence(&d2, &packages, diagnostic).unwrap();
    assert!(ev2.contains("[COMPILE_FAIL_ITERATION 2]"));

    // Iteration 3: counter increments, still retained
    let d3 = state.classify(false);
    assert_eq!(d3, CompileDecision::RetainForRetry { iteration: 3 });
    let ev3 = format_compile_failure_evidence(&d3, &packages, diagnostic).unwrap();
    assert!(ev3.contains("[COMPILE_FAIL_ITERATION 3]"));

    // Iteration 4: 4th failure triggers BudgetExhausted (revert)
    let d4 = state.classify(false);
    assert_eq!(d4, CompileDecision::BudgetExhausted { total_iterations: 4 });
    let ev4 = format_compile_failure_evidence(&d4, &packages, diagnostic).unwrap();
    assert!(ev4.contains("[COMPILE_FAIL_BUDGET_EXHAUSTED]"));
    assert!(!ev4.contains("[COMPILE_FAIL_ITERATION"));

    // Confirmed reset after revert
    state.reset();
    assert!(!state.is_active());
    assert_eq!(state.iterations(), 0);

    // Iteration 5: New failure starts cleanly at iteration 1
    let d5 = state.classify(false);
    assert_eq!(d5, CompileDecision::RetainForRetry { iteration: 1 });

    // Success clears active remediation state
    let d6 = state.classify(true);
    assert_eq!(d6, CompileDecision::Success);
    assert!(!state.is_active());
}

#[test]
fn test_diagnostic_preview_bounds() {
    let long_error = "x".repeat(5000);
    let bounded: String = long_error.chars().take(800).collect();
    let packages = vec!["pkg".to_string()];

    let ev = format_compile_failure_evidence(
        &CompileDecision::RetainForRetry { iteration: 1 },
        &packages,
        &bounded,
    )
    .unwrap();

    // Verify bounded evidence stays under the 1,500-char compress_context tail chunk
    assert!(ev.len() < 1500, "Evidence length ({}) must stay under 1,500 chars", ev.len());
    assert!(ev.contains("[COMPILE_FAIL_ITERATION 1]"));
}
