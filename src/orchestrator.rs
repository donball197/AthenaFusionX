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


// src/orchestrator.rs - v3.3 (Public Neural Link)
use crate::neural_agent::{ContextPacket, MicroAgent};
use anyhow::Result;
use std::sync::Arc;

pub struct Orchestrator {
    pub agent: Arc<MicroAgent>,
}

impl Orchestrator {
    pub fn new(agent: Arc<MicroAgent>) -> Self {
        Self { agent }
    }

    pub async fn run_autonomous_loop(
        &self,
        instruction: String,
        context: ContextPacket,
        client: &reqwest::Client,
        api_key: &str,
    ) -> Result<String> {
        let res: Result<(String, u128), anyhow::Error> =
            self.agent.prepare_and_run(client, api_key, &instruction, context, 1.0).await;

        match res {
            Ok((raw_response, _)) => Ok(raw_response),
            Err(e) => Err(anyhow::anyhow!("Orchestration Fault: {e}")),
        }
    }
}
