//! AthenaFusionX: Sovereign Unified Cognitive Core with Probabilistic Memory Learning

use anyhow::Result;
use reqwest::Client;
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};

use crate::action_gate::ActionGate;
use crate::agent_protocol::AgentAction;
use crate::distributed_scheduler::DistributedScheduler;
use crate::epistemic_engine::EpistemicEngine;
use crate::executor::RoboticExecutor;
use crate::intent_contract::IntentContract;
use crate::memory_cortex::MemoryCortex;
use crate::motor_cortex::MotorCortex;
use crate::policy::Capability;
use crate::probabilistic_memory::ProbabilisticMemory;
use crate::shared_state::VmoState;
use crate::world_model::WorldModel;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SovereignOutcome {
    pub directive: String,
    pub belief_score: f32,
    pub verified_proof: String,
    pub turns_taken: u32,
    pub is_successful: bool,
}

pub struct AthenaSovereignCore {
    client: Client,
    api_key: String,
    world_model: Arc<WorldModel>,
    memory: Arc<MemoryCortex>,
    vmo_state: Arc<Mutex<VmoState>>,
    dist: Arc<DistributedScheduler>,
    tx: broadcast::Sender<String>,
    action_gate: ActionGate,
}

impl AthenaSovereignCore {
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

    fn log(&self, msg: &str) {
        let _ = self.tx.send(
            serde_json::json!({
                "type": "NEURAL_STREAM",
                "payload": format!("{}\n", msg)
            })
            .to_string(),
        );
    }

    pub async fn execute_mission(&mut self, directive: &str) -> Result<SovereignOutcome> {
        self.log(&format!(
            "\x1b[1;36m>> [SOVEREIGN CORE] Ingesting Directive: {}\x1b[0m",
            directive
        ));

        // Step 1: Autonomous Perception & Probabilistic Memory Prior Extraction
        let context_chunks = self.world_model.semantic_query(directive).await;
        let mut world_context = context_chunks.join("\n");

        let (positive_priors, negative_constraints) =
            ProbabilisticMemory::retrieve_priors(&self.memory, &self.api_key, directive).await;

        if !positive_priors.is_empty() {
            self.log(&format!(
                "\x1b[1;32m>> [PROBABILISTIC MEMORY] Recalled {} high-fitness genome pattern(s) from VectorVault.\x1b[0m",
                positive_priors.len()
            ));
            world_context.push_str("\n\n[PROVEN HIGH-FITNESS EXEMPLARS]:\n");
            for p in &positive_priors {
                world_context.push_str(&format!(
                    "- Related Task: {}\n  Proof: {}\n",
                    p.directive, p.evidence
                ));
            }
        }

        if !negative_constraints.is_empty() {
            self.log(&format!(
                "\x1b[1;33m>> [PROBABILISTIC MEMORY] Activated {} inhibitory constraint(s) to avoid past failure traps.\x1b[0m",
                negative_constraints.len()
            ));
            world_context.push_str("\n\n[NEGATIVE CONSTRAINTS TO AVOID]:\n");
            for c in &negative_constraints {
                world_context.push_str(&format!("- {}\n", c));
            }
        }

        let mut turn = 0;
        let max_turns = 20;
        let mut current_belief = 0.5f32;
        let mut evidence_buffer = String::new();

        while turn < max_turns {
            turn += 1;
            self.log(&format!(
                "\n\x1b[1;33m>> [SOVEREIGN COGNITION] Atomic Turn {} / {}\x1b[0m",
                turn, max_turns
            ));

            // Step 2: Internalized Dialectic (Proposal Generation)
            let proposal = EpistemicEngine::propose_action(
                &self.client,
                &self.api_key,
                directive,
                "Unified Strategic Reflex",
                &world_context,
                &evidence_buffer,
            )
            .await?;

            self.log(&format!("\x1b[35m>> [INTENT GENERATED]:\x1b[0m {}", proposal));

            // Step 3: Self-Critique (Internal Adversarial Gate)
            let critic = EpistemicEngine::criticize_proposal(
                &self.client,
                &self.api_key,
                directive,
                "Unified Strategic Reflex",
                &proposal,
                &world_context,
                &evidence_buffer,
            )
            .await?;

            self.log(&format!(
                "\x1b[31m>> [INTERNAL CRITIC]:\x1b[0m {} (Modifier: {:.2})",
                critic.critique, critic.confidence_modifier
            ));

            if critic.confidence_modifier < -0.25 && turn < 15 {
                self.log("\x1b[33m>> [DIALECTIC MEMORY] Internal critic vetoed proposal. Piping critique into memory buffer...\x1b[0m");
                evidence_buffer.push_str(&format!("\n[INTERNAL_CRITIC_VETO] Proposal rejected: {}\nYou MUST adapt your next action.\n", critic.critique));
                continue;
            }

            // Step 4: Motor Execution via ActionGate Sandbox
            let actions = crate::json_parser::extract_actions(&proposal).unwrap_or_default();
            if actions.is_empty() {
                if proposal.to_lowercase().contains("halt") && current_belief >= 0.80 {
                    self.log("\x1b[1;32m>> [SOVEREIGN COMPLETION] Contract satisfied with high confidence. Finalizing mission.\x1b[0m");
                    break;
                }
                continue;
            }

            let mut executor = RoboticExecutor::new(self.api_key.clone(), self.tx.clone());
            let mut turn_evidence = String::new();
            let mut rust_mutated = false;

            for action in actions {
                let intent = IntentContract::new(&proposal, current_belief);
                let capability = match &action {
                    AgentAction::ExecuteCommand { .. } => Capability::ExecuteSystemCall,
                    AgentAction::WriteFile { .. }
                    | AgentAction::DiffPatch { .. }
                    | AgentAction::PatchFunction { .. } => Capability::WriteSubstrate,
                    AgentAction::Halt { .. } => Capability::HaltCognition,
                    _ => Capability::ReadSubstrate,
                };

                let motor_cmd =
                    MotorCortex::translate(&intent, None, action, capability, proposal.clone());

                if let Err(e) = self.action_gate.approve(&motor_cmd) {
                    self.log(&format!("\x1b[31m>> [ACTION GATE BLOCKED]: {}\x1b[0m", e));
                    continue;
                }

                let ev = executor
                    .execute_motor_command(
                        motor_cmd,
                        self.memory.clone(),
                        self.vmo_state.clone(),
                        self.dist.clone(),
                    )
                    .await
                    .unwrap_or_else(|err| format!("[ACTION_ERROR] {}", err));

                if ev.contains(".rs")
                    && (ev.contains("[WRITE_OK]") || ev.contains("[PATCH_FUNCTION_OK]"))
                {
                    rust_mutated = true;
                }

                turn_evidence.push_str(&ev);
                turn_evidence.push('\n');
            }

            // Step 5: Physical Compiler Gate (Zero Hallucination Anchor)
            if rust_mutated {
                self.log("\x1b[36m>> [PHYSICAL COMPILER GATE] Running cargo check verification...\x1b[0m");
                let check_status =
                    tokio::process::Command::new("cargo").arg("check").output().await;

                match check_status {
                    Ok(o) if o.status.success() => {
                        self.log("\x1b[32m>> [COMPILER GATE: PASS] 0 syntax/type errors.\x1b[0m");
                        turn_evidence.push_str("\n[COMPILE_OK] Verified clean compilation.");
                    }
                    _ => {
                        self.log(
                            "\x1b[31m>> [COMPILER GATE: FAIL] Reverting changes via git...\x1b[0m",
                        );
                        let _ = tokio::process::Command::new("git")
                            .args(["checkout", "--", "."])
                            .output()
                            .await;
                        turn_evidence
                            .push_str("\n[COMPILE_FAIL] Patch failed compiler check. Reverted.");
                        current_belief = (current_belief - 0.2).max(0.1);
                        continue;
                    }
                }
            }

            // Step 6: Evidence Integration & Bayesian Update
            evidence_buffer.push_str(&turn_evidence);
            let eval = EpistemicEngine::evaluate_evidence(
                &self.client,
                &self.api_key,
                directive,
                &proposal,
                &turn_evidence,
            )
            .await?;

            current_belief = (current_belief + eval.confidence_modifier).clamp(0.0, 1.0);
            self.log(&format!(
                "\x1b[1;32m>> [SOVEREIGN STATE] Belief Score: {:.2}\x1b[0m",
                current_belief
            ));

            if current_belief >= 0.88 {
                self.log("\x1b[1;36m>> [OMNI-GUARD CONVERGENCE] Epistemic threshold reached (>=0.88). Mission Successful.\x1b[0m");
                break;
            }
        }

        let outcome = SovereignOutcome {
            directive: directive.to_string(),
            belief_score: current_belief,
            verified_proof: evidence_buffer,
            turns_taken: turn,
            is_successful: current_belief >= 0.85,
        };

        // Step 7: Automated Learning Commitment into VectorVault
        let _ =
            ProbabilisticMemory::commit_learning(&self.memory, &self.api_key, directive, &outcome)
                .await;

        Ok(outcome)
    }
}
