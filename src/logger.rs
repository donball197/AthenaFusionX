use tracing::Subscriber;
use tracing_subscriber::prelude::*;
use tracing_subscriber::{Layer, EnvFilter};
use tokio::sync::broadcast;
use std::sync::Mutex;

pub struct NexusTraceLayer {
    pub tx: broadcast::Sender<String>,
}

impl<S: Subscriber> Layer<S> for NexusTraceLayer {
    fn on_event(&self, event: &tracing::Event<'_>, _ctx: tracing_subscriber::layer::Context<'_, S>) {
        let mut payload = String::new();
        let mut visitor = StringVisitor(&mut payload);
        event.record(&mut visitor);

        let level = *event.metadata().level();
        let target = event.metadata().target();
        let clean_payload = payload.trim_matches('"');

        let log_json = serde_json::json!({
            "type": "LOG_EVENT",
            "payload": format!("[{}] {}: {}", level, target, clean_payload)
        }).to_string();

        let _ = self.tx.send(log_json);
        
        if let Ok(mut guard) = TURN_BUFFER.lock() {
            if let Some(buffer) = guard.as_mut() {
                buffer.push(format!("[{}] {}", level, clean_payload));
            }
        }
    }
}

static TURN_BUFFER: Mutex<Option<Vec<String>>> = Mutex::new(None);

pub fn begin_turn_capture() {
    if let Ok(mut guard) = TURN_BUFFER.lock() {
        *guard = Some(Vec::new());
    }
}

pub fn end_turn_capture() -> Vec<String> {
    if let Ok(mut guard) = TURN_BUFFER.lock() {
        guard.take().unwrap_or_default()
    } else {
        Vec::new()
    }
}

struct StringVisitor<'a>(&'a mut String);
impl<'a> tracing::field::Visit for StringVisitor<'a> {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            self.0.push_str(&format!("{:?}", value));
        }
    }
}

pub fn init_tracing(tx: broadcast::Sender<String>) {
    let layer = NexusTraceLayer { tx };
    
    // ⚡ FIX: Silences the dependency noise
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,nexus_singularity=debug,sqlx=warn,hyper=off,neli=off"));

    tracing_subscriber::registry()
        .with(filter)
        .with(layer)
        .with(tracing_subscriber::fmt::layer().with_ansi(true))
        .init();
    
    tracing::info!(">> [TRACING] Sovereign Observability Linked.");
}
