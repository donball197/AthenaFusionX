// src/token_ledger.rs — v1.1 [FLASH_QUOTA_GUARD]
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct ModelQuota {
    pub rpm_limit: u32,
    pub tpm_limit: u32,
}

impl ModelQuota {
    fn for_model(model: &str) -> Self {
        match model {
            // ⚡ ULTRA-CONSERVATIVE: Guarding the 20 RPD limit for Gemini 2.5 Flash
            m if m.contains("flash") => Self { rpm_limit: 1, tpm_limit: 15_000 },
            m if m.contains("31b") => Self { rpm_limit: 15, tpm_limit: 50_000 },
            m if m.contains("pro") => Self { rpm_limit: 5, tpm_limit: 250_000 },
            // Default Workhorse (Gemma 4 26B)
            _ => Self {
                rpm_limit: std::env::var("NEXUS_RPM_OVERRIDE_26B")
                    .ok().and_then(|v| v.parse().ok()).unwrap_or(15),
                tpm_limit: std::env::var("NEXUS_TPM_OVERRIDE_26B")
                    .ok().and_then(|v| v.parse().ok()).unwrap_or(1_000_000),
            },
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

    fn request_count(&mut self) -> u32 {
        self.purge_stale();
        self.entries.len() as u32
    }

    fn token_sum(&mut self) -> u32 {
        self.purge_stale();
        self.entries.iter().map(|e| e.tokens).sum()
    }

    fn add(&mut self, tokens: u32) {
        self.entries.push(WindowEntry { timestamp: Instant::now(), tokens });
    }

    fn ms_until_slot_free(&self) -> u64 {
        if self.entries.is_empty() { return 0; }
        let oldest = self.entries.iter().map(|e| e.timestamp).min().unwrap();
        let age = oldest.elapsed();
        if age >= Duration::from_secs(60) {
            0
        } else {
            (Duration::from_secs(60) - age).as_millis() as u64 + 500
        }
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

        let current_rpm = window.request_count();
        let current_tpm = window.token_sum();

        if current_rpm >= quota.rpm_limit {
            let wait = window.ms_until_slot_free();
            println!("\x1b[33m[LEDGER] RPM ceiling hit for {} ({}/{}). Backoff {}ms\x1b[0m", model, current_rpm, quota.rpm_limit, wait);
            return Err(wait);
        }

        if current_tpm + estimated_tokens > quota.tpm_limit {
            let wait = window.ms_until_slot_free();
            println!("\x1b[33m[LEDGER] TPM ceiling hit for {} ({}/{}). Backoff {}ms\x1b[0m", model, current_tpm + estimated_tokens, quota.tpm_limit, wait);
            return Err(wait);
        }

        window.add(estimated_tokens);
        Ok(())
    }

    pub fn record_actual(&self, model: &str, actual_tokens: u32) {
        let mut map = self.windows.lock().unwrap();
        let window = map.entry(model.to_string()).or_default();
        if let Some(last) = window.entries.last_mut() {
            last.tokens = actual_tokens;
        }
    }
}

pub static LEDGER: Lazy<TokenLedger> = Lazy::new(TokenLedger::new);

pub async fn wait_for_quota(model: &str, estimated_tokens: u32) {
    let mut attempts = 0u32;
    loop {
        match LEDGER.check_and_reserve(model, estimated_tokens) {
            Ok(()) => return,
            Err(wait_ms) => {
                let jitter = (rand_u64() % 300) as u64;
                let backoff = wait_ms + (2u64.saturating_pow(attempts.min(6)) * 200) + jitter;
                println!("\x1b[33m[LEDGER] Waiting {}ms before retry (attempt {})\x1b[0m", backoff, attempts + 1);
                tokio::time::sleep(tokio::time::Duration::from_millis(backoff)).await;
                attempts += 1;
            }
        }
    }
}

fn rand_u64() -> u64 {
    use std::time::SystemTime;
    let stack_entropy = { let local: u64 = 0; &local as *const u64 as u64 };
    let time_entropy = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map(|d| d.subsec_nanos() as u64).unwrap_or(0x8a4f3c1b);
    let mut x = time_entropy ^ stack_entropy ^ 0xdeadbeefcafe1234;
    x ^= x << 13; x ^= x >> 7; x ^= x << 17;
    x
}
