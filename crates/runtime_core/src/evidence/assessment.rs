//! Six-state Assessment lattice (Boolean / De Morgan dual).
//!
//! States: SC, SP, RC, RP, NP, NN
//! Dual:   SC↔RC, SP↔RP, NP↔NP, NN↔NN
//! ALL  = meet  (RC-absorbing)
//! ANY  = join  (SC-absorbing)  via  dual(all(dual(a), dual(b)))

use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Assessment {
    SC, // Strong Confirm
    SP, // Soft Positive
    RC, // Strong Reject / Contradiction
    RP, // Soft Reject
    NP, // Neutral Positive / Partial
    NN, // Neutral / None
}

impl Assessment {
    pub const ALL_STATES: [Assessment; 6] = [
        Assessment::SC,
        Assessment::SP,
        Assessment::RC,
        Assessment::RP,
        Assessment::NP,
        Assessment::NN,
    ];

    /// Order-reversing involution.
    pub fn dual(self) -> Self {
        match self {
            Assessment::SC => Assessment::RC,
            Assessment::SP => Assessment::RP,
            Assessment::RC => Assessment::SC,
            Assessment::RP => Assessment::SP,
            Assessment::NP => Assessment::NP,
            Assessment::NN => Assessment::NN,
        }
    }

    /// Meet (ALL). Commutative, idempotent, RC-absorbing.
    pub fn all(self, other: Self) -> Self {
        use Assessment::*;
        match (self, other) {
            (SC, x) | (x, SC) => x,
            (SP, SP) => SP,
            (SP, NP) | (NP, SP) => NP,
            (SP, NN) | (NN, SP) => NN,
            (SP, RP) | (RP, SP) => RP,
            (SP, RC) | (RC, SP) => RC,
            (NP, NP) => NP,
            (NP, NN) | (NN, NP) => NN,
            (NP, RP) | (RP, NP) => RP,
            (NP, RC) | (RC, NP) => RC,
            (NN, NN) => NN,
            (NN, RP) | (RP, NN) => RP,
            (NN, RC) | (RC, NN) => RC,
            (RP, RP) => RP,
            (RP, RC) | (RC, RP) => RC,
            (RC, RC) => RC,
        }
    }

    /// Join (ANY) via De Morgan dual.
    pub fn any(self, other: Self) -> Self {
        self.dual().all(other.dual()).dual()
    }

    pub fn is_all_zero(self) -> bool {
        matches!(self, Assessment::RC)
    }

    pub fn is_any_unit(self) -> bool {
        matches!(self, Assessment::SC)
    }
}

impl fmt::Display for Assessment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Assessment::SC => write!(f, "SC"),
            Assessment::SP => write!(f, "SP"),
            Assessment::RC => write!(f, "RC"),
            Assessment::RP => write!(f, "RP"),
            Assessment::NP => write!(f, "NP"),
            Assessment::NN => write!(f, "NN"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dual_is_involution() {
        for a in Assessment::ALL_STATES {
            assert_eq!(a.dual().dual(), a);
        }
    }

    #[test]
    fn all_is_commutative_and_idempotent() {
        for a in Assessment::ALL_STATES {
            assert_eq!(a.all(a), a);
            for b in Assessment::ALL_STATES {
                assert_eq!(a.all(b), b.all(a));
            }
        }
    }

    #[test]
    fn rc_is_absorbing_for_all() {
        for a in Assessment::ALL_STATES {
            assert_eq!(Assessment::RC.all(a), Assessment::RC);
            assert_eq!(a.all(Assessment::RC), Assessment::RC);
        }
    }

    #[test]
    fn any_is_commutative_and_idempotent() {
        for a in Assessment::ALL_STATES {
            assert_eq!(a.any(a), a);
            for b in Assessment::ALL_STATES {
                assert_eq!(a.any(b), b.any(a));
            }
        }
    }

    #[test]
    fn sc_is_absorbing_for_any() {
        for a in Assessment::ALL_STATES {
            assert_eq!(Assessment::SC.any(a), Assessment::SC);
            assert_eq!(a.any(Assessment::SC), Assessment::SC);
        }
    }

    #[test]
    fn duality_holds_for_all_36_pairs() {
        for a in Assessment::ALL_STATES {
            for b in Assessment::ALL_STATES {
                assert_eq!(a.any(b), a.dual().all(b.dual()).dual());
            }
        }
    }

    #[test]
    fn ordering_sc_gt_sp_gt_np_gt_nn() {
        assert_eq!(Assessment::SC.all(Assessment::SP), Assessment::SP);
        assert_eq!(Assessment::SP.all(Assessment::NP), Assessment::NP);
        assert_eq!(Assessment::NP.all(Assessment::NN), Assessment::NN);
    }
}
