#![allow(dead_code)]

use crate::agent_protocol::AgentAction;
use crate::json_parser::extract_actions;
use crate::neural_agent::{ContextPacket, MicroAgent};
use anyhow::{Context, Result};
use serde_json::json;
use std::sync::Arc;
use tokio::sync::broadcast;
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
}

impl ReActAgent {
    pub fn new(
        micro_agent: Arc<MicroAgent>,
        client: reqwest::Client,
        api_key: String,
        tx: broadcast::Sender<String>,
    ) -> Self {
        Self { micro_agent, client, api_key, tx }
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
                let _ = self.tx.send(
                    json!({"type": "REACT_FINAL_ANSWER", "payload": {"answer": answer}})
                        .to_string(),
                );
                return Ok(answer);
            }

            match extract_actions(&response) {
                Ok(actions) if !actions.is_empty() => {
                    for action in actions {
                        history.push(ReActStep::Action(action.clone()));
                        let obs = self.execute_action(action, &ctx).await?;
                        history.push(ReActStep::Observation(obs.clone()));
                        let _ = self
                            .tx
                            .send(json!({"type": "REACT_OBSERVATION", "payload": obs}).to_string());
                    }
                }
                _ => warn!("No actions parsed in this ReAct turn"),
            }
        }

        Ok("Task reached maximum ReAct steps.".to_string())
    }

    fn build_react_prompt(
        &self,
        task: &str,
        history: &[ReActStep],
        _ctx: &ContextPacket,
    ) -> String {
        let mut p = format!("You are NEXUS ReAct Agent.\nTask: {}\n\n", task);
        p.push_str("Use Thought -> Action -> Observation format.\n");
        p.push_str(
            "CRITICAL: Your Action MUST be a valid JSON array matching exactly this schema:\n",
        );
        p.push_str(
            r#"
[
  { "type": "execute_command", "command": "ls -la", "reasoning": "..." },
  { "type": "read_file", "path": "...", "reasoning": "..." },
  { "type": "write_file", "path": "...", "content": "...", "reasoning": "..." }
]
"#,
        );

        if !history.is_empty() {
            p.push_str("\nHistory:\n");
            for step in history.iter().take(10) {
                match step {
                    ReActStep::Thought(t) => p.push_str(&format!("Thought: {}\n", t)),
                    ReActStep::Action(a) => p.push_str(&format!("Action: {:?}\n", a)),
                    ReActStep::Observation(o) => p.push_str(&format!("Observation: {}\n", o)),
                    ReActStep::FinalAnswer(a) => p.push_str(&format!("Final Answer: {}\n", a)),
                }
            }
        }

        p.push_str(
            "\nNext response format:\nThought: ...\nAction: [JSON array] OR Final Answer: ...",
        );
        p
    }

    async fn call_llm(&self, prompt: &str, ctx: &ContextPacket) -> Result<String> {
        let (response, _) = self
            .micro_agent
            .prepare_and_run(&self.client, &self.api_key, prompt, ctx.clone(), 0.0, None)
            .await
            .context("LLM call failed")?;

        Ok(response)
    }

    async fn execute_action(&self, action: AgentAction, _ctx: &ContextPacket) -> Result<String> {
        let _ = self.tx.send(
            json!({"type": "ACTION_EXECUTE", "payload": format!("{:?}", action)}).to_string(),
        );
        crate::executor::ToolExecutor::execute(&action).await
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
