use crate::assessment::Assessment;
use crate::evidence::Evidence;

pub trait Predicate {
    fn assess(&self, evidence: Option<&Evidence>) -> Assessment;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct SuccessfulExecution;

impl Predicate for SuccessfulExecution {
    fn assess(&self, evidence: Option<&Evidence>) -> Assessment {
        match evidence {
            Some(e) if e.success => Assessment::SC,
            Some(_) => Assessment::RC,
            None => Assessment::NN,
        }
    }
}
