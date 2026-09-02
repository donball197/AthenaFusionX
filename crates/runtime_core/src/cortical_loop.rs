#![allow(clippy::too_many_arguments)]
#![allow(unreachable_code)]
use anyhow::Result;
use reqwest::Client;
use std::sync::Arc;
use std::fs;
use std::io::Write;
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
use crate::anti_loop::{score_search_relevance, should_raise_belief};

#[derive(Debug)]
pub enum CortexOutcome {
    CompletionProposed { belief: f32, evidence: String },
    TurnsExhausted { best_belief: f32, evidence: String },
}

enum CortexState {
    Strategize,
    Critique { actions: Vec<AgentAction>, proposal: String, belief: f32 },
    Execute { actions: Vec<AgentAction>, proposal: String, belief: f32 },
    Evaluate { evidence: String, proposal: String, belief: f32, is_halt_action: bool, mutation_path: Option<String> },
    Complete { belief: f32, evidence: String },
    Fail { belief: f32, evidence: String },
}

fn is_terminal_strategy(hypothesis: &str) -> bool {
    let h = hypothesis.to_lowercase();
    h.contains("synthesize_report")
        || h.contains("write_file")
        || h.contains("write the report")
        || h.contains("output the report")
        || h.contains("overwrite")
        || h.contains("finalize")
        || h.contains("halt")
}

fn extract_target_artifact(mission: &str) -> Option<String> {
    let contract = crate::completion_engine::SubgoalCompletionEngine::infer_contract("ad_hoc", mission);
    if let Some(art) = contract.required_artifact_paths.first() {
        return Some(art.path.clone());
    }
    for word in mission.split_whitespace() {
        let clean = word.trim_matches(|c: char| c == '`' || c == '\'' || c == '"' || c == ',' || c == '.');
        if (clean.ends_with(".md") || clean.ends_with(".json") || clean.ends_with(".rs") || clean.ends_with(".txt"))
            && (clean.contains('/') || clean.starts_with("apps") || clean.starts_with("crates"))
        {
            return Some(clean.to_string());
        }
    }
    None
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
            action_gate: ActionGate::with_capabilities(vec![
                Capability::ReadSubstrate,
                Capability::WriteSubstrate,
                Capability::HaltCognition,
                Capability::ExecuteSystemCall,
            ]),
        }
    }

    fn log(&self, msg: String) {
        let _ = self.tx.send(
            serde_json::json!({"type": "NEURAL_STREAM", "payload": format!("{}\n", msg)})
                .to_string(),
        );
    }

    fn dump_debug(&self, turn: u32, phase: &str, payload: &str) {
        let dir = ".nexus_debug";
        let _ = fs::create_dir_all(dir);
        let filepath = format!("{}/turn_{}_{}.txt", dir, turn, phase);
        if let Ok(mut file) = fs::File::create(&filepath) {
            let _ = file.write_all(payload.as_bytes());
        }
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
                        format!("WORKSPACE FACTS: workspace_root={workspace_root} | target_dir={target_dir}.")
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
        format!("{}...\n\n[... {} CHARACTERS COMPRESSED ...]\n\n{}", start, ctx.len() - max_chars, end)
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

        let mut context_chunks = self.world_model.semantic_query(mission).await;
        context_chunks.push(Self::resolve_workspace_paths().await);
        if let Some(evidence) = ancestor_evidence {
            if !evidence.trim().is_empty() {
                context_chunks.push(format!("\n[ANCESTOR NODE EVIDENCE]\n{}", evidence));
            }
        }
        let world_context = Self::compress_context(&context_chunks.join("\n"), 2500);

        let mut session = EpistemicSession::new(mission);

        self.log("\x1b[36m>> [CORTEX] Generating competing strategies...\x1b[0m".to_string());
        session.budget.record_call();
        let hypotheses = EpistemicEngine::generate_hypotheses(&self.client, &self.api_key, mission, &world_context).await?;

        for h in hypotheses {
            let hid = session.graph.add_node(NodeKind::Hypothesis, &h, 0.5, 100);
            if session.best_hypothesis_id.is_none() {
                session.best_hypothesis_id = Some(hid);
            }
            self.log(format!("\x1b[90m  -> Strategy: {}\x1b[0m", h));
        }

        let is_mutation_task = session.contract.is_mutation_task || mission.to_lowercase().contains("synthesize_report") || mission.to_lowercase().contains(".md");
        let is_write_task = is_mutation_task
            || session.contract.objective.to_lowercase().contains("write")
            || session.contract.objective.to_lowercase().contains("export")
            || session.contract.objective.to_lowercase().contains(".md")
            || session.contract.objective.to_lowercase().contains(".json")
            || mission.to_lowercase().contains("synthesize_report");

        let mut state = CortexState::Strategize;
        let mut turn = 0;
        let mut shift_rx = self.dist.file_shift_tx.subscribe();

        loop {
            turn += 1;
            self.log(format!("\n\x1b[1;33m>> [CORTEX] Cognitive Turn {} / {}\x1b[0m", turn, session.budget.max_turns));

            // Active code-shift listener
            while let Ok(event) = shift_rx.try_recv() {
                self.log(format!(
                    "\x1b[1;33m>> [CODE-SHIFT] Concurrent mutation detected on `{}` via {}. Invalidating session cache.\x1b[0m",
                    event.path, event.modifier
                ));
                let shift_obs = format!(
                    "[CODE_SHIFT_ALERT] File `{}` was just modified externally. You MUST re-read this file before proposing edits.",
                    event.path
                );
                session.local_evidence.push_str(&format!("\n{}\n", shift_obs));
                if let Some(h_id) = session.best_hypothesis_id {
                    let obs_id = session.graph.add_node(NodeKind::Observation, &shift_obs, 1.0, 85);
                    session.graph.add_edge(obs_id, h_id, EdgeRelation::Contradicts, 0.40);
                    session.graph.propagate_beliefs();
                }
            }

            if let Some(best_node) = session.graph.select_best_hypothesis().cloned() {
                let current_belief = best_node.belief_score;
                if session.budget.is_exhausted(turn) {
                    if current_belief >= 0.80 && session.consecutive_mutation_vetoes < 2 && turn < 80 {
                        self.log(format!("\x1b[1;35m>> [COGNITIVE MOMENTUM] Call limit reached with high belief ({:.2}). Extending budget.\x1b[0m", current_belief));
                        session.budget.max_turns += 10;
                        session.budget.max_model_calls += 30;
                    } else {
                        self.log("\x1b[1;31m>> [SUBGOAL_BUDGET_EXHAUSTED] Cognitive call limit reached. Node finalized.\x1b[0m".to_string());
                        return Ok(CortexOutcome::TurnsExhausted { best_belief: current_belief, evidence: session.collect_evidence(best_node.id) });
                    }
                }
            } else if session.budget.is_exhausted(turn) {
                return Ok(CortexOutcome::TurnsExhausted { best_belief: 0.0, evidence: "No viable hypotheses left.".to_string() });
            }

            state = match state {
                CortexState::Strategize => self.handle_strategize(turn, &mut session, mission, &world_context, is_mutation_task, is_write_task).await?,
                CortexState::Critique { actions, proposal, belief } => self.handle_critic(turn, &mut session, mission, &world_context, actions, proposal, belief, is_mutation_task, is_write_task).await?,
                CortexState::Execute { actions, proposal, belief } => self.handle_execute(&mut session, actions, proposal, belief).await?,
                CortexState::Evaluate { evidence, proposal, belief, is_halt_action, mutation_path } => self.handle_evaluate(&mut session, mission, evidence, proposal, belief, is_mutation_task, is_write_task, is_halt_action, mutation_path).await?,
                CortexState::Complete { belief, evidence } => return Ok(CortexOutcome::CompletionProposed { belief, evidence }),
                CortexState::Fail { belief, evidence } => return Ok(CortexOutcome::TurnsExhausted { best_belief: belief, evidence }),
            };
        }
    }

    async fn handle_strategize(&mut self, turn: u32, session: &mut EpistemicSession, mission: &str, world_context: &str, is_mutation_task: bool, is_write_task: bool) -> Result<CortexState> {
        let best_node = match session.graph.select_best_hypothesis().cloned() {
            Some(node) => node,
            None => return Ok(CortexState::Fail { belief: 0.0, evidence: "No viable hypotheses left.".to_string() })
        };

        let h_id = best_node.id;
        let h_content = best_node.content.clone();
        let current_belief = best_node.belief_score;
        session.best_hypothesis_id = Some(h_id);

        self.log(format!("\x1b[32m>> [CORTEX] Exploring Strategy (Belief: {:.2}):\x1b[0m {}", current_belief, h_content));

        // 1. Check if contract is already physically satisfied on disk
        let status = ContractEvaluator::evaluate(is_mutation_task, is_write_task, &session.local_evidence, current_belief, mission);
        if status == ContractStatus::Satisfied {
            self.log("\x1b[1;36m>> [CORTEX OMNI-GUARD] Contract physically satisfied on substrate. Forcing node completion.\x1b[0m".to_string());
            return Ok(CortexState::Complete { belief: 1.0, evidence: session.collect_evidence(h_id) });
        }

        // 2. Terminal Strategy Forcing on High Belief
        if is_terminal_strategy(&h_content) && current_belief >= 0.88 {
            if let Some(target_path) = extract_target_artifact(mission) {
                self.log(format!(
                    "\x1b[1;35m>> [CORTEX] Terminal strategy detected with high belief ({:.2}). Forcing synthesis of `{}`.\x1b[0m",
                    current_belief, target_path
                ));

                let forced_action = AgentAction::SynthesizeReport {
                    output_path: target_path.clone(),
                    source_files: vec![],
                    reasoning: "Contract criteria verified. Generating final deliverable.".to_string(),
                };
                return Ok(CortexState::Execute {
                    actions: vec![forced_action],
                    proposal: format!("[FORCED SYNTHESIS] -> {}", target_path),
                    belief: current_belief,
                });
            }
        }

        let evidence_history = Self::compress_context(&session.collect_evidence(h_id), 3000);
        
        // Phase-aware mandate: Force transition from Recon to Write Deliverable
        let mission_contract = crate::completion_engine::SubgoalCompletionEngine::infer_contract("ad_hoc", mission);
        let unwritten_targets: Vec<&str> = mission_contract
            .required_artifact_paths
            .iter()
            .map(|a| a.path.as_str())
            .filter(|p| !crate::evidence_markers::has_artifact_write(&session.local_evidence, p))
            .collect();

        let enhanced_mission = if !unwritten_targets.is_empty() && crate::evidence_markers::has_recon_marker(&session.local_evidence) {
            format!(
                "{}\n\n[PHASE MANDATE: RECONNAISSANCE COMPLETE]\nYou have already read the input files. DO NOT search, read, or list directories again. You MUST now write the complete content directly to '{}' using `write_file`.",
                mission, unwritten_targets[0]
            )
        } else {
            mission.to_string()
        };
        session.budget.record_call();
        
        let raw_proposal = match EpistemicEngine::propose_action(&self.client, &self.api_key, &enhanced_mission, &h_content, world_context, &evidence_history).await {
            Ok(p) => p,
            Err(e) => {
                self.log(format!("\x1b[31m>> [STRATEGIST FAULT]\x1b[0m {}", e));
                return Ok(CortexState::Strategize);
            }
        };

        self.dump_debug(turn, "strategist_output", &raw_proposal);
        self.log(format!("\x1b[35m>> [STRATEGIST PROPOSAL]:\x1b[0m {}", raw_proposal));

        // 3. Immediately Extract Typed Actions from Proposal
        let mut actions = crate::json_parser::extract_actions(&raw_proposal).unwrap_or_default();
        if actions.is_empty() {
            if let Some(intercepted) = crate::action_intercept::intercept_fallback_action(&raw_proposal) {
                actions.push(intercepted);
            }
        }

        if actions.is_empty() {
            let prop_trim = raw_proposal.trim().to_lowercase();
            if prop_trim.contains("halt") {
                actions.push(AgentAction::Halt { reasoning: "Model declared task complete".to_string() });
            }
        }

        if actions.is_empty() {
            self.log("\x1b[31m>> [CORTEX FAULT] Could not extract valid action from proposal.\x1b[0m".to_string());
            return Ok(CortexState::Strategize);
        }

        // 4. Deterministic Halt Gate: If action is Halt and physical evidence exists, complete immediately
        let has_halt = actions.iter().any(|a| matches!(a, AgentAction::Halt { .. }));
        if has_halt {
            let has_recon = crate::evidence_markers::has_recon_marker(&session.local_evidence);
            let has_mutation = crate::evidence_markers::has_mutation_marker(&session.local_evidence);
            if status == ContractStatus::Satisfied || has_recon || has_mutation || current_belief >= 0.75 {
                self.log("\x1b[1;36m>> [CORTEX] DETERMINISTIC GUARD: Valid Halt with physical evidence. Task Complete.\x1b[0m".to_string());
                return Ok(CortexState::Complete { belief: 1.0, evidence: session.collect_evidence(h_id) });
            }
        }

        Ok(CortexState::Critique { actions, proposal: raw_proposal, belief: current_belief })
    }

    async fn handle_critic(
        &mut self,
        turn: u32,
        session: &mut EpistemicSession,
        mission: &str,
        world_context: &str,
        actions: Vec<AgentAction>,
        proposal: String,
        belief: f32,
        is_mutation_task: bool,
        is_write_task: bool,
    ) -> Result<CortexState> {
        let status = ContractEvaluator::evaluate(is_mutation_task, is_write_task, &session.local_evidence, belief, mission);
        if status == ContractStatus::Satisfied {
            self.log("\x1b[1;36m>> [SUBGOAL_COMPLETE] Evidence is physically complete. Forcing completion.\x1b[0m".to_string());
            return Ok(CortexState::Complete { belief: 1.0, evidence: session.collect_evidence(session.best_hypothesis_id.unwrap_or_default()) });
        }
        
        let h_content = if let Some(h_id) = session.best_hypothesis_id {
            session.graph.nodes.get(&h_id).map(|n| n.content.clone()).unwrap_or_default()
        } else {
            String::new()
        };

        // Check Anti-Loop on actions before calling Critic
        for action in &actions {
            let action_key = match action {
                AgentAction::ReadFile { path, .. } => format!("read_file:{}", path),
                AgentAction::ReadFileChunk { path, start_line, .. } => format!("read_file_chunk:{}:{}", path, start_line),
                AgentAction::SearchCode { path, query, .. } => format!("search_code:{}:{}", path, query),
                AgentAction::WriteFile { path, .. } => format!("write_file:{}", path),
                AgentAction::DiffPatch { path, .. } => format!("diff_patch:{}", path),
                AgentAction::PatchFunction { path, function_name, .. } => format!("patch_function:{}:{}", path, function_name),
                AgentAction::SynthesizeReport { output_path, .. } => format!("synthesize_report:{}", output_path),
                AgentAction::ExecuteCommand { command, .. } => format!("execute_command:{}", command.trim()),
                _ => format!("{:?}", action),
            };

            let attempt_count = session.global_action_attempt_count(&action_key);
            if attempt_count == 1 {
                self.log(format!("\x1b[33m>> [NOVELTY] Action [{}] already attempted once. Nudging belief.\x1b[0m", action_key));
                if let Some(h_id) = session.best_hypothesis_id {
                    let obs_id = session.graph.add_node(NodeKind::Observation, &format!("[LOW_NOVELTY] `{}` already attempted.", action_key), 0.85, 60);
                    session.graph.add_edge(obs_id, h_id, EdgeRelation::Contradicts, 0.35);
                    session.graph.propagate_beliefs();
                }
            }

            if attempt_count >= 3 {
                self.log(format!("\x1b[1;31m>> [ANTI-LOOP] Action [{}] attempted 3 times. FORCING PIVOT.\x1b[0m", action_key));
                if let Some(h_id) = session.best_hypothesis_id {
                    let obs_id = session.graph.add_node(NodeKind::Observation, "[LOOP_VETO] Repeated action blocked. Must use different tool.", 1.0, 95);
                    session.graph.add_edge(obs_id, h_id, EdgeRelation::Contradicts, 0.99);
                    session.graph.propagate_beliefs();
                }
                return Ok(CortexState::Strategize);
            }
            session.record_global_action_attempt(&action_key);
        }

        self.log("\x1b[31m>> [RED TEAM] Criticizing proposal...\x1b[0m".to_string());
        session.budget.record_call();

        match EpistemicEngine::criticize_proposal(&self.client, &self.api_key, mission, &h_content, &proposal, world_context, &session.local_evidence).await {
            Err(e) => {
                self.log(format!("\x1b[33m>> [CRITIC BYPASS] Parser error ({}). Proceeding with execution.\x1b[0m", e));
                Ok(CortexState::Execute { actions, proposal, belief })
            }
            Ok(critic) => {
                self.dump_debug(turn, "critic_output", &format!("{:?}", critic));
                self.log(format!("\x1b[31m>> [CRITIC] Verdict:\x1b[0m {} (Modifier: {:.2})", critic.critique, critic.confidence_modifier));

                if critic.confidence_modifier > -0.25 {
                    Ok(CortexState::Execute { actions, proposal, belief })
                } else {
                    self.log("\x1b[33m>> [DIALECTIC FEEDBACK] Veto registered in graph. Re-strategizing...\x1b[0m".to_string());
                    if let Some(h_id) = session.best_hypothesis_id {
                        let obs_id = session.graph.add_node(NodeKind::Observation, &format!("[CRITIC_VETO] {}", critic.critique), 1.0, 75);
                        session.graph.add_edge(obs_id, h_id, EdgeRelation::Contradicts, 0.40);
                        session.graph.propagate_beliefs();
                    }
                    Ok(CortexState::Strategize)
                }
            }
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
                AgentAction::WriteFile { .. } | AgentAction::DiffPatch { .. } | AgentAction::PatchFunction { .. } | AgentAction::SynthesizeReport { .. } => Capability::WriteSubstrate,
                AgentAction::Halt { .. } => Capability::HaltCognition,
                _ => Capability::ReadSubstrate,
            };

            if let AgentAction::AtomicPatch { patches, .. } = &action {
                for p in patches {
                    mutation_paths.push(p.path.clone());
                    if p.path.ends_with(".rs") {
                        rust_files_mutated = true;
                    }
                }
            }
            let mutation_path = match &action {
                AgentAction::WriteFile { path, .. } | AgentAction::DiffPatch { path, .. } | AgentAction::PatchFunction { path, .. } => Some(path.clone()),
                AgentAction::SynthesizeReport { output_path, .. } => Some(output_path.clone()),
                _ => None,
            };
            
            if let Some(path) = mutation_path {
                mutation_paths.push(path.clone());
                if path.ends_with(".rs") {
                    rust_files_mutated = true;
                }
            }

            if matches!(action, AgentAction::Halt { .. }) {
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

        // If an action failed with PATH_NOT_FOUND, prevent re-attempting that path
        if latest_evidence.contains("PATH_NOT_FOUND") {
            if let Some(h_id) = session.best_hypothesis_id {
                let obs_id = session.graph.add_node(
                    NodeKind::Observation,
                    "[PATH_DOES_NOT_EXIST] The requested path does not exist on disk. Do NOT attempt to read it again. Proceed directly to synthesis with the data you already have.",
                    1.0,
                    95,
                );
                session.graph.add_edge(obs_id, h_id, EdgeRelation::Contradicts, 0.90);
                session.graph.propagate_beliefs();
            }
        }

        if latest_evidence.trim().is_empty() {
            return Ok(CortexState::Strategize);
        }

        if rust_files_mutated {
            self.log("\x1b[36m>> [COMPILER CRITIC] Running cargo check to validate Rust mutation...\x1b[0m".to_string());
            
            let compile_output = tokio::process::Command::new("cargo")
                .arg("check")
                .output()
                .await;

            match compile_output {
                Ok(output) if output.status.success() => {
                    latest_evidence.push_str("\n[COMPILE_OK] Workspace compiles successfully.");
                    self.log("\x1b[32m>> [COMPILER CRITIC] Compile OK. Proceeding to LLM Evaluator.\x1b[0m".to_string());
                }
                Ok(output) => {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    let err_preview = stderr.lines().take(20).collect::<Vec<_>>().join("\n");
                    
                    self.log("\x1b[1;31m>> [COMPILER CRITIC] Compile FAILED. Reverting mutations.\x1b[0m".to_string());
                    let compile_fail_evidence = format!("[COMPILE_FAIL] Mutation failed compiler check:\n{}", err_preview);
                    
                    for path in &mutation_paths {
                        let _ = crate::podman::PodmanShield::execute(&format!("git checkout -- {}", path));
                    }

                    session.local_evidence.truncate(evidence_checkpoint);
                    session.local_evidence.push_str(&compile_fail_evidence);
                    session.local_evidence.push('\n');
                    
                    if let Some(h_id) = session.best_hypothesis_id {
                        let obs_id = session.graph.add_node(NodeKind::Observation, "[COMPILE_FAIL] Patch failed cargo check. Reverted.", 1.0, 90);
                        session.graph.add_edge(obs_id, h_id, EdgeRelation::Contradicts, 0.99);
                        session.graph.propagate_beliefs();
                    }
                    
                    return Ok(CortexState::Strategize);
                }
                Err(e) => {
                    self.log(format!("\x1b[33m>> [COMPILER CRITIC] Failed to run cargo check: {}. Proceeding.\x1b[0m", e));
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

    async fn handle_evaluate(
        &mut self,
        session: &mut EpistemicSession,
        _mission: &str,
        evidence: String,
        proposal: String,
        belief: f32,
        _is_mutation_task: bool,
        _is_write_task: bool,
        is_halt_action: bool,
        mutation_path: Option<String>,
    ) -> Result<CortexState> {
        self.log("\x1b[33m>> [CORTEX] Evaluating physical evidence against strategy...\x1b[0m".to_string());
        session.budget.record_call();
        
        let h_id = session.best_hypothesis_id.unwrap_or_default();
        let h_content = session
            .graph
            .nodes
            .get(&h_id)
            .map(|n| n.content.clone())
            .unwrap_or_default();
        let eval_evidence_context = format!("Current Action Output:\n{}\n\nPrior Evidence in Task:\n{}", evidence, session.local_evidence);

        match EpistemicEngine::evaluate_evidence(&self.client, &self.api_key, &h_content, &proposal, &eval_evidence_context).await {
            Err(e) => {
                self.log(format!("\x1b[31m>> [CORTEX FATAL] Evaluator panicked: {}\x1b[0m", e));
                Ok(CortexState::Strategize)
            }
            Ok(eval) => {
                let is_search_like = proposal.to_ascii_lowercase().contains("search_code")
                    || proposal.to_ascii_lowercase().contains("search_memory");
                if is_search_like {
                    let query_hint = proposal.chars().take(80).collect::<String>();
                    let rel = score_search_relevance(&query_hint, &evidence);
                    if !should_raise_belief(rel, 0.45) {
                        let mut eval = eval;
                        if eval.confidence_modifier > 0.0 {
                            eval.confidence_modifier = 0.0;
                        }
                        session.record_evidence(&evidence, &eval, h_id);
                        return Ok(CortexState::Strategize);
                    }
                }
                self.log(format!("\x1b[33m>> [EVALUATION]\x1b[0m {} (Modifier: {:.2})", eval.critique, eval.confidence_modifier));
                session.record_evidence(&evidence, &eval, h_id);

                if is_halt_action && eval.confidence_modifier >= 0.0 && belief > 0.70 {
                    self.log("\x1b[1;36m>> [CORTEX TERMINAL] Valid halt executed and verified. Node complete.\x1b[0m".to_string());
                    return Ok(CortexState::Complete { belief: belief.max(0.95), evidence: session.collect_evidence(h_id) });
                }

                if eval.confidence_modifier < 0.0 {
                    if let Some(path) = &mutation_path {
                        let _ = crate::podman::PodmanShield::execute(&format!("git checkout -- {}", path));
                        session.local_evidence = session.local_evidence.replace(&evidence, "");
                    }
                }

                if let Some(updated_node) = session.graph.nodes.get(&h_id) {
                    self.log(format!("\x1b[1;32m>> [CORTEX] Strategy Belief updated to {:.2}\x1b[0m", updated_node.belief_score));
                }
                Ok(CortexState::Strategize)
            }
        }
    }
}
