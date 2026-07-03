use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CriticVerdict {
    pub approved: bool,
    pub confidence: f32,
    pub reason: String,
    pub suggested_recovery: Option<String>,
}

pub struct Critic;

impl Critic {
    pub async fn evaluate(_objective: &str, observations: &str, _history: &str) -> Result<CriticVerdict, Box<dyn std::error::Error>> {
        let has_halt = observations.contains("[HALT]");
        let has_write = observations.contains("[WRITE_OK]") || observations.contains("success");
        let has_read = observations.contains("[READ_OK]");

        let approved = has_halt && (has_write || has_read);
        let confidence = if approved { 0.95 } else { 0.4 };

        Ok(CriticVerdict {
            approved,
            confidence,
            reason: if approved { "All success markers present".into() } else { "Missing concrete success indicators".into() },
            suggested_recovery: if !approved { Some("Replan with more specific actions".into()) } else { None },
        })
    }
}
