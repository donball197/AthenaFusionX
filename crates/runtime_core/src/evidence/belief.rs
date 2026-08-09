use serde::{Deserialize, Serialize};
use std::ops::{Add, AddAssign, Mul};

/// Multidimensional belief state used by the Executive Cortex.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct BeliefVector {
    pub implementation_confidence: f32,
    pub evidence_quality: f32,
    pub architectural_risk: f32,
    pub security_confidence: f32,
    /// Absolute change from the previous update (used by residual scheduler).
    pub residual: f32,
    /// True once a concrete success marker has been observed.
    pub has_success_marker: bool,
    /// Bit-packed counter of verified artifacts (cache-friendly on ARM64).
    pub verified_artifacts: u8,
}

impl BeliefVector {
    pub const ZERO: Self = Self {
        implementation_confidence: 0.0,
        evidence_quality: 0.0,
        architectural_risk: 0.0,
        security_confidence: 0.0,
        residual: 0.0,
        has_success_marker: false,
        verified_artifacts: 0,
    };

    pub fn new(
        implementation_confidence: f32,
        evidence_quality: f32,
        architectural_risk: f32,
        security_confidence: f32,
    ) -> Self {
        Self {
            implementation_confidence: implementation_confidence.clamp(0.0, 1.0),
            evidence_quality: evidence_quality.clamp(0.0, 1.0),
            architectural_risk: architectural_risk.clamp(0.0, 1.0),
            security_confidence: security_confidence.clamp(0.0, 1.0),
            residual: 0.0,
            has_success_marker: false,
            verified_artifacts: 0,
        }
    }

    /// Hard penalty method when a Policy Denial / Security Violation occurs.
    pub fn penalize_security(mut self) -> Self {
        self.security_confidence = 0.0;
        self.implementation_confidence = 0.0;
        self.architectural_risk = 1.0;
        self.residual = 1.0;
        self
    }

    /// Scalar utility used by ranking logic.
    pub fn scalar_score(&self) -> f32 {
        let positive = (self.implementation_confidence * 0.40)
            + (self.evidence_quality * 0.35)
            + (self.security_confidence * 0.25);
        let penalty = self.architectural_risk * 0.45;
        (positive - penalty).clamp(0.0, 1.0)
    }

    pub fn update(&self, other: &Self, weight: f32) -> Self {
        let w = weight.clamp(0.0, 1.0);
        let mut next = Self::new(
            self.implementation_confidence * (1.0 - w) + other.implementation_confidence * w,
            self.evidence_quality * (1.0 - w) + other.evidence_quality * w,
            self.architectural_risk * (1.0 - w) + other.architectural_risk * w,
            self.security_confidence * (1.0 - w) + other.security_confidence * w,
        );
        next.residual = (self.scalar_score() - next.scalar_score()).abs();
        next.has_success_marker = self.has_success_marker || other.has_success_marker;
        next.verified_artifacts = self.verified_artifacts | other.verified_artifacts;
        next
    }

    /// Calibrated Bayesian update on the positive dimensions with damping.
    pub fn bayesian_update(&self, likelihood: f32, damping: f32) -> Self {
        let likelihood = likelihood.clamp(0.0, 1.0);
        let damping = damping.clamp(0.0, 1.0);

        fn bayes(prior: f32, like: f32) -> f32 {
            let num = prior * like;
            let den = num + (1.0 - prior) * (1.0 - like);
            if den == 0.0 {
                prior
            } else {
                num / den
            }
        }

        let impl_post = bayes(self.implementation_confidence, likelihood);
        let evid_post = bayes(self.evidence_quality, likelihood);
        let sec_post = bayes(self.security_confidence, likelihood);

        let impl_d = damping * impl_post + (1.0 - damping) * self.implementation_confidence;
        let evid_d = damping * evid_post + (1.0 - damping) * self.evidence_quality;
        let sec_d = damping * sec_post + (1.0 - damping) * self.security_confidence;

        let mut next = Self::new(impl_d, evid_d, self.architectural_risk, sec_d);
        next.residual = (self.scalar_score() - next.scalar_score()).abs();
        next.has_success_marker = self.has_success_marker;
        next.verified_artifacts = self.verified_artifacts;
        next
    }

    /// Hard evidence gate. Belief alone is never enough.
    /// required_artifacts is a bitmask -- ALL required bits must actually be
    /// verified (bitwise AND), not merely a numerically-larger bit pattern.
    pub fn may_prove(&self, required_artifacts: u8, threshold: f32) -> bool {
        self.scalar_score() >= threshold
            && self.has_success_marker
            && (self.verified_artifacts & required_artifacts) == required_artifacts
    }

    /// Mark that a concrete success artifact was observed.
    pub fn mark_success(&mut self, artifact_bit: u8) {
        self.has_success_marker = true;
        self.verified_artifacts |= artifact_bit;
    }

    /// Convenience for residual-driven schedulers.
    pub fn scheduling_priority(&self, impact: f32, mission_priority: f32) -> f32 {
        self.residual * impact * mission_priority
    }
}

impl Default for BeliefVector {
    fn default() -> Self {
        Self::new(0.45, 0.30, 0.40, 0.55)
    }
}

impl Add for BeliefVector {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Self::new(
            self.implementation_confidence + rhs.implementation_confidence,
            self.evidence_quality + rhs.evidence_quality,
            self.architectural_risk + rhs.architectural_risk,
            self.security_confidence + rhs.security_confidence,
        )
    }
}

impl AddAssign for BeliefVector {
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs;
    }
}

impl Mul<f32> for BeliefVector {
    type Output = Self;
    fn mul(self, rhs: f32) -> Self {
        Self::new(
            self.implementation_confidence * rhs,
            self.evidence_quality * rhs,
            self.architectural_risk * rhs,
            self.security_confidence * rhs,
        )
    }
}

/// Artifact bitflags for verified WASM-related evidence.
pub const ARTIFACT_WASM_COMPILED: u8 = 0b0000_0001;
pub const ARTIFACT_WASM_OPTIMIZED: u8 = 0b0000_0010;
pub const ARTIFACT_WASM_FUEL_OK: u8 = 0b0000_0100;
