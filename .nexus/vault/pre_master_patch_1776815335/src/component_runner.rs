use bitflags::bitflags;
use tokio::sync::mpsc;
use crate::agent_protocol::AgentAction;

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct Capabilities: u32 {
        const NONE = 0;
        const READ = 1 << 0;
        const WRITE = 1 << 1;
        const EXEC = 1 << 2;
        const NETWORK = 1 << 3;
        const ALL = Self::READ.bits() | Self::WRITE.bits() | Self::EXEC.bits() | Self::NETWORK.bits();
    }
}

pub struct AgentHandle {
    pub tx: mpsc::UnboundedSender<String>,
    pub id: String,
    pub capabilities: Capabilities,
}

impl AgentHandle {
    pub async fn execute_actions(&self, actions: Vec<AgentAction>) -> Result<Vec<String>, String> {
        let mut results = Vec::new();
        for action in actions {
            // Route the legacy action straight to the async loop
            let _ = self.tx.send(format!("{:?}", action));
            results.push("ACTION_ROUTED_TO_CORTEX".to_string());
        }
        Ok(results)
    }
}

pub struct AgentComponent;

impl AgentComponent {
    pub fn spawn(id: &str, _caps: Capabilities) -> AgentHandle {
        let (tx, mut rx) = mpsc::unbounded_channel::<String>();
        let agent_id = id.to_string();

        tokio::spawn(async move {
            while let Some(msg) = rx.recv().await {
                println!("\x1b[36m>> [MICRO-AGENT {}] Directive received: {}\x1b[0m", agent_id, msg);
            }
        });

        AgentHandle { tx, id: id.to_string(), capabilities: Capabilities::ALL }
    }

    // Generic wrapper to swallow the 3 extra legacy arguments
    pub fn spawn_legacy<A, T, M>(id: &str, caps: Capabilities, _agent: A, _tx: T, _memory: M) -> AgentHandle {
        Self::spawn(id, caps)
    }
}
