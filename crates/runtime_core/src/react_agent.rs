#![allow(dead_code)] // Will be fully wired in next iterations

use crate::agent_protocol::AgentAction;
use crate::json_parser::extract_actions;
use crate::neural_agent::{ContextPacket, MicroAgent};
use crate::executor::RoboticExecutor;
use nexus_substrate::memory::MemoryPalace;
use crate::shared_state::VmoState;
use crate::distributed_scheduler::DistributedScheduler;

use anyhow::{Context, Result};
use serde_json::json;
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};
use tracing::{info, warn};

const MAX_REACT_STEPS: usize = 12;

#[derive(Debug, Clone)]
pub enum ReActStep {
    Thought(String),
    Action(AgentAction),
    Observation(String),
    FinalAnswer(String),
}

pub struct ReActAgent {
    micro_agent: Arc<MicroAgent>,
    client: reqwest::Client,
    api_key: String,
    tx: broadcast::Sender<String>,
    memory: Arc<Mutex<MemoryPalace>>,
    vmo_state: Arc<Mutex<VmoState>>,
    dist: Arc<DistributedScheduler>,
}

impl ReActAgent {
    pub fn new(
        micro_agent: Arc<MicroAgent>,
        client: reqwest::Client,
        api_key: String,
        tx: broadcast::Sender<String>,
        memory: Arc<Mutex<MemoryPalace>>,
        vmo_state: Arc<Mutex<VmoState>>,
        dist: Arc<DistributedScheduler>,
    ) -> Self {
        Self { micro_agent, client, api_key, tx, memory, vmo_state, dist }
    }

    pub async fn run(&self, task: &str, ctx: ContextPacket) -> Result<String> {
        let mut history: Vec<ReActStep> = Vec::new();
        let mut step = 0usize;

        let _ = self.tx.send(json!({"type": "REACT_START", "payload": {"task": task}}).to_string());

        while step < MAX_REACT_STEPS {
            step += 1;
            info!("🔄 ReAct Step {}/{}", step, MAX_REACT_STEPS);

            let prompt = self.build_react_prompt(task, &history, &ctx);
            let response = self.call_llm(&prompt, &ctx).await?;

            let thought = self.extract_thought(&response);
            let _ = self.tx.send(json!({"type": "NEURAL_THOUGHT", "payload": thought}).to_string());
            history.push(ReActStep::Thought(thought));

            if let Some(answer) = self.extract_final_answer(&response) {
                let _ = self.tx.send(json!({"type": "REACT_FINAL_ANSWER", "payload": {"answer": answer}}).to_string());
                return Ok(answer);
            }

            match extract_actions(&response) {
                Ok(actions) if !actions.is_empty() => {
                    for action in actions {
                        history.push(ReActStep::Action(action.clone()));
                        let obs = self.execute_action(action, &ctx).await?;
                        history.push(ReActStep::Observation(obs.clone()));
                        let _ = self.tx.send(json!({"type": "REACT_OBSERVATION", "payload": obs}).to_string());
                    }
                }
                _ => warn!("No actions parsed in this ReAct turn"),
            }
        }

        Ok("Task reached maximum ReAct steps.".to_string())
    }

    fn build_react_prompt(&self, task: &str, history: &[ReActStep], _ctx: &ContextPacket) -> String {
        let mut p = format!("You are NEXUS ReAct Agent.\nTask: {}\n\n", task);
        p.push_str("Use Thought → Action → Observation format.\n");
        p.push_str("Available actions: read_file, write_file, vmo_create, diff_patch, etc.\n\n");

        if !history.is_empty() {
            p.push_str("History:\n");
            for step in history.iter().take(10) {
                match step {
                    ReActStep::Thought(t) => p.push_str(&format!("Thought: {}\n", t)),
                    ReActStep::Action(a) => p.push_str(&format!("Action: {:?}\n", a)),
                    ReActStep::Observation(o) => p.push_str(&format!("Observation: {}\n", o)),
                    ReActStep::FinalAnswer(a) => p.push_str(&format!("Final Answer: {}\n", a)),
                }
            }
        }

        p.push_str("\nNext response format:\nThought: ...\nAction: [JSON array of AgentAction] OR Final Answer: ...");
        p
    }

    async fn call_llm(&self, prompt: &str, ctx: &ContextPacket) -> Result<String> {
        let (response, _) = self.micro_agent
            .prepare_and_run(&self.client, &self.api_key, prompt, ctx.clone(), 0.0, None)
            .await
            .context("LLM call failed")?;

        Ok(response)
    }

    async fn execute_action(&self, action: AgentAction, _ctx: &ContextPacket) -> Result<String> {
        let _ = self.tx.send(json!({"type": "ACTION_EXECUTE", "payload": format!("{:?}", action)}).to_string());
        
        // ⚡ Unification: ReAct Agent now uses the full RoboticExecutor 
        let mut executor = RoboticExecutor::new(self.api_key.clone(), self.tx.clone());
        executor.execute_single_action(action, self.memory.clone(), self.vmo_state.clone(), self.dist.clone()).await
    }

    fn extract_thought(&self, resp: &str) -> String {
        resp.lines()
            .find(|l| l.trim_start().starts_with("Thought:"))
            .map(|l| l.trim_start_matches("Thought:").trim().to_string())
            .unwrap_or_else(|| "Thinking step-by-step...".into())
    }

    fn extract_final_answer(&self, resp: &str) -> Option<String> {
        resp.lines()
            .find(|l| l.trim_start().starts_with("Final Answer:"))
            .map(|l| l.trim_start_matches("Final Answer:").trim().to_string())
    }
}
