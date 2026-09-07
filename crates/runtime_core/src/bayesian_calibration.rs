//! Bayesian calibration of hypothesis belief from tool evidence and LLM modifiers.
//!
//! logit(posterior) = logit(prior) + log(P(E|H)/P(E|¬H)) + clamped LLM LLR

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[inline]
fn logit(p: f32) -> f32 {
    let p = p.clamp(1e-6, 1.0 - 1e-6);
    (p / (1.0 - p)).ln()
}

#[inline]
fn sigmoid(x: f32) -> f32 {
    1.0 / (1.0 + (-x).exp())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EvidenceClass {
    MutationSuccess,
    MutationFailure,
    ReconSuccess,
    Halt,
    TestSuccess,
    TestFailure,
    CompileOk,
    CompileFail,
    ActionBlocked,
    Timeout,
    Unknown,
}

impl EvidenceClass {
    pub fn classify(blob: &str) -> Self {
        let t = blob.trim_start();
        if t.contains("[MUTATION_REVERTED]") || t.contains("[COMPILE_FAIL]") {
            if t.contains("[COMPILE_FAIL]") {
                return EvidenceClass::CompileFail;
            }
            return EvidenceClass::MutationFailure;
        }
        if t.starts_with("[COMPILE_OK]") || t.contains("\n[COMPILE_OK]") {
            return EvidenceClass::CompileOk;
        }
        if t.starts_with("[WRITE_OK]")
            || t.starts_with("[DIFF_PATCH_OK]")
            || t.starts_with("[PATCH_FUNCTION_OK]")
            || t.starts_with("[REPLACE_LINES_OK]")
        {
            return EvidenceClass::MutationSuccess;
        }
        if t.starts_with("[TEST_OK]") {
            return EvidenceClass::TestSuccess;
        }
        if t.starts_with("[TEST_FAILED]") {
            return EvidenceClass::TestFailure;
        }
        if t.starts_with("[HALT]") {
            return EvidenceClass::Halt;
        }
        if t.starts_with("[TIMEOUT]") {
            return EvidenceClass::Timeout;
        }
        if t.starts_with("[ACTION_GATE_BLOCKED]") || t.starts_with("[ACTION FAILED]") {
            return EvidenceClass::ActionBlocked;
        }
        // ⚡ OS FAILURE INTERCEPT: Catch non-zero exit codes and missing paths BEFORE ReconSuccess
        if t.contains("exit status:") && !t.contains("exit status: 0") {
            return EvidenceClass::ActionBlocked;
        }
        if t.contains("No such file or directory") || t.contains("PATH_NOT_FOUND") {
            return EvidenceClass::ActionBlocked;
        }

        if t.starts_with("[READ_OK]")
            || t.starts_with("[READ_CHUNK_OK]")
            || t.starts_with("[SEARCH_CODE_OK]")
            || t.starts_with("[CMD_OK]")
            || t.starts_with("[SEARCH_OK]")
            || t.starts_with("[SEARCH_MEMORY_OK]")
            || t.starts_with("[CACHE_HIT]")
        {
            return EvidenceClass::ReconSuccess;
        }
        EvidenceClass::Unknown
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LikelihoodTable {
    pub p_e_given_h: HashMap<String, f32>,
    pub p_e_given_not_h: HashMap<String, f32>,
}

impl Default for LikelihoodTable {
    fn default() -> Self {
        use EvidenceClass::*;
        let mut p_h = HashMap::new();
        let mut p_nh = HashMap::new();
        let rows: &[(EvidenceClass, f32, f32)] = &[
            (MutationSuccess, 0.85, 0.08),
            (MutationFailure, 0.12, 0.55),
            (CompileOk, 0.80, 0.15),
            (CompileFail, 0.05, 0.70),
            (TestSuccess, 0.82, 0.12),
            (TestFailure, 0.15, 0.55),
            (ReconSuccess, 0.55, 0.45),
            (Halt, 0.70, 0.25),
            (ActionBlocked, 0.20, 0.40),
            (Timeout, 0.48, 0.52),
            (Unknown, 0.50, 0.50),
        ];
        for (c, a, b) in rows {
            p_h.insert(format!("{c:?}"), *a);
            p_nh.insert(format!("{c:?}"), *b);
        }
        Self { p_e_given_h: p_h, p_e_given_not_h: p_nh }
    }
}

impl LikelihoodTable {
    fn lr(&self, class: EvidenceClass) -> f32 {
        let key = format!("{class:?}");
        let p_h = self.p_e_given_h.get(&key).copied().unwrap_or(0.5).clamp(1e-4, 1.0 - 1e-4);
        let p_nh = self.p_e_given_not_h.get(&key).copied().unwrap_or(0.5).clamp(1e-4, 1.0 - 1e-4);
        p_h / p_nh
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModifierConfig {
    pub scale: f32,
    pub max_abs_llr: f32,
}

impl Default for ModifierConfig {
    fn default() -> Self {
        Self { scale: 2.0, max_abs_llr: 1.5 }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalibrationReport {
    pub prior: f32,
    pub posterior: f32,
    pub evidence_class: EvidenceClass,
    pub llr_marker: f32,
    pub llr_modifier: f32,
    pub llr_total: f32,
}

impl CalibrationReport {
    pub fn edge_strength(&self) -> f32 {
        let a = self.llr_total.abs();
        (1.0 - (-a).exp()).clamp(0.05, 0.95)
    }

    pub fn supports(&self) -> bool {
        self.llr_total >= 0.0
    }
}

#[derive(Debug, Clone)]
pub struct BayesianCalibrator {
    pub table: LikelihoodTable,
    pub modifier: ModifierConfig,
    pub shrinkage: f32,
}

impl Default for BayesianCalibrator {
    fn default() -> Self {
        Self {
            table: LikelihoodTable::default(),
            modifier: ModifierConfig::default(),
            shrinkage: 0.05,
        }
    }
}

impl BayesianCalibrator {
    pub fn calibrate(
        &self,
        prior_belief: f32,
        evidence_blob: &str,
        llm_modifier: Option<f32>,
    ) -> CalibrationReport {
        let prior = prior_belief.clamp(1e-6, 1.0 - 1e-6);
        let class = EvidenceClass::classify(evidence_blob);
        let llr_marker = self.table.lr(class).ln();
        let llr_modifier = match llm_modifier {
            Some(m) => {
                let m = m.clamp(-0.5, 0.5);
                (m * self.modifier.scale)
                    .clamp(-self.modifier.max_abs_llr, self.modifier.max_abs_llr)
            }
            None => 0.0,
        };
        let effective_marker =
            if class == EvidenceClass::ReconSuccess { llr_marker * 0.20 } else { llr_marker };
        let effective_modifier =
            if class == EvidenceClass::ReconSuccess { llr_modifier * 0.20 } else { llr_modifier };
        let llr_total = effective_marker + effective_modifier;
        let mut posterior = sigmoid(logit(prior) + llr_total);
        if self.shrinkage > 0.0 {
            posterior = posterior * (1.0 - self.shrinkage) + 0.5 * self.shrinkage;
        }
        CalibrationReport {
            prior,
            posterior: posterior.clamp(0.0, 1.0),
            evidence_class: class,
            llr_marker,
            llr_modifier,
            llr_total,
        }
    }

    pub fn update_belief(
        &self,
        prior_belief: f32,
        evidence_blob: &str,
        llm_modifier: Option<f32>,
    ) -> f32 {
        self.calibrate(prior_belief, evidence_blob, llm_modifier).posterior
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mutation_success_raises_belief() {
        let cal = BayesianCalibrator::default();
        let r = cal.calibrate(0.5, "[DIFF_PATCH_OK] Applied changes to foo.py", Some(0.25));
        assert!(r.posterior > 0.5, "posterior={}", r.posterior);
        assert_eq!(r.evidence_class, EvidenceClass::MutationSuccess);
        assert!(r.supports());
    }

    #[test]
    fn compile_fail_lowers_belief() {
        let cal = BayesianCalibrator::default();
        let r = cal.calibrate(0.8, "[COMPILE_FAIL] type error", Some(-0.2));
        assert!(r.posterior < 0.8, "posterior={}", r.posterior);
        assert!(!r.supports());
    }

    #[test]
    fn recon_is_weak() {
        let cal = BayesianCalibrator::default();
        let r = cal.calibrate(0.5, "[READ_OK] foo.py\nCONTENT...", Some(0.05));
        assert!(r.posterior > 0.48 && r.posterior < 0.70, "posterior={}", r.posterior);
    }

    #[test]
    fn successive_patches_converge() {
        let cal = BayesianCalibrator::default();
        let mut b = 0.5_f32;
        for _ in 0..3 {
            b = cal.update_belief(b, "[PATCH_FUNCTION_OK] Updated function 'f' in a.rs", Some(0.2));
        }
        assert!(b > 0.85, "belief after 3 patches={}", b);
    }
}
