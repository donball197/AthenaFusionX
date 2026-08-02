use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BeliefValidationEvent {
    pub prediction: String,

    pub observed: String,

    pub error_score: f32,

    pub confidence_delta: f32,

    pub lesson: String,
}

impl BeliefValidationEvent {
    pub fn new(prediction: String, observed: String) -> Self {
        let error_score = if prediction == observed { 0.0 } else { 1.0 };

        let confidence_delta = if error_score == 0.0 { 0.05 } else { -0.25 };

        let lesson = if error_score == 0.0 {
            "Prediction confirmed".to_string()
        } else {
            format!("Prediction failed. Expected '{}', observed '{}'", prediction, observed)
        };

        Self { prediction, observed, error_score, confidence_delta, lesson }
    }
}
