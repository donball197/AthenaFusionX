pub mod agent_protocol;
pub mod agents;
pub mod arbiter_agent;
pub mod ast_editor;
pub mod cli_utils;
pub mod cluster;
pub mod conductor;
pub mod diff;
pub mod distributed_scheduler;
pub mod executor;
pub mod factory;
pub mod json_parser;
pub mod logic_engine;
pub mod metrics;
pub mod neural_agent;
pub mod orchestrator;
pub mod podman;
pub mod router;
pub mod shared_state;
pub mod task_manager;
pub mod wasm_forge;
pub mod wasm_runtime;

pub async fn establish_neural_uplink() {
    println!(">> Neural Uplink Established: NEXUS CORTEX ONLINE.");
}
pub mod agent_runtime;
pub mod governance;
pub mod inquisitor;
pub mod lazarus_vault;
pub mod orchestrator_v2;
pub mod self_healing_engine;
pub mod v5_engine {
    pub mod actor;
    pub mod capabilities;
    pub mod event;
    pub mod graph;
}
pub mod model_config;

pub mod event_bus;
pub mod memory_actor;
pub mod graph_validator;
pub mod wasm_executor;
pub mod runtime_metrics;
