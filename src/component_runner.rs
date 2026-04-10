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


use crate::agent_protocol::AgentAction;
use crate::executor::Executor;
use crate::memory::MemoryPalace;
use anyhow::Result;
use bitflags::bitflags;
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc, oneshot, Mutex};

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Capabilities: u32 {
        const READ_FS   = 0b0000_0001;
        const WRITE_FS  = 0b0000_0010;
        const NETWORK   = 0b0000_0100;
        const SYSTEM_EXEC = 0b0001_0000;
        const ALL        = 0b1111_1111;
    }
}

pub enum ComponentMessage {
    ExecuteActions { actions: Vec<AgentAction>, reply: oneshot::Sender<Result<Vec<String>>> },
    Shutdown,
}

#[derive(Clone)]
pub struct AgentHandle {
    pub tx: mpsc::Sender<ComponentMessage>,
    pub id: String,
    pub capabilities: Capabilities,
}

impl AgentHandle {
    pub async fn execute_actions(&self, actions: Vec<AgentAction>) -> Result<Vec<String>> {
        let (reply_tx, reply_rx) = oneshot::channel();
        let _ = self.tx.send(ComponentMessage::ExecuteActions { actions, reply: reply_tx }).await;
        reply_rx.await?
    }
}

pub struct AgentComponent {
    pub id: String,
    pub agent: Arc<crate::neural_agent::MicroAgent>,
    pub mailbox: mpsc::Receiver<ComponentMessage>,
    pub self_tx: mpsc::Sender<ComponentMessage>,
    pub memory: Arc<Mutex<MemoryPalace>>,
}

impl AgentComponent {
    pub fn spawn(
        id: &str,
        _caps: Capabilities,
        agent: Arc<crate::neural_agent::MicroAgent>,
        _tx: broadcast::Sender<String>,
        memory: Arc<Mutex<MemoryPalace>>,
    ) -> AgentHandle {
        let (tx, rx) = mpsc::channel(32);
        let actor = Self { id: id.to_string(), agent, mailbox: rx, self_tx: tx.clone(), memory };
        tokio::spawn(actor.run());
        AgentHandle { tx, id: id.to_string(), capabilities: Capabilities::ALL }
    }

    async fn run(mut self) {
        while let Some(msg) = self.mailbox.recv().await {
            match msg {
                ComponentMessage::ExecuteActions { actions, reply } => {
                    let result =
                        Executor::execute_handle(&self.to_handle(), actions, self.memory.clone())
                            .await;
                    let _ = reply.send(result);
                }
                ComponentMessage::Shutdown => break,
            }
        }
    }

    fn to_handle(&self) -> AgentHandle {
        AgentHandle {
            tx: self.self_tx.clone(),
            id: self.id.clone(),
            capabilities: Capabilities::ALL,
        }
    }
}
