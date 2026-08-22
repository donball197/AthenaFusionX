pub mod evidence_markers;
pub mod agent_protocol;
pub mod arbiter_agent;
pub mod cli_utils;
pub mod cluster;
pub mod cognition;
pub mod cognition_graph;
pub mod conductor;
pub mod cortical_loop;
pub mod critic;
pub mod diff;
pub mod distributed_scheduler;
pub mod epistemic_engine;
pub mod event_bus;
pub mod evolution_engine;
pub mod executor;
pub mod fitness_evaluator;
pub mod governance;
pub mod hypothesis;
pub mod hypothesis_engine;
pub mod json_parser;
pub mod lazarus_vault;
pub mod light_verifier;
pub mod logger;
pub mod logic_engine;
pub mod memory_actor;
pub mod metrics;
pub mod model_config;
pub mod orchestrator;
pub mod orchestrator_v2;
pub mod podman;
pub mod policy;
pub mod router;
pub mod runtime_metrics;
pub mod shared_state;
pub mod sidecar_host;
pub mod task_manager;
pub mod wasm_forge;
pub mod wasm_runtime;
pub mod world_model;

pub mod verification;

pub mod intent_contract;

pub mod motor_cortex;

pub mod cognitive_events;
pub mod causal_recall;
pub mod evidence;
pub mod memory_consolidation;
pub mod memory_cortex;
pub mod memory_recall;
pub mod memory_semantic;
pub mod memory_types;
pub mod reflection_engine;
pub mod goal_adaptation;
pub mod executive_cortex;

pub mod action_gate;
pub mod action_intercept;
pub mod completion_engine;
pub mod outcome_verifier;
pub mod tools;
pub mod belief_policy;
pub mod belief_update;
pub mod belief_transition;
pub mod belief_causality;
pub mod belief_validation;
pub mod belief_feedback;
pub mod ast_editor;

pub mod config;
pub mod client;

pub use config::NexusConfig;
pub use client::ApiClient;
pub mod contract_status;
