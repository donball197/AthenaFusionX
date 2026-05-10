use tokio::sync::mpsc;
use crate::agent_protocol::{MemoryCommand, SubsystemEvent, SubsystemContext};
use nexus_substrate::memory::MemoryPalace;
use std::time::Duration;

pub struct MemoryActor {
    palace: MemoryPalace,
    mailbox: mpsc::Receiver<MemoryCommand>,
    ctx: SubsystemContext,
}

impl MemoryActor {
    pub async fn spawn(db_path: String, ctx: SubsystemContext) -> mpsc::Sender<MemoryCommand> {
        let (tx, rx) = mpsc::channel(64);
        let actor_ctx = ctx.clone();
        tokio::spawn(async move {
            let name = "MEMORY";
            let palace = MemoryPalace::new(&db_path).await.expect("DB_INIT_FAIL");
            let mut actor = Self { palace, mailbox: rx, ctx: actor_ctx };
            let mut heartbeat = tokio::time::interval(Duration::from_secs(2));
            loop {
                tokio::select! {
                    _ = actor.ctx.cancellation.cancelled() => break,
                    _ = heartbeat.tick() => { 
                        let _ = actor.ctx.event_tx.send(SubsystemEvent::Heartbeat(name)).await; 
                    }
                    Some(cmd) = actor.mailbox.recv() => {
                        match cmd {
                            MemoryCommand::Search { query, responder } => {
                                let res = actor.palace.search_memories(&query).await.unwrap_or_default();
                                let _ = responder.send(res);
                            }
                            MemoryCommand::Store { origin, directive, result } => {
                                let _ = actor.palace.store_memory(&origin, &directive, &result, None).await;
                            }
                        }
                    }
                }
            }
            let _ = actor.ctx.event_tx.send(SubsystemEvent::ActorExited { name, panic: false }).await;
        });
        tx
    }
}
