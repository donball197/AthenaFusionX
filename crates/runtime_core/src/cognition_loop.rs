use std::sync::Arc;

use crate::memory_cortex::MemoryCortex;
use crate::outcome_verifier::OutcomeVerifier;
use crate::strategy_revision::StrategyRevision;

pub struct CognitiveLoop {
    pub memory: Arc<MemoryCortex>,
}

impl CognitiveLoop {
    pub fn new(memory: Arc<MemoryCortex>) -> Self {
        Self { memory }
    }

    pub async fn evaluate_cycle(&self, expected: &str, observed: &str) -> anyhow::Result<()> {
        let assessment = OutcomeVerifier::evaluate(expected, observed);

        let decision = StrategyRevision::decide(assessment.success_probability);

        self.memory
            .remember(crate::memory_types::CognitiveMemory {
                id: uuid::Uuid::new_v4(),

                kind: crate::memory_types::MemoryKind::Reflection,

                key: "cognitive_loop_result".into(),

                content: format!(
                    "Decision {:?}, confidence {}",
                    decision, assessment.success_probability
                ),

                source: "CognitiveLoop".into(),

                confidence: assessment.success_probability,

                importance: 0.8,

                created_at: chrono::Utc::now().timestamp(),

                tags: vec!["belief".into(), "feedback".into()],
            })
            .await?;

        Ok(())
    }
}
