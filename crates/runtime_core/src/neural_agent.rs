use serde_json::Value;
use std::collections::VecDeque;
use std::sync::Mutex;
use tokio::sync::broadcast;

#[derive(Debug, Default, Clone)]
pub struct ContextPacket {
    pub system_prompt: Option<String>,
    pub model_override: Option<String>,
    pub current_file: String,
    pub current_content: Option<String>,
    pub recent_logs: Vec<String>,
    pub task_complexity: u8,
}

pub struct MicroAgent {
    pub memory: Mutex<VecDeque<Value>>,
    pub tx: broadcast::Sender<String>,
}

impl MicroAgent {
    pub fn init(tx: broadcast::Sender<String>) -> anyhow::Result<Self> {
        Ok(MicroAgent { memory: Mutex::new(VecDeque::with_capacity(16)), tx })
    }
}
