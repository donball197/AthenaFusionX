//! Unit tests for the Bayesian Calibration engine.
//! Proves that evidence markers mathematically drive belief as expected.

use nexus_runtime_core::bayesian_calibration::{BayesianCalibrator, EvidenceClass};

#[test]
fn test_mutation_success_causes_massive_jump() {
    let cal = BayesianCalibrator::default();
    // Start at neutral 0.50 belief
    let prior = 0.50;
    
    // Simulate a successful patch with a slightly positive LLM modifier
    let report = cal.calibrate(prior, "[DIFF_PATCH_OK] Applied changes to foo.rs", Some(0.2));
    
    // The LLR for MutationSuccess is ~2.36. 
    // logit(0.50) = 0. 0 + 2.36 + 0.2 = 2.56. sigmoid(2.56) ≈ 0.92.
    // We assert that the belief jumped past the 0.90 completion threshold!
    assert_eq!(report.evidence_class, EvidenceClass::MutationSuccess);
    assert!(report.posterior >= 0.90, "Posterior {} should be >= 0.90", report.posterior);
}

#[test]
fn test_compile_fail_tanks_belief() {
    let cal = BayesianCalibrator::default();
    // Start with high confidence
    let prior = 0.95;
    
    // Simulate a compiler failure with an LLM saying "this is bad"
    let report = cal.calibrate(prior, "[COMPILE_FAIL] syntax error", Some(-0.5));
    
    // The LLR for CompileFail is negative. Belief should drop significantly.
    assert_eq!(report.evidence_class, EvidenceClass::CompileFail);
    assert!(report.posterior < 0.50, "Posterior {} should be < 0.50", report.posterior);
}

#[test]
fn test_recon_cannot_accidentally_complete() {
    let cal = BayesianCalibrator::default();
    let mut belief = 0.50;
    
    // Simulate reading a file 5 times with an overly generous LLM (+0.5 modifier)
    for _ in 0..5 {
        let report = cal.calibrate(belief, "[READ_OK] read file", Some(0.5));
        belief = report.posterior;
    }
    
    // Even with 5 reads and max LLM modifiers, the weak ReconSuccess LLR 
    // and 0.05 shrinkage should keep it well below the 0.90 threshold.
    assert!(belief < 0.80, "Belief {} should be < 0.80", belief);
}

#[test]
fn test_llm_modifier_is_capped() {
    let cal = BayesianCalibrator::default();
    let prior = 0.50;
    
    // Simulate a hallucinating LLM giving a massive +5.0 modifier on a recon task
    let report = cal.calibrate(prior, "[SEARCH_CODE_OK] found it", Some(5.0));
    
    // The modifier should be clamped to 1.5 max_abs_llr. 
    // It should NOT shoot the belief to 0.99.
    assert!(report.posterior < 0.85, "Posterior {} should be capped", report.posterior);
    // Check the internal math
    assert!(report.llr_modifier <= 1.5, "LLM modifier {} should be clamped to 1.5", report.llr_modifier);
}

#[test]
fn test_successive_patches_converge_to_high_belief() {
    let cal = BayesianCalibrator::default();
    let mut belief = 0.50;
    
    // Simulate patching, compiling, and testing successfully
    let patch_report = cal.calibrate(belief, "[DIFF_PATCH_OK] patched", Some(0.1));
    belief = patch_report.posterior;
    
    let compile_report = cal.calibrate(belief, "[COMPILE_OK] compiled", Some(0.1));
    belief = compile_report.posterior;
    
    let test_report = cal.calibrate(belief, "[TEST_OK] tests passed", Some(0.1));
    belief = test_report.posterior;
    
    // After a full successful cycle, belief should be near max
    assert!(belief > 0.95, "Belief {} should be > 0.95 after full cycle", belief);
}
