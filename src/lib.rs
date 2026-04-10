/**
 * Copyright 2026 Donald Ball
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 * http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR REPRESENTATIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */


// src/lib.rs — v4.0.4 Realigned (Mission Queue Restored)
pub mod agent_protocol;
pub mod alloc;
pub mod arbiter_agent;
pub mod auth;
pub mod cli;
pub mod component_runner;
pub mod diff;
pub mod executor;
pub mod fs_utils;
pub mod janitor;
pub mod json_parser;
pub mod kernel;
pub mod lazarus_vault;
pub mod memory;
pub mod metrics;
pub mod mission_control;
pub mod mission_planner;
pub mod mission_queue; // SUTURE: Restored missing module declaration
pub mod neural_agent;
pub mod orchestrator;
pub mod podman;
pub mod polyglot;
pub mod resonator;
pub mod safety_valve;
pub mod sandbox;
pub mod scheduler;
pub mod scraper_drone;
pub mod self_healing_engine;
pub mod server;
pub mod system_monitor;
pub mod task_manager;
pub mod thermal_governor;
pub mod utils;
pub mod validator;

// Restored missing logic strands
pub mod broker;
pub mod inquisitor;
pub mod local_llm;
pub mod server_terminal;

#[cfg(feature = "vision")]
pub mod vision;

// Future / Heavy strands (Keep pruned if RAM is tight)
// pub mod void_walker;
