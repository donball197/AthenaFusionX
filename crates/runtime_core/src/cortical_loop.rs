use anyhow::Result;
use reqwest::Client;
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};
use tokio::time::{sleep, Duration};

use crate::action_gate::ActionGate;
use crate::agent_protocol::AgentAction;
use crate::cognition::epistemic_session::EpistemicSession;
use crate::cognition_graph::{EdgeRelation, NodeKind};
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
    Proven { belief: f32, evidence: String },
    TurnsExhausted { best_belief: f32, evidence: String },
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
                        let workspace_root = meta
                            .get("workspace_root")
                            .and_then(|v| v.as_str())
                            .unwrap_or("<unresolved>");
                        let target_dir = meta
                            .get("target_directory")
                            .and_then(|v| v.as_str())
                            .unwrap_or("<unresolved>");
                        format!(
                            "WORKSPACE FACTS (ground truth, do not guess): workspace_root={workspace_root} | target_dir={target_dir}."
                        )
                    }
                    Err(e) => format!("WORKSPACE FACTS: cargo metadata parse failed ({e})."),
                }
            }
            Ok(output) => {
                format!("WORKSPACE FACTS: cargo metadata exited non-zero ({}).", output.status)
            }
            Err(e) => format!("WORKSPACE FACTS: cargo metadata failed to run ({e})."),
        }
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
        let world_context = context_chunks.join("\n");

        let mut session = EpistemicSession::new(mission);

        self.log("\x1b[36m>> [CORTEX] Generating competing strategies...\x1b[0m".to_string());
        session.budget.record_call();
        let hypotheses = EpistemicEngine::generate_hypotheses(
            &self.client,
            &self.api_key,
            mission,
            &world_context,
        )
        .await?;

        for h in hypotheses {
            session.graph.add_node(NodeKind::Hypothesis, &h, 0.5, 100);
            self.log(format!("\x1b[90m  -> Strategy: {}\x1b[0m", h));
        }

        let is_mutation_task = session.contract.is_mutation_task;
        let is_write_task = is_mutation_task
            && (session.contract.objective.to_lowercase().contains("write")
                || session.contract.objective.to_lowercase().contains("export"));

        for turn in 1..=session.budget.max_turns {
            self.log(format!(
                "\n\x1b[1;33m>> [CORTEX] Cognitive Turn {} / {}\x1b[0m",
                turn, session.budget.max_turns
            ));

            if let Some(best_node) = session.graph.select_best_hypothesis().cloned() {
                let h_id = best_node.id;
                let h_content = best_node.content.clone();
                let current_belief = best_node.belief_score;
                if current_belief > session.max_observed_belief {
                    session.max_observed_belief = current_belief;
                    session.best_hypothesis_id = Some(h_id);
                }
                self.log(format!(
                    "\x1b[32m>> [CORTEX] Exploring Top Strategy (Belief: {:.2}):\x1b[0m {}",
                    current_belief, h_content
                ));

                let evidence_history = session.collect_evidence(h_id);

                // ⚡ LOCAL DETERMINISTIC BACKSTOP
                if is_write_task
                    && session.local_evidence.lines().any(|l| l.starts_with("[WRITE_OK]"))
                {
                    self.log("\x1b[1;36m>> [CORTEX] DETERMINISTIC GUARD: Write objective satisfied with local [WRITE_OK]. Halting Node successfully.\x1b[0m".to_string());
                    return Ok(CortexOutcome::Proven {
                        belief: 1.0,
                        evidence: session.collect_evidence(h_id),
                    });
                } else if is_mutation_task && session.is_subgoal_satisfied(current_belief) {
                    self.log("\x1b[1;36m>> [CORTEX] DETERMINISTIC GUARD: Patch objective satisfied with verified mutation contract. Halting Node successfully.\x1b[0m".to_string());
                    return Ok(CortexOutcome::Proven {
                        belief: 1.0,
                        evidence: session.collect_evidence(h_id),
                    });
                } else if !is_mutation_task
                    && !is_write_task
                    && current_belief >= 0.85
                    && (session.local_evidence.contains("[READ_CHUNK_OK]")
                        || session.local_evidence.contains("[READ_OK]"))
                {
                    self.log("\x1b[1;36m>> [CORTEX] DETERMINISTIC GUARD: Search/Audit objective satisfied with valid read evidence. Halting Node successfully.\x1b[0m".to_string());
                    return Ok(CortexOutcome::Proven {
                        belief: current_belief,
                        evidence: session.collect_evidence(h_id),
                    });
                }

                // ⚡ STRICT BUDGET EXHAUSTION GUARD (PHASE 8C)
                if session.budget.is_exhausted(turn) {
                    self.log("\x1b[1;31m>> [SUBGOAL_BUDGET_EXHAUSTED] Cognitive call limit reached before contract satisfaction. Node failed.\x1b[0m".to_string());
                    return Ok(CortexOutcome::TurnsExhausted {
                        best_belief: current_belief,
                        evidence: session.collect_evidence(h_id),
                    });
                }

                self.log("\x1b[36m>> [STRATEGIST] Planning next maneuver...\x1b[0m".to_string());
                session.budget.record_call();
                let proposed_action = match EpistemicEngine::propose_action(
                    &self.client,
                    &self.api_key,
                    mission,
                    &h_content,
                    &world_context,
                    &evidence_history,
                )
                .await
                {
                    Ok(action) => action,
                    Err(e) => {
                        self.log(format!("\x1b[31m>> [STRATEGIST FAULT]\x1b[0m {}", e));
                        continue;
                    }
                };
                self.log(format!("\x1b[35m>> [STRATEGIST PROPOSAL]:\x1b[0m {}", proposed_action));

                // ⚡ DETERMINISTIC HALT SHORT-CIRCUIT
                let prop_lower = proposed_action.trim().to_lowercase();
                if prop_lower == "halt" || prop_lower.starts_with("halt ") {
                    let has_prior_evidence = if is_write_task {
                        session
                            .local_evidence
                            .lines()
                            .any(|l| l.starts_with("[WRITE_OK]") || l.starts_with("[CMD_OK]"))
                    } else if is_mutation_task {
                        session.local_evidence.lines().any(|l| {
                            l.starts_with("[DIFF_PATCH_OK]") || l.starts_with("[REPLACE_LINES_OK]")
                        }) || session
                            .local_evidence
                            .lines()
                            .any(|l| l.starts_with("[PATCH_FUNCTION_OK]"))
                            || session.local_evidence.lines().any(|l| l.starts_with("[WRITE_OK]"))
                    } else {
                        session.local_evidence.lines().any(|l| l.starts_with("[READ_CHUNK_OK]"))
                            || session
                                .local_evidence
                                .lines()
                                .any(|l| l.starts_with("[SEARCH_CODE_OK]"))
                            || session.local_evidence.lines().any(|l| l.starts_with("[READ_OK]"))
                            || session.local_evidence.lines().any(|l| l.starts_with("[WRITE_OK]"))
                            || session.local_evidence.lines().any(|l| l.starts_with("[CMD_OK]"))
                    };

                    let compound_satisfied = if is_mutation_task {
                        session.is_subgoal_satisfied(current_belief)
                    } else if is_write_task {
                        true
                    } else {
                        let mission_lower = mission.to_lowercase();
                        let objective_part = mission_lower
                            .lines()
                            .find(|line| line.starts_with("objective:"))
                            .unwrap_or(mission_lower.as_str());
                        let evidence_lower = format!(
                            "{} {}",
                            evidence_history.to_lowercase(),
                            session.local_evidence.to_lowercase()
                        );
                        let and_segments: Vec<&str> = objective_part
                            .split(" and ")
                            .map(|s| s.trim())
                            .filter(|s| !s.is_empty())
                            .collect();
                        and_segments.iter().all(|segment| {
                            let or_parts: Vec<&str> = segment
                                .split(" or ")
                                .map(|s| s.trim())
                                .filter(|s| !s.is_empty())
                                .collect();
                            if or_parts.len() <= 1 {
                                segment
                                    .split_whitespace()
                                    .filter(|w| w.len() > 3)
                                    .any(|w| evidence_lower.contains(w))
                            } else {
                                or_parts.iter().any(|part| {
                                    part.split_whitespace()
                                        .filter(|w| w.len() > 3)
                                        .any(|w| evidence_lower.contains(w))
                                })
                            }
                        })
                    };

                    if has_prior_evidence && compound_satisfied {
                        self.log("\x1b[1;36m>> [CORTEX] DETERMINISTIC GUARD: Halt proposed following successful local evidence. Subtask complete.\x1b[0m".to_string());
                        return Ok(CortexOutcome::Proven {
                            belief: 1.0,
                            evidence: session.collect_evidence(h_id),
                        });
                    } else if has_prior_evidence && !compound_satisfied {
                        self.log("\x1b[1;33m>> [CORTEX] DETERMINISTIC GUARD SUPPRESSED: Halt proposed but evidence covers only part of a compound objective; continuing to explore.\x1b[0m".to_string());
                    } else if (is_mutation_task || is_write_task) && !has_prior_evidence {
                        self.log("\x1b[1;31m>> [CORTEX FAULT] Halt rejected: Objective requires patching/writing, but no mutation success marker found.\x1b[0m".to_string());
                        let obs_id = session.graph.add_node(NodeKind::Observation, "[HALT_REJECTED] The directive requires modifying files. You proposed halt without successfully patching anything. You MUST use diff_patch, patch_function, or write_file.", 1.0, 50);
                        session.graph.add_edge(obs_id, h_id, EdgeRelation::Contradicts, 0.6);
                        session.graph.propagate_beliefs();
                        continue;
                    }
                }

                // --- ANTI-LOOP KINETIC GUARD (PATCHED) ---
                let action_key = proposed_action.trim().to_string();
                if session.is_action_attempted(&action_key) {
                    if session.is_subgoal_satisfied(current_belief)
                        || (!session.local_evidence.trim().is_empty() && current_belief >= 0.60)
                    {
                        self.log("\x1b[1;36m>> [SUBGOAL_COMPLETE] Loop detected but evidence is sufficient. Halting.\x1b[0m".to_string());
                        return Ok(CortexOutcome::Proven {
                            belief: current_belief,
                            evidence: session.collect_evidence(h_id),
                        });
                    } else {
                        self.log(format!("\x1b[1;31m>> [ENTERPRISE CORTEX] Anti-Loop Guard Triggered: Action [{}] already attempted.\x1b[0m", action_key));
                        let obs_id = session.graph.add_node(
                            NodeKind::Observation,
                            "[CRITICAL VETO] You are repeating a failed action! YOU MUST MUTATE YOUR STRATEGY, USE A DIFFERENT TOOL, OR HALT.",
                            1.0, 95
                        );
                        session.graph.add_edge(obs_id, h_id, EdgeRelation::Contradicts, 0.99);
                        session.graph.propagate_beliefs();
                        continue;
                    }
                }
                session.record_action_attempt(&action_key);

                self.log("\x1b[31m>> [RED TEAM] Criticizing proposal...\x1b[0m".to_string());
                session.budget.record_call();
                match EpistemicEngine::criticize_proposal(
                    &self.client,
                    &self.api_key,
                    mission,
                    &h_content,
                    &proposed_action,
                    &world_context,
                    &evidence_history,
                )
                .await
                {
                    Err(e) => {
                        self.log(format!(
                            "\x1b[31m>> [CORTEX FATAL] Red Team critic parser panicked: {}\x1b[0m",
                            e
                        ));
                        let evidence = format!(
                            "[CRITIC_PARSE_FAILURE] Critic response could not be parsed: {}",
                            e
                        );
                        let obs_id =
                            session.graph.add_node(NodeKind::Observation, &evidence, 1.0, 50);
                        session.graph.add_edge(obs_id, h_id, EdgeRelation::Contradicts, 0.15);
                        session.graph.propagate_beliefs();
                    }
                    Ok(critic) => {
                        self.log(format!(
                            "\x1b[31m>> [CRITIC] Verdict:\x1b[0m {} (Modifier: {:.2})",
                            critic.critique, critic.confidence_modifier
                        ));

                        if critic.confidence_modifier > -0.2 {
                            self.log("\x1b[36m>> [CORTEX] Translating intent to physical action...\x1b[0m".to_string());
                            session.budget.record_call();

                            let parsed_actions = if let Ok(direct_actions) =
                                crate::json_parser::extract_actions(&proposed_action)
                            {
                                if !direct_actions.is_empty() {
                                    self.log("\x1b[32m>> [CORTEX INTERCEPT] Direct JSON Proposal Intercept: Bypassing Translator LLM.\x1b[0m".to_string());
                                    Ok(direct_actions)
                                } else {
                                    if let Ok(raw_json) = EpistemicEngine::translate_to_action(
                                        &self.client,
                                        &self.api_key,
                                        &h_content,
                                        &proposed_action,
                                    )
                                    .await
                                    {
                                        crate::json_parser::extract_actions(&raw_json)
                                    } else {
                                        Ok(vec![])
                                    }
                                }
                            } else {
                                if let Ok(raw_json) = EpistemicEngine::translate_to_action(
                                    &self.client,
                                    &self.api_key,
                                    &h_content,
                                    &proposed_action,
                                )
                                .await
                                {
                                    crate::json_parser::extract_actions(&raw_json)
                                } else {
                                    Ok(vec![])
                                }
                            };

                            if let Ok(mut actions) = parsed_actions {
                                if actions.is_empty() && !proposed_action.trim().is_empty() {
                                    let action_str = proposed_action.trim();
                                    if let Some(intercepted) =
                                        crate::action_intercept::intercept_fallback_action(
                                            action_str,
                                        )
                                    {
                                        self.log("\x1b[32m>> [CORTEX INTERCEPT] Native Tool Intercept: Handling action directly.\x1b[0m".to_string());
                                        actions.push(intercepted);
                                    } else {
                                        self.log("\x1b[33m>> [CORTEX FALLBACK] Translator produced empty JSON. Executing proposed command directly...\x1b[0m".to_string());
                                        actions.push(AgentAction::ExecuteCommand {
                                            command: proposed_action.clone(),
                                            reasoning: "Fallback raw command execution".to_string(),
                                            confidence: 0.8,
                                        });
                                    }
                                }

                                if actions.is_empty() {
                                    self.log("\x1b[31m>> [CORTEX FAULT] Translator produced an empty action list.\x1b[0m".to_string());
                                    let obs_id = session.graph.add_node(
                                        NodeKind::Observation,
                                        "[TRANSLATION_EMPTY]",
                                        1.0,
                                        50,
                                    );
                                    session.graph.add_edge(
                                        obs_id,
                                        h_id,
                                        EdgeRelation::Contradicts,
                                        0.15,
                                    );
                                    session.graph.propagate_beliefs();
                                } else {
                                    let mut executor =
                                        RoboticExecutor::new(self.api_key.clone(), self.tx.clone());

                                    for action in actions {
                                        let intent =
                                            IntentContract::new(&h_content, current_belief);
                                        let capability = match &action {
                                            AgentAction::ExecuteCommand { .. } => {
                                                Capability::ExecuteSystemCall
                                            }
                                            AgentAction::WriteFile { .. }
                                            | AgentAction::DiffPatch { .. }
                                            | AgentAction::PatchFunction { .. } => {
                                                Capability::WriteSubstrate
                                            }
                                            AgentAction::Halt { .. } => Capability::HaltCognition,
                                            _ => Capability::ReadSubstrate,
                                        };

                                        // Capture the mutation target path now -- `action` is moved
                                        // into MotorCortex::translate() below and won't be readable
                                        // after this point, but we need the path later to auto-revert
                                        // if the Evaluator scores this mutation negative.
                                        let mutation_path: Option<String> = match &action {
                                            AgentAction::WriteFile { path, .. }
                                            | AgentAction::DiffPatch { path, .. }
                                            | AgentAction::PatchFunction { path, .. } => {
                                                Some(path.clone())
                                            }
                                            _ => None,
                                        };

                                        let motor_cmd = MotorCortex::translate(
                                            &intent,
                                            None,
                                            action,
                                            capability,
                                            h_content.clone(),
                                        );

                                        if let Err(gate_err) = self.action_gate.approve(&motor_cmd)
                                        {
                                            self.log(format!(
                                                "\x1b[1;31m>> [ACTION GATE REJECTED]: {}\x1b[0m",
                                                gate_err
                                            ));
                                            let obs_id = session.graph.add_node(
                                                NodeKind::Observation,
                                                &format!("[ACTION_GATE_BLOCKED] {}", gate_err),
                                                1.0,
                                                50,
                                            );
                                            session.graph.add_edge(
                                                obs_id,
                                                h_id,
                                                EdgeRelation::Contradicts,
                                                0.50,
                                            );
                                            session.graph.propagate_beliefs();
                                            continue;
                                        }

                                        self.log("\x1b[35m>> [MOTOR CORTEX] Emitting authorized MotorCommand to RoboticExecutor...\x1b[0m".to_string());
                                        let evidence = match executor
                                            .execute_motor_command(
                                                motor_cmd,
                                                self.memory.clone(),
                                                self.vmo_state.clone(),
                                                self.dist.clone(),
                                            )
                                            .await
                                        {
                                            Ok(ev) => ev,
                                            Err(e) => {
                                                format!("[ACTION FAILED] OS rejected action: {}", e)
                                            }
                                        };

                                        session.local_evidence.push_str(&evidence);
                                        session.local_evidence.push('\n');

                                        let preview =
                                            evidence.lines().take(5).collect::<Vec<_>>().join("\n");
                                        self.log(format!(
                                            "\x1b[32m>> [EVIDENCE ACQUIRED]\x1b[0m\n{}",
                                            preview
                                        ));

                                        self.log("\x1b[33m>> [CORTEX] Evaluating physical evidence against strategy...\x1b[0m".to_string());
                                        let eval_evidence_context = format!("Current Action Output:\n{}\n\nPrior Evidence in Task:\n{}", evidence, evidence_history);
                                        session.budget.record_call();
                                        match EpistemicEngine::evaluate_evidence(
                                            &self.client,
                                            &self.api_key,
                                            &h_content,
                                            &proposed_action,
                                            &eval_evidence_context,
                                        )
                                        .await
                                        {
                                            Err(e) => {
                                                self.log(format!("\x1b[31m>> [CORTEX FATAL] Evaluator panicked: {}\x1b[0m", e));
                                            }
                                            Ok(eval) => {
                                                self.log(format!(
                                                    "\x1b[33m>> [EVALUATION]\x1b[0m {} (Modifier: {:.2})",
                                                    eval.critique, eval.confidence_modifier
                                                ));

                                                session.record_evidence(&evidence, &eval, h_id);

                                                // AUTO-REVERT: a mutation the Evaluator scored negative must
                                                // not be allowed to stand as satisfied evidence. Revert the
                                                // file on disk and strip the raw marker from local_evidence so
                                                // completion contracts correctly see this as unresolved instead
                                                // of "patch succeeded technically" being treated as "objective
                                                // accomplished."
                                                if eval.confidence_modifier < 0.0 {
                                                    if let Some(path) = &mutation_path {
                                                        self.log(format!("\x1b[1;31m>> [CORTEX SAFETY] Mutation to '{}' scored negative ({:.2}) -- reverting via git checkout.\x1b[0m", path, eval.confidence_modifier));
                                                        let _ =
                                                            crate::podman::PodmanShield::execute(
                                                                &format!(
                                                                    "git checkout -- {}",
                                                                    path
                                                                ),
                                                            );
                                                        session.local_evidence = session
                                                            .local_evidence
                                                            .replace(&evidence, "");
                                                        let reverted_tag = format!(
                                                            "[MUTATION_REVERTED] {} -- Evaluator rejected this mutation ({:.2}): {}\n",
                                                            path, eval.confidence_modifier, eval.critique
                                                        );
                                                        session
                                                            .local_evidence
                                                            .push_str(&reverted_tag);
                                                    }
                                                }

                                                if let Some(updated_node) =
                                                    session.graph.nodes.get(&h_id)
                                                {
                                                    self.log(format!("\x1b[1;32m>> [CORTEX] Strategy Belief updated to {:.2}\x1b[0m", updated_node.belief_score));

                                                    if session.is_subgoal_satisfied(
                                                        updated_node.belief_score,
                                                    ) {
                                                        self.log("\x1b[1;36m>> [SUBGOAL_COMPLETE] Subgoal contract satisfied. Returning control.\x1b[0m".to_string());
                                                        return Ok(CortexOutcome::Proven {
                                                            belief: 1.0,
                                                            evidence: session
                                                                .collect_evidence(h_id),
                                                        });
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        } else {
                            self.log(
                                "\x1b[31m>> [RED TEAM] VETO. Proposal rejected.\x1b[0m".to_string(),
                            );
                            let veto_msg = format!(
                                "[RED TEAM VETO] Action blocked. Critic feedback: {}",
                                critic.critique
                            );
                            let veto_obs_id =
                                session.graph.add_node(NodeKind::Observation, &veto_msg, 1.0, 50);
                            session.graph.add_edge(
                                veto_obs_id,
                                h_id,
                                EdgeRelation::Contradicts,
                                critic.confidence_modifier.abs(),
                            );
                            session.graph.propagate_beliefs();
                        }
                    }
                }
            } else {
                self.log(
                    "\x1b[1;35m>> [CORTEX] All strategies exhausted. Re-orienting graph...\x1b[0m"
                        .to_string(),
                );
                let mut reorient_context = world_context.clone();
                if !session.executed_actions.is_empty() {
                    reorient_context.push_str("\n[ALREADY ATTEMPTED ACTIONS -- DO NOT REPEAT]\n");
                    for a in &session.executed_actions {
                        reorient_context.push_str(&format!("- {}\n", a));
                    }
                }
                session.budget.record_call();
                let new_hypotheses = EpistemicEngine::generate_hypotheses(
                    &self.client,
                    &self.api_key,
                    mission,
                    &reorient_context,
                )
                .await;
                for h in new_hypotheses.unwrap_or_default() {
                    session.graph.add_node(NodeKind::Hypothesis, &h, 0.5, 100);
                    self.log(format!("\x1b[90m  -> Strategy: {}\x1b[0m", h));
                }
            }
            sleep(Duration::from_secs(2)).await;
        }
        self.log("\x1b[1;32m>> [CORTEX] Epistemic Session Complete.\x1b[0m".to_string());
        let final_evidence = session
            .best_hypothesis_id
            .map(|id| session.collect_evidence(id))
            .unwrap_or_else(|| "No evidence gathered.".to_string());

        if is_mutation_task
            && !session.local_evidence.contains("[DIFF_PATCH_OK]")
            && !session.local_evidence.contains("[PATCH_FUNCTION_OK]")
            && !session.local_evidence.contains("[WRITE_OK]")
        {
            self.log("\x1b[1;31m>> [CORTEX FAULT] Task exhausted budget without satisfying mutation constraints.\x1b[0m".to_string());
            return Ok(CortexOutcome::TurnsExhausted {
                best_belief: 0.0,
                evidence: final_evidence,
            });
        }

        Ok(CortexOutcome::Proven {
            belief: session.max_observed_belief.max(0.70),
            evidence: final_evidence,
        })
    }
}
