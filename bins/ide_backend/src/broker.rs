//! Broker — deduplicates rapid fire directives within a 200 ms window
//! so the same button-mash doesn't flood the agent pipeline.
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Instant;

static LAST_FIRE: Lazy<Mutex<HashMap<String, Instant>>> = Lazy::new(|| Mutex::new(HashMap::new()));

const DEBOUNCE_MS: u128 = 200;

pub fn init() {
    tracing::info!(">> [BROKER] Directive deduplicator online");
}

/// Returns true when the directive should be forwarded (not a duplicate).
pub fn should_forward(key: &str) -> bool {
    let mut map = LAST_FIRE.lock().unwrap();
    let now = Instant::now();
    if let Some(last) = map.get(key) {
        if last.elapsed().as_millis() < DEBOUNCE_MS {
            return false;
        }
    }
    map.insert(key.to_string(), now);
    true
}
