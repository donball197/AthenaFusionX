#![allow(clippy::too_many_arguments)]
#![allow(unreachable_code)]
use anyhow::Result;
use reqwest::Client;
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};

use crate::action_gate::ActionGate;
use crate::agent_protocol::AgentAction;
use crate::cognition::epistemic_session::EpistemicSession;
use crate::cognition_graph::{EdgeRelation, NodeKind};
use crate::contract_status::{ContractEvaluator, ContractStatus};
use crate::distributed_scheduler::DistributedScheduler;
use crate::epistemic_engine::EpistemicEngine;
use crate::executor::RoboticExecutor;
use crate::intent_contract::IntentContract;
use crate::memory_cortex::MemoryCortex;
use crate::motor_cortex::MotorCortex;
use crate::policy::Capability;
use crate::shared_state::VmoState;
use crate::world_model::WorldModel;

#[derive(Debug)]
pub enum CortexOutcome {
    CompletionProposed { belief: f32, evidence: String },
    TurnsExhausted { best_belief: f32, evidence: String },
}

enum CortexState {
    Strategize,
    Critique { proposal: String, belief: f32 },
    Translate { proposal: String, belief: f32 },
    Execute { actions: Vec<AgentAction>, proposal: String, belief: f32 },
    Evaluate { evidence: String, proposal: String, belief: f32, is_halt_action: bool, mutation_path: Option<String> },
    Complete { belief: f32, evidence: String },
    Fail { belief: f32, evidence: String },
}

pub struct CorticalLoop {
    client: Client,
    api_key: String,
    world_model: Arc<WorldModel>,
    memory: Arc<MemoryCortex>,
    vmo_state: Arc<Mutex<VmoState>>,
    dist: Arc<DistributedScheduler>,
    tx: broadcast::Sender<String>,
    action_gate: ActionGate,
}

impl CorticalLoop {
    pub fn new(
        api_key: String,
        world_model: Arc<WorldModel>,
        memory: Arc<MemoryCortex>,
        vmo_state: Arc<Mutex<VmoState>>,
        dist: Arc<DistributedScheduler>,
        tx: broadcast::Sender<String>,
    ) -> Self {
        Self {
            client: Client::new(),
            api_key,
            world_model,
            memory,
            vmo_state,
            dist,
            tx,
            action_gate: ActionGate::new(),
        }
    }

    fn log(&self, msg: String) {
        let _ = self.tx.send(
            serde_json::json!({"type": "NEURAL_STREAM", "payload": format!("{}\n", msg)})
                .to_string(),
        );
    }

    async fn resolve_workspace_paths() -> String {
        match tokio::process::Command::new("cargo")
            .args(["metadata", "--no-deps", "--format-version=1"])
            .output()
            .await
        {
            Ok(output) if output.status.success() => {
                match serde_json::from_slice::<serde_json::Value>(&output.stdout) {
                    Ok(meta) => {
                        let workspace_root = meta.get("workspace_root").and_then(|v| v.as_str()).unwrap_or("<unresolved>");
                        let target_dir = meta.get("target_directory").and_then(|v| v.as_str()).unwrap_or("<unresolved>");
                        format!("WORKSPACE FACTS (ground truth, do not guess): workspace_root={workspace_root} | target_dir={target_dir}.")
                    }
                    Err(e) => format!("WORKSPACE FACTS: cargo metadata parse failed ({e})."),
                }
            }
            Ok(output) => format!("WORKSPACE FACTS: cargo metadata exited non-zero ({}).", output.status),
            Err(e) => format!("WORKSPACE FACTS: cargo metadata failed to run ({e})."),
        }
    }

    fn compress_context(ctx: &str, max_chars: usize) -> String {
        if ctx.len() <= max_chars {
            return ctx.to_string();
        }
        let chunk_size = max_chars / 2;
        let start = &ctx[..chunk_size];
        let end = &ctx[ctx.len() - chunk_size..];
        format!("{}...\n\n[... {} CHARACTERS COMPRESSED TO PREVENT CONTEXT OVERFLOW ...]\n\n{}", start, ctx.len() - max_chars, end)
    }

    pub async fn ignite(&mut self, mission: &str) -> Result<CortexOutcome> {
        self.ignite_with_evidence(mission, None).await
    }

    pub async fn ignite_with_evidence(
        &mut self,
        mission: &str,
        ancestor_evidence: Option<&str>,
    ) -> Result<CortexOutcome> {
        self.log("\x1b[1;36m>> [CORTEX] Igniting Epistemic Reasoning Session\x1b[0m".to_string());

        let mut context_chunks = (*self.world_model).semantic_query(mission).await;
        context_chunks.push(Self::resolve_workspace_paths().await);
        if let Some(evidence) = ancestor_evidence {
            if !evidence.trim().is_empty() {
                context_chunks.push(format!("\n[ANCESTOR NODE EVIDENCE]\n{}", evidence));
            }
        }
        let world_context = Self::compress_context(&context_chunks.join("\n"), 12000);

        let mut session = EpistemicSession::new(mission);

        self.log("\x1b[36m>> [CORTEX] Generating competing strategies...\x1b[0m".to_string());
        session.budget.record_call();
        let hypotheses = EpistemicEngine::generate_hypotheses(&self.client, &self.api_key, mission, &world_context).await?;

        for h in hypotheses {
            session.graph.add_node(NodeKind::Hypothesis, &h, 0.5, 100);
            self.log(format!("\x1b[90m  -> Strategy: {}\x1b[0m", h));
        }

        let is_mutation_task = session.contract.is_mutation_task;
        let is_write_task = is_mutation_task
            && (session.contract.objective.to_lowercase().contains("write")
                || session.contract.objective.to_lowercase().contains("export")
                || session.contract.objective.to_lowercase().contains(".md")
                || session.contract.objective.to_lowercase().contains(".json"));

        let mut state = CortexState::Strategize;
        let mut turn = 0;

        loop {
            turn += 1;
            self.log(format!("\n\x1b[1;33m>> [CORTEX] Cognitive Turn {} / {}\x1b[0m", turn, session.budget.max_turns));

            if let Some(best_node) = session.graph.select_best_hypothesis().cloned() {
                let current_belief = best_node.belief_score;
                if session.budget.is_exhausted(turn) {
                    if current_belief >= 0.55 && session.consecutive_mutation_vetoes < 3 && turn < 300 {
                        self.log(format!("\x1b[1;35m>> [COGNITIVE MOMENTUM] Call limit reached, but Belief is high ({:.2}). Dynamically extending budget.\x1b[0m", current_belief));
                        session.budget.max_turns += 20;
                        session.budget.max_model_calls += 80;
                    } else {
                        self.log("\x1b[1;31m>> [SUBGOAL_BUDGET_EXHAUSTED] Cognitive call limit reached with low belief or stagnation. Node failed.\x1b[0m".to_string());
                        return Ok(CortexOutcome::TurnsExhausted { best_belief: current_belief, evidence: session.collect_evidence(best_node.id) });
                    }
                }
            } else if session.budget.is_exhausted(turn) {
                return Ok(CortexOutcome::TurnsExhausted { best_belief: 0.0, evidence: "No viable hypotheses left.".to_string() });
            }

            state = match state {
                CortexState::Strategize => self.handle_strategize(&mut session, mission, &world_context, is_mutation_task, is_write_task).await?,
                CortexState::Critique { proposal, belief } => self.handle_critic(&mut session, mission, &world_context, proposal, belief, is_mutation_task, is_write_task).await?,
                CortexState::Translate { proposal, belief } => self.handle_translate(&mut session, proposal, belief).await?,
                CortexState::Execute { actions, proposal, belief } => self.handle_execute(&mut session, actions, proposal, belief).await?,
                CortexState::Evaluate { evidence, proposal, belief, is_halt_action, mutation_path } => self.handle_evaluate(&mut session, mission, evidence, proposal, belief, is_mutation_task, is_write_task, is_halt_action, mutation_path).await?,
                CortexState::Complete { belief, evidence } => return Ok(CortexOutcome::CompletionProposed { belief, evidence }),
                CortexState::Fail { belief, evidence } => return Ok(CortexOutcome::TurnsExhausted { best_belief: belief, evidence }),
            };
        }
    }

    async fn handle_strategize(&mut self, session: &mut EpistemicSession, mission: &str, world_context: &str, is_mutation_task: bool, is_write_task: bool) -> Result<CortexState> {
        let best_node = match session.graph.select_best_hypothesis().cloned() {
            Some(node) => node,
            None => return Ok(CortexState::Fail { belief: 0.0, evidence: "No viable hypotheses left.".to_string() })
        };

        let h_id = best_node.id;
        let h_content = best_node.content.clone();
        let current_belief = best_node.belief_score;

        self.log(format!("\x1b[32m>> [CORTEX] Exploring Top Strategy (Belief: {:.2}):\x1b[0m {}", current_belief, h_content));

        let status = ContractEvaluator::evaluate(is_mutation_task, is_write_task, &session.local_evidence, current_belief, mission);

        if status == ContractStatus::Satisfied {
            self.log("\x1b[1;36m>> [CORTEX OMNI-GUARD] Contract mathematically satisfied. Stripping LLM authority and forcing deterministic node completion.\x1b[0m".to_string());
            return Ok(CortexState::Complete { belief: current_belief, evidence: session.collect_evidence(h_id) });
        }

        let evidence_history = Self::compress_context(&session.collect_evidence(h_id), 64000);
        session.budget.record_call();
        
        match EpistemicEngine::propose_action(&self.client, &self.api_key, mission, &h_content, world_context, &evidence_history).await {
            Ok(proposal) => {
                self.log(format!("\x1b[35m>> [STRATEGIST PROPOSAL]:\x1b[0m {}", proposal));
                let prop_lower = proposal.trim().to_lowercase();
                if prop_lower == "halt" || prop_lower.starts_with("halt ") {
                    if status == ContractStatus::Satisfied || status == ContractStatus::PartialEvidence {
                        self.log("\x1b[1;36m>> [CORTEX] DETERMINISTIC GUARD: Halt proposed following successful local evidence. Subtask complete.\x1b[0m".to_string());
                        return Ok(CortexState::Complete { belief: 1.0, evidence: session.collect_evidence(h_id) });
                    } else if (is_mutation_task || is_write_task) && status == ContractStatus::NoEvidence {
                        if current_belief > 0.90 {
                            self.log("\x1b[1;33m>> [CORTEX GUARD OVERRIDE] High belief (>.90) halt proposed without mutation. Allowing Evaluator to assess validity.\x1b[0m".to_string());
                        } else {
                            self.log("\x1b[1;31m>> [CORTEX FAULT] Halt rejected: Objective requires patching/writing, but no mutation success marker found.\x1b[0m".to_string());
                            let obs_id = session.graph.add_node(NodeKind::Observation, "[HALT_REJECTED] The directive requires modifying files. You proposed halt without successfully patching anything. You MUST use diff_patch, patch_function, or write_file.", 1.0, 50);
                            session.graph.add_edge(obs_id, h_id, EdgeRelation::Contradicts, 0.6);
                            session.graph.propagate_beliefs();
                            return Ok(CortexState::Strategize);
                        }
                    } else {
                        self.log("\x1b[1;33m>> [CORTEX] DETERMINISTIC GUARD SUPPRESSED: Halt proposed but evidence covers only part of a compound objective; continuing to explore.\x1b[0m".to_string());
                    }
                }
                Ok(CortexState::Critique { proposal, belief: current_belief })
            }
            Err(e) => {
                self.log(format!("\x1b[31m>> [STRATEGIST FAULT]\x1b[0m {}", e));
                Ok(CortexState::Strategize)
            }
        }
    }

    async fn handle_critic(&mut self, session: &mut EpistemicSession, mission: &str, world_context: &str, proposal: String, belief: f32, is_mutation_task: bool, is_write_task: bool) -> Result<CortexState> {
        self.log("\x1b[31m>> [RED TEAM] Criticizing proposal...\x1b[0m".to_string());
        
        let action_key = proposal.trim().to_string();
        if session.is_action_attempted(&action_key) {
            let status = ContractEvaluator::evaluate(is_mutation_task, is_write_task, &session.local_evidence, belief, mission);
            if status == ContractStatus::Satisfied || status == ContractStatus::PartialEvidence {
                self.log("\x1b[1;36m>> [SUBGOAL_COMPLETE] Loop detected but evidence is sufficient. Halting.\x1b[0m".to_string());
                return Ok(CortexState::Complete { belief, evidence: session.collect_evidence(session.best_hypothesis_id.unwrap_or_default()) });
            } else {
                self.log(format!("\x1b[1;31m>> [ENTERPRISE CORTEX] Anti-Loop Guard Triggered: Action [{}] already attempted.\x1b[0m", action_key));
                let obs_id = session.graph.add_node(NodeKind::Observation, "[CRITICAL VETO] You are repeating a failed action! YOU MUST MUTATE YOUR STRATEGY, USE A DIFFERENT TOOL, OR HALT.", 1.0, 95);
                if let Some(h_id) = session.best_hypothesis_id {
                    session.graph.add_edge(obs_id, h_id, EdgeRelation::Contradicts, 0.99);
                    session.graph.propagate_beliefs();
                }
                return Ok(CortexState::Strategize);
            }
        }
        session.record_action_attempt(&action_key);

        session.budget.record_call();
        match EpistemicEngine::criticize_proposal(&self.client, &self.api_key, mission, &proposal, world_context, &session.local_evidence, &session.local_evidence).await {
            Err(e) => {
                self.log(format!("\x1b[31m>> [CORTEX FATAL] Red Team critic parser panicked: {}\x1b[0m", e));
                if let Some(h_id) = session.best_hypothesis_id {
                    let obs_id = session.graph.add_node(NodeKind::Observation, "[CRITIC_PARSE_FAILURE]", 1.0, 50);
                    session.graph.add_edge(obs_id, h_id, EdgeRelation::Contradicts, 0.15);
                    session.graph.propagate_beliefs();
                }
                Ok(CortexState::Strategize)
            }
            Ok(mut critic) => {
                self.log(format!("\x1b[31m>> [CRITIC] Verdict:\x1b[0m {} (Modifier: {:.2})", critic.critique, critic.confidence_modifier));

                let is_mutation_shaped_proposal = proposal.contains("write_file") || proposal.contains("patch_function") || proposal.contains("diff_patch") || proposal.contains("replace_lines");
                let contract_still_unmet = (is_mutation_task || is_write_task) && !session.local_evidence.lines().any(|l| l.starts_with("[WRITE_OK]") || l.starts_with("[PATCH_FUNCTION_OK]") || l.starts_with("[REPLACE_LINES_OK]") || l.starts_with("[DIFF_PATCH_OK]"));

                if critic.confidence_modifier <= -0.2 && is_mutation_shaped_proposal && contract_still_unmet {
                    session.consecutive_mutation_vetoes += 1;
                    if session.consecutive_mutation_vetoes >= 3 {
                        self.log(format!("\x1b[1;35m>> [CIRCUIT_BREAKER_OVERRIDE] Critic vetoed {} consecutive mutation proposals while the contract's mutation marker is still missing. Overriding veto and executing.\x1b[0m", session.consecutive_mutation_vetoes));
                        critic.confidence_modifier = 0.1;
                        session.consecutive_mutation_vetoes = 0;
                    }
                } else {
                    session.consecutive_mutation_vetoes = 0;
                }

                if critic.confidence_modifier > -0.2 {
                    Ok(CortexState::Translate { proposal, belief })
                } else {
                    Ok(CortexState::Strategize)
                }
            }
        }
    }

    async fn handle_translate(&mut self, session: &mut EpistemicSession, proposal: String, belief: f32) -> Result<CortexState> {
        self.log("\x1b[36m>> [CORTEX] Translating intent to physical action...\x1b[0m".to_string());
        session.budget.record_call();

        let parsed_actions = if let Ok(direct_actions) = crate::json_parser::extract_actions(&proposal) {
            if !direct_actions.is_empty() {
                self.log("\x1b[32m>> [CORTEX INTERCEPT] Direct JSON Proposal Intercept: Bypassing Translator LLM.\x1b[0m".to_string());
                Ok(direct_actions)
            } else {
                if let Ok(raw_json) = EpistemicEngine::translate_to_action(&self.client, &self.api_key, &proposal, &proposal).await {
                    crate::json_parser::extract_actions(&raw_json)
                } else {
                    Ok(vec![])
                }
            }
        } else {
            if let Ok(raw_json) = EpistemicEngine::translate_to_action(&self.client, &self.api_key, &proposal, &proposal).await {
                crate::json_parser::extract_actions(&raw_json)
            } else {
                Ok(vec![])
            }
        };

        let mut actions = parsed_actions.unwrap_or_default();
        if actions.is_empty() && !proposal.trim().is_empty() {
            if let Some(intercepted) = crate::action_intercept::intercept_fallback_action(&proposal) {
                self.log("\x1b[32m>> [CORTEX INTERCEPT] Native Tool Intercept: Handling action directly.\x1b[0m".to_string());
                actions.push(intercepted);
            } else {
                self.log("\x1b[31m>> [CORTEX FAULT] Translator produced empty JSON and proposal text matched no known tool. Refusing raw fallback execution.\x1b[0m".to_string());
                if let Some(h_id) = session.best_hypothesis_id {
                    let obs_id = session.graph.add_node(NodeKind::Observation, "[TRANSLATION_UNRECOGNIZED]", 1.0, 50);
                    session.graph.add_edge(obs_id, h_id, EdgeRelation::Contradicts, 0.3);
                    session.graph.propagate_beliefs();
                }
                return Ok(CortexState::Strategize);
            }
        }

        if actions.is_empty() {
            self.log("\x1b[31m>> [CORTEX FAULT] Translator produced an empty action list.\x1b[0m".to_string());
            if let Some(h_id) = session.best_hypothesis_id {
                let obs_id = session.graph.add_node(NodeKind::Observation, "[TRANSLATION_EMPTY]", 1.0, 50);
                session.graph.add_edge(obs_id, h_id, EdgeRelation::Contradicts, 0.15);
                session.graph.propagate_beliefs();
            }
            Ok(CortexState::Strategize)
        } else {
            Ok(CortexState::Execute { actions, proposal, belief })
        }
    }

    async fn handle_execute(
        &mut self, 
        session: &mut EpistemicSession, 
        actions: Vec<AgentAction>, 
        proposal: String,
        belief: f32
    ) -> Result<CortexState> {
        let mut executor = RoboticExecutor::new(self.api_key.clone(), self.tx.clone());
        let mut latest_evidence = String::new();
        let mut mutation_paths: Vec<String> = Vec::new();
        let mut last_is_halt = false;
        let mut rust_files_mutated = false;
        let evidence_checkpoint = session.local_evidence.len();

        for action in actions {
            let intent = IntentContract::new(&proposal, belief);
            let capability = match &action {
                AgentAction::ExecuteCommand { .. } => Capability::ExecuteSystemCall,
                AgentAction::WriteFile { .. } | AgentAction::DiffPatch { .. } | AgentAction::PatchFunction { .. } => Capability::WriteSubstrate,
                AgentAction::Halt { .. } => Capability::HaltCognition,
                _ => Capability::ReadSubstrate,
            };

            let mutation_path = match &action {
                AgentAction::WriteFile { path, .. } | AgentAction::DiffPatch { path, .. } | AgentAction::PatchFunction { path, .. } => Some(path.clone()),
                _ => None,
            };
            
            if let Some(path) = mutation_path {
                mutation_paths.push(path.clone());
                if path.ends_with(".rs") {
                    rust_files_mutated = true;
                }
            }

            let is_halt_action = matches!(action, AgentAction::Halt { .. }) || proposal.trim().to_lowercase().starts_with("halt");
            if is_halt_action {
                last_is_halt = true;
            }

            let motor_cmd = MotorCortex::translate(&intent, None, action, capability, proposal.clone());

            if let Err(gate_err) = self.action_gate.approve(&motor_cmd) {
                self.log(format!("\x1b[1;31m>> [ACTION GATE REJECTED]: {}\x1b[0m", gate_err));
                if let Some(h_id) = session.best_hypothesis_id {
                    let obs_id = session.graph.add_node(NodeKind::Observation, &format!("[ACTION_GATE_BLOCKED] {}", gate_err), 1.0, 50);
                    session.graph.add_edge(obs_id, h_id, EdgeRelation::Contradicts, 0.50);
                    session.graph.propagate_beliefs();
                }
                continue;
            }

            self.log("\x1b[35m>> [MOTOR CORTEX] Emitting authorized MotorCommand to RoboticExecutor...\x1b[0m".to_string());
            let evidence = match executor.execute_motor_command(motor_cmd, self.memory.clone(), self.vmo_state.clone(), self.dist.clone()).await {
                Ok(ev) => ev,
                Err(e) => format!("[ACTION FAILED] OS rejected action: {}", e)
            };

            latest_evidence.push_str(&evidence);
            latest_evidence.push('\n');
            session.local_evidence.push_str(&evidence);
            session.local_evidence.push('\n');

            let preview = evidence.lines().take(5).collect::<Vec<_>>().join("\n");
            self.log(format!("\x1b[32m>> [EVIDENCE ACQUIRED]\x1b[0m\n{}", preview));
        }

        if latest_evidence.trim().is_empty() {
            return Ok(CortexState::Strategize);
        }

        // ⚡ COMPILER AS A CRITIC + DETERMINISTIC AUTO-HEALER
        if rust_files_mutated {
            self.log("\x1b[36m>> [COMPILER CRITIC] Running cargo check to validate Rust mutation...\x1b[0m".to_string());
            
            let mut attempts = 0;
            loop {
                attempts += 1;
                if attempts > 3 {
                    self.log("\x1b[1;31m>> [COMPILER CRITIC] Max auto-heal attempts reached. Reverting.\x1b[0m".to_string());
                    let compile_fail_evidence = "[COMPILE_FAIL] Auto-healer attempted 3 fixes but errors persist.".to_string();
                    for path in &mutation_paths {
                        let _ = crate::podman::PodmanShield::execute(&format!("git checkout -- {}", path));
                    }
                    session.local_evidence.truncate(evidence_checkpoint);
                    session.local_evidence.push_str(&compile_fail_evidence);
                    session.local_evidence.push('\n');
                    if let Some(h_id) = session.best_hypothesis_id {
                        let obs_id = session.graph.add_node(NodeKind::Observation, "[CRITICAL VETO] Persistent compilation errors after 3 auto-heal attempts.", 1.0, 90);
                        session.graph.add_edge(obs_id, h_id, EdgeRelation::Contradicts, 0.99);
                        session.graph.propagate_beliefs();
                    }
                    return Ok(CortexState::Strategize);
                }

                let compile_output = tokio::process::Command::new("cargo")
                    .arg("check")
                    .arg("--message-format=json")
                    .output()
                    .await;

                match compile_output {
                    Ok(output) if output.status.success() => {
                        latest_evidence.push_str("\n[COMPILE_OK] Workspace compiles successfully.");
                        self.log("\x1b[32m>> [COMPILER CRITIC] Compile OK. Proceeding to LLM Evaluator.\x1b[0m".to_string());
                        break; // Exit loop, proceed to Evaluate
                    }
                    Ok(output) => {
                        let stdout = String::from_utf8_lossy(&output.stdout);
                        let mut applied_heals = 0;
                        
                        // Parse cargo JSON output line by line
                        for line in stdout.lines() {
                            if let Some(diag) = nexus_ast_tool::engine::parse_compiler_line(line) {
                                if let Some(action) = nexus_ast_tool::engine::determine_policy(&diag) {
                                    let file_path = std::path::PathBuf::from(&action.file);
                                    if file_path.exists() {
                                        if let Ok(source) = std::fs::read_to_string(&file_path) {
                                            if let Some(strategy) = nexus_ast_tool::mutation::get_strategy(&action.strategy) {
                                                match strategy.apply(&source, &action.symbol, &action.payload) {
                                                    Ok(new_source) => {
                                                        let _ = std::fs::write(&file_path, new_source);
                                                        applied_heals += 1;
                                                        self.log(format!("\x1b[1;36m>> [AUTO-HEAL] Applied {} to {}\x1b[0m", action.strategy, action.file));
                                                    }
                                                    Err(e) => self.log(format!("\x1b[33m>> [AUTO-HEAL FAIL] {}: {}\x1b[0m", action.strategy, e))
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        if applied_heals > 0 {
                            latest_evidence.push_str(&format!("\n[AUTO_HEAL] Applied {} deterministic fixes. Re-checking...", applied_heals));
                            session.local_evidence.push_str(&latest_evidence);
                            continue; // Re-run cargo check
                        }

                        // No healable errors found, standard failure path
                        let stderr = String::from_utf8_lossy(&output.stderr);
                        let err_preview = stderr.lines().take(15).collect::<Vec<_>>().join("\n");
                        
                        self.log("\x1b[1;31m>> [COMPILER CRITIC] Compile FAILED. Reverting mutations and bypassing LLM Evaluator.\x1b[0m".to_string());
                        
                        let compile_fail_evidence = format!("[COMPILE_FAIL] Your patch caused a compilation error:\n{}", err_preview);
                        
                        for path in &mutation_paths {
                            self.log(format!("\x1b[1;31m>> [CORTEX SAFETY] Reverting {} via git checkout.\x1b[0m", path));
                            let _ = crate::podman::PodmanShield::execute(&format!("git checkout -- {}", path));
                        }

                        session.local_evidence.truncate(evidence_checkpoint);
                        let stripped_this_turn = latest_evidence
                            .replace("[WRITE_OK]", "[MUTATION_REVERTED]")
                            .replace("[DIFF_PATCH_OK]", "[MUTATION_REVERTED]")
                            .replace("[PATCH_FUNCTION_OK]", "[MUTATION_REVERTED]")
                            .replace("[REPLACE_LINES_OK]", "[MUTATION_REVERTED]");
                        
                        session.local_evidence.push_str(&stripped_this_turn);
                        session.local_evidence.push_str(&compile_fail_evidence);
                        session.local_evidence.push('\n');
                        
                        if let Some(h_id) = session.best_hypothesis_id {
                            let obs_id = session.graph.add_node(
                                NodeKind::Observation, 
                                "[CRITICAL VETO] Your patch caused a compilation error. Files reverted. Try again with valid syntax.", 
                                1.0, 
                                90 
                            );
                            session.graph.add_edge(obs_id, h_id, EdgeRelation::Contradicts, 0.99);
                            session.graph.propagate_beliefs();
                        }
                        
                        return Ok(CortexState::Strategize);
                    }
                    Err(e) => {
                        self.log(format!("\x1b[33m>> [COMPILER CRITIC] Failed to run cargo check: {}. Proceeding with LLM Evaluator.\x1b[0m", e));
                        break;
                    }
                }
            }
        }

        Ok(CortexState::Evaluate { 
            evidence: latest_evidence, 
            proposal: proposal.clone(), 
            belief, 
            is_halt_action: last_is_halt, 
            mutation_path: mutation_paths.last().cloned()
        })
    }

    async fn handle_evaluate(&mut self, session: &mut EpistemicSession, _mission: &str, evidence: String, proposal: String, belief: f32, _is_mutation_task: bool, _is_write_task: bool, is_halt_action: bool, mutation_path: Option<String>) -> Result<CortexState> {
        self.log("\x1b[33m>> [CORTEX] Evaluating physical evidence against strategy...\x1b[0m".to_string());
        session.budget.record_call();
        
        let h_id = session.best_hypothesis_id.unwrap_or_default();
        let eval_evidence_context = format!("Current Action Output:\n{}\n\nPrior Evidence in Task:\n{}", evidence, session.local_evidence);

        match EpistemicEngine::evaluate_evidence(&self.client, &self.api_key, &proposal, &proposal, &eval_evidence_context).await {
            Err(e) => {
                self.log(format!("\x1b[31m>> [CORTEX FATAL] Evaluator panicked: {}\x1b[0m", e));
                Ok(CortexState::Strategize)
            }
            Ok(eval) => {
                self.log(format!("\x1b[33m>> [EVALUATION]\x1b[0m {} (Modifier: {:.2})", eval.critique, eval.confidence_modifier));
                session.record_evidence(&evidence, &eval, h_id);

                if is_halt_action && eval.confidence_modifier >= 0.0 && belief > 0.85 {
                    self.log("\x1b[1;36m>> [CORTEX TERMINAL] Valid halt executed and verified. Submitting evidence to ContractVerifier.\x1b[0m".to_string());
                    return Ok(CortexState::Complete { belief: belief.max(0.95), evidence: session.collect_evidence(h_id) });
                }

                if eval.confidence_modifier < 0.0 {
                    if let Some(path) = &mutation_path {
                        self.log(format!("\x1b[1;31m>> [CORTEX SAFETY] Mutation to '{}' scored negative ({:.2}) -- reverting via git checkout.\x1b[0m", path, eval.confidence_modifier));
                        let _ = crate::podman::PodmanShield::execute(&format!("git checkout -- {}", path));
                        session.local_evidence = session.local_evidence.replace(&evidence, "");
                        let reverted_tag = format!("[MUTATION_REVERTED] {} -- Evaluator rejected this mutation ({:.2}): {}\n", path, eval.confidence_modifier, eval.critique);
                        session.local_evidence.push_str(&reverted_tag);
                    }
                }

                if let Some(updated_node) = session.graph.nodes.get(&h_id) {
                    self.log(format!("\x1b[1;32m>> [CORTEX] Strategy Belief updated to {:.2}\x1b[0m", updated_node.belief_score));
                    if session.is_subgoal_satisfied(updated_node.belief_score) {
                        self.log("\x1b[1;32m>> [CORTEX] Contract criteria met. Awaiting explicit Agent HALT.\x1b[0m".to_string());
                    }
                }
                Ok(CortexState::Strategize)
            }
        }
    }
}
