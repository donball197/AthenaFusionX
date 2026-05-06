use notify_debouncer_full::{new_debouncer, notify::*, DebounceEventResult, Debouncer, NoCache};
use std::time::Duration;
use tracing::{info, warn, debug};
use tokio::sync::broadcast;

pub struct NexusWatcher;

impl NexusWatcher {
    /// ignite: Starts the OS-level event loop with Warp-style debouncing.
    /// ⚡ CONTAINMENT: Strictly bound to telemetry. Autonomous agent triggers are explicitly forbidden.
    pub fn ignite(tx: broadcast::Sender<String>) -> anyhow::Result<Debouncer<RecommendedWatcher, NoCache>> {
        let tx_clone = tx.clone();
        
        let mut debouncer = new_debouncer(Duration::from_millis(200), None, move |res: DebounceEventResult| {
            match res {
                Ok(events) => {
                    for event in events {
                        let paths: Vec<String> = event.event.paths.iter()
                            .map(|p| p.to_string_lossy().to_string()).collect();
                        
                        let msg = match event.event.kind {
                            EventKind::Modify(_) => format!("FS_MODIFIED: {:?}", paths),
                            EventKind::Create(_) => format!("FS_CREATED: {:?}", paths),
                            EventKind::Remove(_) => format!("FS_DELETED: {:?}", paths),
                            _ => continue,
                        };
                        
                        let _ = tx_clone.send(serde_json::json!({
                            "type": "LOG_EVENT",
                            "payload": msg
                        }).to_string());
                        debug!("{}", msg);
                    }
                }
                Err(e) => warn!("FS_WATCHER_ERROR: {:?}", e),
            }
        })?;

        debouncer.watch(std::path::Path::new("./src"), RecursiveMode::Recursive)?;
        info!("\x1b[36m>> [AUTONOMIC_WATCHER] OS-Level Hooks Active (Agent Dispatch DISABLED)\x1b[0m");
        Ok(debouncer)
    }
}
