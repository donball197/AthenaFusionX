#!/bin/bash
# Root Setup: codebase_dumper.sh
# Universal execution across ARM64 DNA (Termux/Crostini)

FILES=(
    # --- Configuration & Root Substrate ---
    "Cargo.toml"
    "config/prompts.yaml"
    "README.md"
    "nexus.sh"

    # --- Runtime Core: Engine, Loop & Protocols ---
    "crates/runtime_core/src/lib.rs"
    "crates/runtime_core/src/cortical_loop.rs"
    "crates/runtime_core/src/sovereign_core.rs"
    "crates/runtime_core/src/task_manager.rs"
    "crates/runtime_core/src/executive_cortex.rs"
    "crates/runtime_core/src/epistemic_engine.rs"
    "crates/runtime_core/src/hypothesis_engine.rs"
    "crates/runtime_core/src/contract_status.rs"
    "crates/runtime_core/src/completion_engine.rs"
    "crates/runtime_core/src/evidence_markers.rs"
    "crates/runtime_core/src/agent_protocol.rs"
    "crates/runtime_core/src/json_parser.rs"
    "crates/runtime_core/src/tools/schema.rs"

    # --- Runtime Core: Orchestrator V2 & GoT Engine ---
    "crates/runtime_core/src/orchestrator_v2/mod.rs"
    "crates/runtime_core/src/orchestrator_v2/planner.rs"
    "crates/runtime_core/src/orchestrator_v2/scheduler.rs"
    "crates/runtime_core/src/orchestrator_v2/task_graph.rs"
    "crates/runtime_core/src/orchestrator_v2/verifier.rs"
    "crates/runtime_core/src/orchestrator_v2/critic.rs"
    "crates/runtime_core/src/orchestrator_v2/got_engine.rs"
    "crates/runtime_core/src/orchestrator_v2/got/mod.rs"

    # --- Runtime Core: Execution, Safety & Routing ---
    "crates/runtime_core/src/executor.rs"
    "crates/runtime_core/src/action_gate.rs"
    "crates/runtime_core/src/action_intercept.rs"
    "crates/runtime_core/src/motor_cortex.rs"
    "crates/runtime_core/src/compiler_critic.rs"
    "crates/runtime_core/src/provider.rs"
    "crates/runtime_core/src/router.rs"
    "crates/runtime_core/src/model_config.rs"
    "crates/runtime_core/src/anti_loop.rs"

    # --- Runtime Core: Memory, World Model & Evolution ---
    "crates/runtime_core/src/world_model.rs"
    "crates/runtime_core/src/memory_cortex.rs"
    "crates/runtime_core/src/memory_consolidation.rs"
    "crates/runtime_core/src/reflection_engine.rs"
    "crates/runtime_core/src/evolution_engine.rs"
    "crates/runtime_core/src/bayesian_calibration.rs"
    "crates/runtime_core/src/distributed_scheduler.rs"
    "crates/runtime_core/src/cognition/blackboard.rs"
    "crates/runtime_core/src/repository_intelligence/proposal.rs"
    "crates/runtime_core/src/observation_adapter.rs"

    # --- Nexus Substrate: Vector Bridge & Storage ---
    "crates/nexus_substrate/src/vector_bridge.rs"
    "crates/nexus_substrate/src/code_graph.rs"
    "crates/nexus_substrate/src/memory.rs"

    # --- DAG Orchestrator ---
    "crates/nexus_dag_orchestrator/src/lib.rs"
    "crates/nexus_dag_orchestrator/src/main.rs"
    "crates/nexus_dag_orchestrator/src/executor.rs"
    "crates/nexus_dag_orchestrator/src/graph.rs"
    "crates/nexus_dag_orchestrator/src/planner.rs"
    "crates/nexus_dag_orchestrator/src/bin/nexus_orchestrator.rs"

    # --- Edge Minion ---
    "crates/nexus_edge_minion/src/api_gateway.rs"

    # --- Binaries & Apps ---
    "bins/cli/src/main.rs"
    "bins/ide_backend/src/server.rs"
    "apps/nexus_ast_tool/src/sanitizer.rs"
)

# Initialize/clear the dump file
> codebase_dump.txt

for f in "${FILES[@]}"; do
    if [ -f "$f" ]; then
        echo "=========================================" >> codebase_dump.txt
        echo "FILE: $f" >> codebase_dump.txt
        echo "=========================================" >> codebase_dump.txt
        cat -n "$f" >> codebase_dump.txt
        echo "" >> codebase_dump.txt
    else
        echo "[WARNING] File not found: $f"
    fi
done

echo "[SUCCESS] AthenaFusionX complete codebase dumped to codebase_dump.txt"
