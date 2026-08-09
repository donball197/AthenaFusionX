use super::traits::{MissionOutcome, ReflectionReport, Reflector};
use async_trait::async_trait;

/// V5 ReflectionEngine: Evaluates CortexOutcome exhaustion and negative traits
/// into formatted Lesson payloads meant strictly for prompt injection.
#[derive(Clone, Default)]
pub struct ReflectionEngine {}

impl ReflectionEngine {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl Reflector for ReflectionEngine {
    async fn reflect(&self, outcome: &MissionOutcome) -> anyhow::Result<ReflectionReport> {
        let mut lessons = Vec::new();
        
        if !outcome.success && !outcome.failures.is_empty() {
            for failure in &outcome.failures {
                // Formatting matches what [ANCESTOR NODE EVIDENCE] expects
                let lesson = format!(
                    "LESSON: Action triggered failure.\nPATTERN: {}\nAVOID: {}\nSUGGESTED: Assess substrate capability.",
                    failure, outcome.evidence_summary
                );
                lessons.push(lesson);
            }
        } else if outcome.success {
            lessons.push(format!("SUCCESS PATTERN: {}", outcome.evidence_summary));
        }

        Ok(ReflectionReport {
            mission_id: outcome.mission_id,
            lessons,
            confidence: if outcome.success { 0.9 } else { 0.4 },
            recommended_strategy: outcome.successful_strategies.first().cloned(),
            memory_updates: vec![],
            belief_updates: vec![],
            metrics: std::collections::HashMap::new(),
        })
    }
}
