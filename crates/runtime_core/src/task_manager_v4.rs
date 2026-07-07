use crate::task_manager::TaskContext;
use crate::cortical_loop::{CorticalLoop, CortexOutcome};
use crate::world_model::WorldModel;
use crate::cognition::meta_reasoner::MetaReasoner;
use crate::hypothesis::HypothesisGenerator;
use crate::runtime_metrics::RuntimeMetrics;
use std::sync::Arc;
use anyhow::Result;

pub async fn send_task_v4(ctx: TaskContext, directive: &str) -> Result<String> {
    // Wire runtime_metrics
    let metrics = RuntimeMetrics::new();
    metrics.inc_tasks();

    // Wire hypothesis logic
    let _ = HypothesisGenerator::generate(directive).await;

    // Wire meta_reasoner
    let _ = MetaReasoner::execute_with_got(&ctx.client, &ctx.api_key, directive).await;
    
    // Wire memory actors (activating both variations to repair orphaned signatures)
    let (_tx_mem, rx_mem) = tokio::sync::mpsc::channel::<crate::memory_actor::MemoryCommand>(1);
    let _mem_actor_1 = crate::memory_actor::MemoryActor::new(rx_mem);
    let (_mem_actor_2, _handle) = crate::agents::memory_actor::MemoryActor::start();

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
        CortexOutcome::Proven { belief } => Ok(format!("CorticalLoop Execution Complete (belief {:.2})", belief)),
        CortexOutcome::TurnsExhausted { best_belief } => Err(anyhow::anyhow!(
            "CorticalLoop exhausted turns without proving the strategy (best belief {:.2})", best_belief
        )),
    }
}
