use crate::genome::SystemGenome;
use anyhow::Result;

#[derive(Debug, Clone)]
pub struct FitnessScore {
    pub score: f32,
    pub execution_time_ms: u64,
    pub token_efficiency: f32,
}

pub struct FitnessEvaluator;

impl FitnessEvaluator {
    pub async fn evaluate(
        genome: &SystemGenome,
        benchmark_latency_ms: u64,
        tokens_used: u32,
    ) -> Result<FitnessScore> {
        let time_penalty = (benchmark_latency_ms as f32) * 0.01;
        let token_penalty = (tokens_used as f32) * 0.005;
        
        let base_score = 100.0 + (genome.weights.heuristic_reward * 50.0);
        let final_score = (base_score - time_penalty - token_penalty).max(0.0);

        Ok(FitnessScore {
            score: final_score,
            execution_time_ms: benchmark_latency_ms,
            token_efficiency: if tokens_used > 0 { 1000.0 / (tokens_used as f32) } else { 0.0 },
        })
    }
}
