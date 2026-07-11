use crate::cortical_loop::{CortexOutcome, CorticalLoop};
use crate::runtime_metrics::RuntimeMetrics;
use crate::task_manager::TaskContext;
use crate::world_model::WorldModel;
use anyhow::Result;
use std::sync::Arc;

pub async fn send_task_v4(ctx: TaskContext, directive: &str) -> Result<String> {
    // Wire runtime_metrics
    let metrics = RuntimeMetrics::new();
    metrics.inc_tasks();

    // Wire hypothesis logic
    let _ = crate::hypothesis::HypothesisGenerator::generate(directive).await;

    // Wire meta_reasoner
    let _ = crate::cognition::meta_reasoner::MetaReasoner::execute_with_got(&ctx.client, &ctx.api_key, directive).await;

    // Wire unified CorticalLoop
    let world_model = Arc::new(WorldModel::new());
    let mut cortex = CorticalLoop::new(
        ctx.api_key.clone(),
        world_model,
        ctx.memory.clone(),
        ctx.vmo_state.clone(),
        ctx.dist.clone(),
        ctx.tx.clone(),
    );
    
    match cortex.ignite(directive).await? {
        CortexOutcome::Proven { belief } => {
            Ok(format!("CorticalLoop Execution Complete (belief {:.2})", belief))
        }
        CortexOutcome::Halted { reasoning } => {
            Ok(format!("CorticalLoop Halted intentionally. Reasoning: {}", reasoning))
        }
        CortexOutcome::TurnsExhausted { best_belief } => Err(anyhow::anyhow!(
            "CorticalLoop exhausted turns without proving the strategy (best belief {:.2})",
            best_belief
        )),
    }
}
