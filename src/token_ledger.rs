// src/token_ledger.rs — v1.3 [QUOTA_ENFORCED]
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
        match model {
            // Guarding Flash limits (e.g. 15 RPM / 1M TPM)
            m if m.contains("flash") => Self { rpm_limit: 15, tpm_limit: 1_000_000 },
            // Higher limits for stable workhorses
            m if m.contains("31b") => Self { rpm_limit: 20, tpm_limit: 2_000_000 },
            _ => Self { rpm_limit: 15, tpm_limit: 1_000_000 },
        }
    }
}

#[derive(Debug)]
struct WindowEntry {
    timestamp: Instant,
    tokens: u32,
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

    /// Sums tokens used in the current 60s window.
    fn token_sum(&self) -> u32 {
        self.entries.iter().map(|e| e.tokens).sum()
    }

    fn ms_until_slot_free(&self) -> u64 {
        if self.entries.is_empty() { return 0; }
        let oldest = self.entries.iter().map(|e| e.timestamp).min().unwrap();
        let age = oldest.elapsed();
        (Duration::from_secs(60).saturating_sub(age)).as_millis() as u64 + 100
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

        // 1. Check RPM (Requests Per Minute)
        if window.entries.len() as u32 >= quota.rpm_limit {
            return Err(window.ms_until_slot_free());
        }

        // 2. Check TPM (Tokens Per Minute) - FIX: Field is now read here
        let current_tpm = window.token_sum();
        if current_tpm + estimated_tokens > quota.tpm_limit {
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
                if attempts > 10 { break; }
            }
        }
    }
}

/// generate_seed: Basic XORShift-style entropy for jitter
fn generate_seed() -> u64 {
    let t = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_nanos();
    let ptr = &t as *const u128 as u64;
    let mut x = (t as u64) ^ ptr;
    x ^= x << 13; x ^= x >> 7; x ^= x << 17;
    x
}
