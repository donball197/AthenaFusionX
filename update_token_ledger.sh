#!/bin/bash
echo "=== Updating Token Ledger for Gemma 4 models ==="

cat << 'NEWLEDGER' > crates/nexus_substrate/src/token_ledger.rs
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime};

#[derive(Debug, Clone)]
pub struct ModelQuota {
    pub rpm_limit: u32,
    pub tpm_limit: u32,
}

impl ModelQuota {
    fn for_model(model: &str) -> Self {
        let m = model.to_lowercase();

        // Gemma 4 Series — High reasoning models
        if m.contains("gemma-4-31b") || m.contains("gemma-4-31b-it") {
            Self { rpm_limit: 20, tpm_limit: 120_000 }   // Strong but respectful
        }
        else if m.contains("gemma-4-26b") || m.contains("gemma-4-26b-a4b") {
            Self { rpm_limit: 18, tpm_limit: 100_000 }
        }
        // Gemini 2.5 / 2.0 Flash — Fast tier
        else if m.contains("2.5-flash") || m.contains("2.0-flash") || m.contains("flash") {
            Self { rpm_limit: 30, tpm_limit: 300_000 }
        }
        // Gemini Pro / Heavy models
        else if m.contains("2.5-pro") || m.contains("3.") || m.contains("pro") {
            Self { rpm_limit: 15, tpm_limit: 150_000 }
        }
        // Embedding models
        else if m.contains("embedding") {
            Self { rpm_limit: 40, tpm_limit: 80_000 }
        }
        // Universal fallback (conservative)
        else {
            Self { rpm_limit: 12, tpm_limit: 80_000 }
        }
    }
}

#[derive(Debug)]
struct WindowEntry {
    timestamp: Instant,
    tokens:    u32,
}

#[derive(Debug, Default)]
struct ModelWindow {
    entries: Vec<WindowEntry>,
}

impl ModelWindow {
    fn purge_stale(&mut self) {
        let cutoff = Instant::now() - Duration::from_secs(60);
        self.entries.retain(|e| e.timestamp > cutoff);
    }

    fn token_sum(&self) -> u32 {
        self.entries.iter().map(|e| e.tokens).sum()
    }

    fn ms_until_slot_free(&self) -> u64 {
        if self.entries.is_empty() { return 0; }
        let oldest = self.entries.iter().map(|e| e.timestamp).min().unwrap();
        (Duration::from_secs(60).saturating_sub(oldest.elapsed())).as_millis() as u64 + 100
    }
}

pub struct TokenLedger {
    windows: Mutex<HashMap<String, ModelWindow>>,
}

impl TokenLedger {
    pub fn new() -> Self {
        Self { windows: Mutex::new(HashMap::new()) }
    }

    pub fn check_and_reserve(&self, model: &str, estimated_tokens: u32) -> Result<(), u64> {
        let quota = ModelQuota::for_model(model);
        let mut map = self.windows.lock().unwrap();
        let window = map.entry(model.to_string()).or_default();
        window.purge_stale();

        if window.entries.len() as u32 >= quota.rpm_limit {
            return Err(window.ms_until_slot_free());
        }
        if window.token_sum() + estimated_tokens > quota.tpm_limit {
            return Err(window.ms_until_slot_free());
        }

        window.entries.push(WindowEntry { timestamp: Instant::now(), tokens: estimated_tokens });
        Ok(())
    }
}

pub static LEDGER: Lazy<TokenLedger> = Lazy::new(TokenLedger::new);

pub async fn wait_for_quota(model: &str, estimated_tokens: u32) {
    let mut attempts = 0;
    loop {
        match LEDGER.check_and_reserve(model, estimated_tokens) {
            Ok(()) => return,
            Err(wait_ms) => {
                let jitter = (generate_seed() % 500) as u64;
                println!(
                    "\x1b[33m[LEDGER] {} ceiling hit. Waiting {}ms (attempt {})...\x1b[0m",
                    model, wait_ms + jitter, attempts + 1
                );
                tokio::time::sleep(Duration::from_millis(wait_ms + jitter)).await;
                attempts += 1;
                if attempts > 12 { break; }
            }
        }
    }
}

fn generate_seed() -> u64 {
    use std::hash::{BuildHasher, Hasher};
    let t = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64;
    let mut h = std::collections::hash_map::RandomState::new().build_hasher();
    h.write_u64(t);
    h.finish()
}

// Keep the rest of the file (HTTP limits, etc.) unchanged
fn http_rpm_limit() -> u32 {
    std::env::var("NEXUS_HTTP_RPM").ok().and_then(|v| v.parse().ok()).unwrap_or(60)
}

static HTTP_WINDOWS: Lazy<Mutex<HashMap<String, Vec<Instant>>>> = Lazy::new(|| Mutex::new(HashMap::new()));

pub fn http_check(ip: &str) -> Result<(), u64> {
    let limit = http_rpm_limit();
    let cutoff = Instant::now() - Duration::from_secs(60);

    let mut map = HTTP_WINDOWS.lock().unwrap();
    let window = map.entry(ip.to_string()).or_default();
    window.retain(|&ts| ts > cutoff);

    if window.len() as u32 >= limit {
        let oldest = *window.first().unwrap();
        let wait = (Duration::from_secs(60).saturating_sub(oldest.elapsed())).as_millis() as u64;
        return Err(wait + 100);
    }

    window.push(Instant::now());
    Ok(())
}
NEWLEDGER

echo "✅ Updated crates/nexus_substrate/src/token_ledger.rs with better Gemma 4 quotas."
echo "Next step: cargo check -p nexus_substrate"
