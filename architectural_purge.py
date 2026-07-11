import os
from pathlib import Path

def remove_module(filepath, module_name, mod_file):
    p = Path(filepath)
    if p.exists():
        p.unlink()
        print(f"[EXCISED] Dead tissue removed: {filepath}")
    
    mf = Path(mod_file)
    if mf.exists():
        content = mf.read_text()
        if f"pub mod {module_name};" in content:
            # Remove the mod declaration and any trailing newlines
            clean_content = content.replace(f"pub mod {module_name};\n", "").replace(f"pub mod {module_name};", "")
            mf.write_text(clean_content)
            print(f"[UNWIRED] Unlinked {module_name} from {mod_file}")

# 1. Eradicate MemoryActor Schizophrenia
remove_module("crates/runtime_core/src/agents/memory_actor.rs", "memory_actor", "crates/runtime_core/src/agents/mod.rs")

# 2. Excise Orphaned VMO Handler (Node actively uses kernel/safe_vmo.rs)
remove_module("crates/nexus_substrate/src/vmo_handler.rs", "vmo_handler", "crates/nexus_substrate/src/lib.rs")

# 3. Exterminate Dead Critic Triplets
remove_module("crates/runtime_core/src/cognition/critic.rs", "critic", "crates/runtime_core/src/cognition/mod.rs")
remove_module("crates/runtime_core/src/orchestrator_v2/critic.rs", "critic", "crates/runtime_core/src/orchestrator_v2/mod.rs")

# 4. Resolve GoT Struct Collisions
meta_path = Path("crates/runtime_core/src/cognition/meta_reasoner.rs")
if meta_path.exists():
    meta_path.write_text("""use crate::hypothesis_engine::HypothesisEngine;
use anyhow::{anyhow, Result};
use reqwest::Client;

#[derive(Default)]
pub struct MetaReasoner;

impl MetaReasoner {
    pub fn new() -> Self {
        Self
    }

    pub async fn execute_with_got(
        client: &Client,
        api_key: &str,
        directive: &str,
    ) -> Result<String> {
        let resolution =
            HypothesisEngine::evaluate_fault(client, api_key, directive, "MetaReasoner GoT call")
                .await
                .map_err(|e| anyhow!("GoT failed: {}", e))?;

        Ok(format!(
            "[GoT] Root Cause: {}\\nSelected: {} (prob {:.2})",
            resolution.root_cause_analysis,
            resolution.selected_path.description,
            resolution.selected_path.probability_score
        ))
    }
}
""")
    print("[OK] GoT Struct collisions resolved in meta_reasoner.rs")

# 5. Lock Unified Memory Channel in Task Manager V4
tm4_path = Path("crates/runtime_core/src/task_manager_v4.rs")
if tm4_path.exists():
    tm4_path.write_text("""use crate::cognition::meta_reasoner::MetaReasoner;
use crate::cortical_loop::{CortexOutcome, CorticalLoop};
use crate::hypothesis::HypothesisGenerator;
use crate::runtime_metrics::RuntimeMetrics;
use crate::task_manager::TaskContext;
use crate::world_model::WorldModel;
use anyhow::Result;
use std::sync::Arc;

pub async fn send_task_v4(ctx: TaskContext, directive: &str) -> Result<String> {
    let metrics = RuntimeMetrics::new();
    metrics.inc_tasks();

    let _ = HypothesisGenerator::generate(directive).await;
    let _ = MetaReasoner::execute_with_got(&ctx.client, &ctx.api_key, directive).await;

    let (_tx_mem, rx_mem) = tokio::sync::mpsc::channel::<crate::memory_actor::MemoryCommand>(1);
    let _mem_actor_1 = crate::memory_actor::MemoryActor::new(rx_mem);

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
            Ok(format!("CorticalLoop self-halted: {}", reasoning))
        }
        CortexOutcome::TurnsExhausted { best_belief } => Err(anyhow::anyhow!(
            "CorticalLoop exhausted turns without proving the strategy (best belief {:.2})",
            best_belief
        )),
    }
}
""")
    print("[OK] Unified memory actor locked in task_manager_v4.rs")
