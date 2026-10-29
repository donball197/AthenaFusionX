use crate::assessment::Assessment;
use crate::evidence::Evidence;
use crate::predicate::Predicate;

pub fn evaluate(predicate: &dyn Predicate, evidence: Option<&Evidence>) -> Assessment {
    match evidence {
        None => Assessment::NN,
        Some(e) => predicate.assess(Some(e)),
    }
}
