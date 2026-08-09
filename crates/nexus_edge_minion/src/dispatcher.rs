use crate::api_gateway::{ApiGateway, AgentRequest, AgentResponse};
use crate::sandbox::LocalSandbox;
use std::error::Error;

pub struct HybridDispatcher {
    gateway: ApiGateway,
    cascade: Vec<String>,
}

impl HybridDispatcher {
    pub fn new(cascade: Vec<String>) -> Self {
        Self {
            gateway: ApiGateway::new(),
            cascade,
        }
    }

    pub fn process_task(
        &self,
        token_stream_summary: &str,
        prompt: &str,
        max_tokens: usize,
    ) -> Result<AgentResponse, Box<dyn Error>> {
        let mut enriched_prompt = format!(
            "[Local Context Summary: {}]\n\nTask Request: {}",
            token_stream_summary, prompt
        );

        let mut total_tokens = 0;
        let mut last_model;

        // Autonomous ReAct Loop: Max 3 iterations per task
        for _step in 1..=3 {
            let request = AgentRequest {
                prompt: enriched_prompt.clone(),
                temperature: 0.2,
                max_tokens,
            };

            let response = self.gateway.dispatch_cascade(request, &self.cascade)?;
            total_tokens += response.tokens_used;
            last_model = response.model_used.clone();

            let mut command_to_run: Option<String> = None;

            // Interceptor 1: Custom XML tags (<execute>...</execute>)
            if let (Some(start), Some(end)) = (response.text.find("<execute>"), response.text.find("</execute>")) {
                let cmd_start = start + 9;
                if cmd_start < end {
                    command_to_run = Some(response.text[cmd_start..end].trim().to_string());
                }
            } 
            // Interceptor 2: Natural Markdown code blocks (```bash ... ``` or ```sh ... ```)
            else if let Some(start_idx) = response.text.find("```bash").or_else(|| response.text.find("```sh")) {
                let block_start = response.text[start_idx..].find('\n').map(|i| start_idx + i + 1);
                if let Some(content_start) = block_start {
                    // Corrected to use Rust range syntax (..)
                    if let Some(end_idx) = response.text[content_start..].find("```") {
                        let cmd = response.text[content_start..content_start + end_idx].trim().to_string();
                        if !cmd.is_empty() {
                            command_to_run = Some(cmd);
                        }
                    }
                }
            }

            // If a command was successfully intercepted, execute it in the sandbox
            if let Some(cmd) = command_to_run {
                let host_output = LocalSandbox::execute(&cmd);
                
                enriched_prompt.push_str(&format!(
                    "\n\n[Agent Attempted Command]: {}\n[Sandbox Host Output]:\n{}\n\n[Directive]: Based on this real host output, provide the final summarized answer.",
                    cmd, host_output
                ));
                continue;
            }

            // If no execution block is found, the agent has concluded its reasoning
            return Ok(AgentResponse {
                text: response.text,
                tokens_used: total_tokens,
                model_used: last_model,
            });
        }

        Err("Task failed: Exceeded maximum ReAct sandbox iterations (3).".into())
    }
}
