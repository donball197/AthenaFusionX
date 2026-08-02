use crate::belief_validation::BeliefValidationEvent;

pub struct BeliefValidationEngine;

impl BeliefValidationEngine {
    pub fn validate(prediction: String, observed: String) -> BeliefValidationEvent {
        BeliefValidationEvent::new(prediction, observed)
    }
}
