use tokio::sync::{mpsc, oneshot};
use std::collections::HashMap;

pub enum MemoryCommand {
    Store { key: String, value: Vec<u8> },
    Load { key: String, respond_to: oneshot::Sender<Option<Vec<u8>>> },
}

pub struct MemoryActor {
    rx: mpsc::Receiver<MemoryCommand>,
}

impl MemoryActor {
    pub fn new(rx: mpsc::Receiver<MemoryCommand>) -> Self {
        Self { rx }
    }

    pub async fn run(mut self) {
        let mut db = HashMap::new();
        while let Some(cmd) = self.rx.recv().await {
            match cmd {
                MemoryCommand::Store { key, value } => { db.insert(key, value); }
                MemoryCommand::Load { key, respond_to } => {
                    let _ = respond_to.send(db.get(&key).cloned());
                }
            }
        }
    }
}
