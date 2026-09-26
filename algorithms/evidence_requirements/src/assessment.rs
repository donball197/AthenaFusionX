#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Assessment {
    SC,
    SP,
    RC,
    RP,
    NP,
    NN,
}

impl Assessment {
    pub const STATES: [Self; 6] = [Self::SC, Self::SP, Self::RC, Self::RP, Self::NP, Self::NN];

    pub fn dual(self) -> Self {
        match self {
            Self::SC => Self::RC,
            Self::RC => Self::SC,
            Self::SP => Self::RP,
            Self::RP => Self::SP,
            Self::NP => Self::NP,
            Self::NN => Self::NN,
        }
    }

    // ALL is the authoritative six-state composition algebra.
    //
    // Ordering among non-rejection states:
    //     SC > SP > NP > NN
    //
    // RC and RP are absorbing for ALL, with RC stronger than RP.
    //
    // ANY is intentionally derived from ALL through duality below.
    pub fn all(self, other: Self) -> Self {
        use Assessment::*;

        if self == RC || other == RC {
            return RC;
        }

        if self == RP || other == RP {
            return RP;
        }

        match (self, other) {
            (SC, SC) => SC,
            (SC, SP) | (SP, SC) => SP,
            (SC, NP) | (NP, SC) => NP,
            (SC, NN) | (NN, SC) => NN,

            (SP, SP) => SP,
            (SP, NP) | (NP, SP) => NP,
            (SP, NN) | (NN, SP) => NN,

            (NP, NP) => NP,
            (NP, NN) | (NN, NP) => NN,

            (NN, NN) => NN,

            _ => unreachable!("RC/RP handled above"),
        }
    }

    // DERIVED FROM ALL VIA DUALITY.
    // Do not hand-edit independently.
    pub fn any(self, other: Self) -> Self {
        self.dual().all(other.dual()).dual()
    }
}
