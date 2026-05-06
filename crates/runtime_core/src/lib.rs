pub mod agents;
pub mod agent_protocol;
pub mod neural_agent;
pub mod orchestrator;
pub mod json_parser;
pub mod factory;
pub mod conductor;
pub mod router;
pub mod logic_engine;
pub mod metrics;
pub mod wasm_forge;
pub mod wasm_runtime;
pub mod arbiter_agent;
pub mod podman;
pub mod diff;
pub mod ast_editor;
pub mod cluster;
pub mod distributed_scheduler;
pub mod executor;
pub mod task_manager;
pub mod shared_state;
pub mod cli_utils;

pub async fn establish_neural_uplink() {
    println!(">> Neural Uplink Established: NEXUS CORTEX ONLINE.");
}
pub mod agent_runtime;
pub mod orchestrator_v2;
