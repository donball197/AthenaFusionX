use std::collections::HashSet;
use crate::repository_intelligence::symbol_index::SymbolId;

#[derive(Debug, Clone)]
pub struct ImpactReport {
    pub direct_dependencies: HashSet<SymbolId>,
    pub transitive_closure: Vec<SymbolId>,
    pub risk_score: f64,
}

impl ImpactReport {
    pub fn new() -> Self {
        Self {
            direct_dependencies: HashSet::new(),
            transitive_closure: Vec::new(),
            risk_score: 0.0,
        }
    }
}

impl Default for ImpactReport {
    fn default() -> Self {
        Self::new()
    }
}
