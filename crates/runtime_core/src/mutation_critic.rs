#[derive(Debug, Clone, PartialEq)]
pub struct MutationEvaluation {
    pub accepted: bool,
    pub structural_similarity: f32,
    pub confidence_delta: f32,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Evaluation {
    Approved,
    Veto(String),
}

pub fn evaluate(proposed_code: &str) -> Evaluation {
    if proposed_code.contains("pub struct ") && proposed_code.contains(";") && !proposed_code.contains("{") {
        return Evaluation::Veto("Empty unit struct detected. Fields required.".to_string());
    }
    Evaluation::Approved
}

pub fn evaluate_evidence(proposed_code: &str) -> Evaluation {
    if proposed_code.contains("pub struct ") && proposed_code.contains(";") && !proposed_code.contains("{") {
        return Evaluation::Veto("Empty unit struct detected. Fields required.".to_string());
    }
    Evaluation::Approved
}
