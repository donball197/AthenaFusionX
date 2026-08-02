use serde::{Deserialize, Serialize};
use std::ops::{Add, AddAssign, Mul};

/// Multidimensional belief state used by the Executive Cortex.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct BeliefVector {
    pub implementation_confidence: f32,
    pub evidence_quality: f32,
    pub architectural_risk: f32,
    pub security_confidence: f32,
}

impl BeliefVector {
    pub const ZERO: Self = Self {
        implementation_confidence: 0.0,
        evidence_quality: 0.0,
        architectural_risk: 0.0,
        security_confidence: 0.0,
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
        }
    }

    /// Hard penalty method when a Policy Denial / Security Violation occurs.
    pub fn penalize_security(mut self) -> Self {
        self.security_confidence = 0.0;
        self.implementation_confidence = 0.0;
        self.architectural_risk = 1.0;
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
        Self::new(
            self.implementation_confidence * (1.0 - w) + other.implementation_confidence * w,
            self.evidence_quality * (1.0 - w) + other.evidence_quality * w,
            self.architectural_risk * (1.0 - w) + other.architectural_risk * w,
            self.security_confidence * (1.0 - w) + other.security_confidence * w,
        )
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
