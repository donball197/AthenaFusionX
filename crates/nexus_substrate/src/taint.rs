//! Two-dimensional taint lattice: integrity x secrecy.
//!
//! Integrity is DOWNWARD-CLOSED. Combining two values takes the
//! LOW-WATER MARK (min): an output is only as trustworthy as its
//! least-trustworthy input. This is a meet, not a least-upper-bound.
//!
//! Secrecy is UPWARD-CLOSED. Combining takes the HIGH-WATER MARK (max).
//!
//! Effect is a separate tag (Observed/Mutated/Internal) recording how
//! the value was produced. It is combined by precedence Mutated >
//! Observed > Internal.

use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[repr(i32)]
pub enum Integrity {
    Untrusted = 0,
    Observed = 1,
    Internal = 2,
    Verified = 3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[repr(i32)]
pub enum Secrecy {
    Public = 0,
    Internal = 1,
    Secret = 2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Effect {
    Observed,
    Mutated,
    Internal,
}

impl Default for Effect {
    fn default() -> Self {
        Effect::Internal
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TaintLabel {
    pub integrity: Integrity,
    pub secrecy: Secrecy,
    pub effect: Effect,
}

impl TaintLabel {
    pub const UNTRUSTED_PUBLIC: Self = Self {
        integrity: Integrity::Untrusted,
        secrecy: Secrecy::Public,
        effect: Effect::Observed,
    };
    pub const OBSERVED: Self = Self {
        integrity: Integrity::Observed,
        secrecy: Secrecy::Internal,
        effect: Effect::Observed,
    };
    pub const MUTATED: Self = Self {
        integrity: Integrity::Observed,
        secrecy: Secrecy::Internal,
        effect: Effect::Mutated,
    };
    pub const INTERNAL: Self = Self {
        integrity: Integrity::Internal,
        secrecy: Secrecy::Internal,
        effect: Effect::Internal,
    };
    pub const VERIFIED: Self = Self {
        integrity: Integrity::Verified,
        secrecy: Secrecy::Internal,
        effect: Effect::Internal,
    };

    /// Lattice combine.
    ///
    /// integrity: LOW-WATER MARK. Untrusted.join(Internal) == Untrusted.
    /// integrity is a meet, not a join, because any untrusted input
    /// contaminates the output.
    ///
    /// secrecy: HIGH-WATER MARK. Public.join(Secret) == Secret.
    ///
    /// effect: Mutated > Observed > Internal.
    pub fn join(self, other: Self) -> Self {
        Self {
            integrity: self.integrity.min(other.integrity),
            secrecy: self.secrecy.max(other.secrecy),
            effect: match (self.effect, other.effect) {
                (Effect::Mutated, _) | (_, Effect::Mutated) => Effect::Mutated,
                (Effect::Observed, _) | (_, Effect::Observed) => Effect::Observed,
                _ => Effect::Internal,
            },
        }
    }

    pub fn with_effect(mut self, effect: Effect) -> Self {
        self.effect = effect;
        self
    }
    pub fn with_integrity(mut self, i: Integrity) -> Self {
        self.integrity = i;
        self
    }
    pub fn with_secrecy(mut self, s: Secrecy) -> Self {
        self.secrecy = s;
        self
    }

    pub fn to_storage(self) -> String {
        format!("I:{},S:{},E:{}", self.integrity as i32, self.secrecy as i32, self.effect as i32)
    }

    pub fn from_storage(s: &str) -> Option<Self> {
        let mut i = None;
        let mut sec = None;
        let mut e = None;
        for part in s.split(',') {
            if let Some(v) = part.strip_prefix("I:") {
                i = v.parse::<i32>().ok().and_then(Integrity::from_i32);
            } else if let Some(v) = part.strip_prefix("S:") {
                sec = v.parse::<i32>().ok().and_then(Secrecy::from_i32);
            } else if let Some(v) = part.strip_prefix("E:") {
                e = v.parse::<i32>().ok().and_then(Effect::from_i32);
            }
        }
        Some(Self { integrity: i?, secrecy: sec?, effect: e? })
    }

    pub fn from_legacy(s: &str) -> Option<Self> {
        match s {
            "Untrusted" => Some(Self::UNTRUSTED_PUBLIC),
            "Observed" => Some(Self::OBSERVED),
            "Mutated" => Some(Self::MUTATED),
            "Internal" => Some(Self::INTERNAL),
            _ => None,
        }
    }

    pub fn flow_to(self, policy: FlowPolicy) -> Result<(), TaintError> {
        if self.integrity < policy.min_integrity {
            return Err(TaintError::Integrity {
                source: self.integrity,
                sink: policy.min_integrity,
            });
        }
        if self.secrecy > policy.max_secrecy {
            return Err(TaintError::Secrecy { source: self.secrecy, sink: policy.max_secrecy });
        }
        Ok(())
    }

    #[must_use]
    pub fn declassify(self, new_integrity: Integrity, new_secrecy: Secrecy) -> Self {
        Self { integrity: new_integrity, secrecy: new_secrecy, effect: self.effect }
    }
}

impl Integrity {
    pub fn from_i32(v: i32) -> Option<Self> {
        match v {
            0 => Some(Self::Untrusted),
            1 => Some(Self::Observed),
            2 => Some(Self::Internal),
            3 => Some(Self::Verified),
            _ => None,
        }
    }
}

impl Secrecy {
    pub fn from_i32(v: i32) -> Option<Self> {
        match v {
            0 => Some(Self::Public),
            1 => Some(Self::Internal),
            2 => Some(Self::Secret),
            _ => None,
        }
    }
}

impl Effect {
    pub fn from_i32(v: i32) -> Option<Self> {
        match v {
            0 => Some(Self::Observed),
            1 => Some(Self::Mutated),
            2 => Some(Self::Internal),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct FlowPolicy {
    pub min_integrity: Integrity,
    pub max_secrecy: Secrecy,
}

impl FlowPolicy {
    pub const ANY: Self =
        Self { min_integrity: Integrity::Untrusted, max_secrecy: Secrecy::Secret };
    pub const PUBLIC_SINK: Self =
        Self { min_integrity: Integrity::Untrusted, max_secrecy: Secrecy::Public };
    pub const INTERNAL_SINK: Self =
        Self { min_integrity: Integrity::Observed, max_secrecy: Secrecy::Internal };
    pub const VERIFIED_SINK: Self =
        Self { min_integrity: Integrity::Verified, max_secrecy: Secrecy::Internal };
}

#[derive(Debug, PartialEq, Eq)]
pub enum TaintError {
    Integrity { source: Integrity, sink: Integrity },
    Secrecy { source: Secrecy, sink: Secrecy },
}

impl fmt::Display for TaintError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TaintError::Integrity { source, sink } => {
                write!(f, "integrity violation: source {source:?} < sink minimum {sink:?}")
            }
            TaintError::Secrecy { source, sink } => {
                write!(f, "secrecy violation: source {source:?} > sink maximum {sink:?}")
            }
        }
    }
}

impl std::error::Error for TaintError {}

impl fmt::Display for TaintLabel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_storage())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// integrity is a LOW-WATER MARK, not a least-upper-bound. An
    /// earlier version used .max() and shipped a trust-laundering bug.
    /// This test exists specifically to prevent that regression.
    #[test]
    fn join_takes_the_low_water_mark_for_integrity() {
        let j = TaintLabel::UNTRUSTED_PUBLIC.join(TaintLabel::INTERNAL);
        assert_eq!(j.integrity, Integrity::Untrusted); // NOT Internal
        assert_eq!(j.secrecy, Secrecy::Internal);
        assert_eq!(j.effect, Effect::Observed);
    }

    #[test]
    fn join_is_commutative() {
        let a = TaintLabel::UNTRUSTED_PUBLIC;
        let b = TaintLabel::VERIFIED;
        assert_eq!(a.join(b), b.join(a));
    }

    #[test]
    fn join_is_idempotent() {
        let a = TaintLabel::MUTATED;
        assert_eq!(a.join(a), a);
    }

    #[test]
    fn secrecy_high_water_mark() {
        let a = TaintLabel::INTERNAL.with_secrecy(Secrecy::Public);
        let b = TaintLabel::INTERNAL.with_secrecy(Secrecy::Secret);
        assert_eq!(a.join(b).secrecy, Secrecy::Secret);
    }

    #[test]
    fn untrusted_cannot_flow_to_verified_sink() {
        assert!(TaintLabel::UNTRUSTED_PUBLIC.flow_to(FlowPolicy::VERIFIED_SINK).is_err());
    }

    #[test]
    fn storage_roundtrip() {
        let l = TaintLabel::MUTATED;
        assert_eq!(TaintLabel::from_storage(&l.to_storage()), Some(l));
    }

    #[test]
    fn legacy_roundtrip() {
        assert_eq!(TaintLabel::from_legacy("Untrusted"), Some(TaintLabel::UNTRUSTED_PUBLIC));
    }
}
