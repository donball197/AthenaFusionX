use tokio::sync::mpsc;
use tokio::task::JoinHandle;

#[derive(Debug, Clone)]
pub enum MemoryEvent {
    Store(String),
    Recall(String),
    Shutdown,
}

pub struct MemoryActor {
    tx: mpsc::Sender<MemoryEvent>,
}

impl MemoryActor {
    pub fn start() -> (Self, JoinHandle<()>) {
        let (tx, mut rx) = mpsc::channel::<MemoryEvent>(64);

        let handle = tokio::spawn(async move {
            while let Some(event) = rx.recv().await {
                match event {
                    MemoryEvent::Store(data) => {
                        println!("[MEMORY] storing: {}", data);
                    }

                    MemoryEvent::Recall(query) => {
                        println!("[MEMORY] recalling: {}", query);
                    }

                    MemoryEvent::Shutdown => {
                        println!("[MEMORY] shutdown");
                        break;
                    }
                }
            }
        });

        (Self { tx }, handle)
    }

    pub async fn send(
        &self,
        event: MemoryEvent,
    ) -> Result<(), mpsc::error::SendError<MemoryEvent>> {
        self.tx.send(event).await
    }
}
