// src/token_ledger.rs — Token Ledger v1.0
// Copyright 2026 Donald Ball — Apache 2.0
//
// Tracks per-model token and request spend inside a rolling 60-second window.
// Call `check_and_reserve` before every API call. It returns:
//   Ok(())           — cleared, proceed
//   Err(wait_ms)     — backoff this many milliseconds before retrying
//
// Gemini free-tier limits (as of April 2026):
//   gemma-4-26b-a4b-it : 15 RPM, 1_000_000 TPM
//   gemma-4-31b-it     : 2 RPM,    50_000 TPM
//   gemini-2.5-pro     : 5 RPM,   250_000 TPM  (emergency only)
//
// Set NEXUS_RPM_OVERRIDE_26B / NEXUS_TPM_OVERRIDE_26B env vars to tune.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use once_cell::sync::Lazy;

#[derive(Debug, Clone)]
pub struct ModelQuota {
    pub rpm_limit: u32,
    pub tpm_limit: u32,
}

impl ModelQuota {
    fn for_model(model: &str) -> Self {
        match model {
            m if m.contains("31b") => Self { rpm_limit: 2,  tpm_limit: 50_000 },
            m if m.contains("2.5-pro") => Self { rpm_limit: 5, tpm_limit: 250_000 },
            // gemma-4-26b-a4b-it and all other MoE models
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
    tokens:    u32,
}

#[derive(Debug, Default)]
struct ModelWindow {
    entries: Vec<WindowEntry>,
}

impl ModelWindow {
    /// Purge entries older than 60 seconds
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

    /// Returns how many ms until the oldest entry expires (leaves the window)
    fn ms_until_slot_free(&self) -> u64 {
        if self.entries.is_empty() { return 0; }
        let oldest = self.entries.iter().map(|e| e.timestamp).min().unwrap();
        let age = oldest.elapsed();
        if age >= Duration::from_secs(60) {
            0
        } else {
            (Duration::from_secs(60) - age).as_millis() as u64 + 500 // +500ms safety buffer
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

    /// check_and_reserve: Call BEFORE every API request.
    /// estimated_tokens: rough estimate of input+output tokens for this call.
    /// Returns Ok(()) if cleared, Err(wait_ms) if the caller should back off.
    pub fn check_and_reserve(&self, model: &str, estimated_tokens: u32) -> Result<(), u64> {
        let quota = ModelQuota::for_model(model);
        let mut map = self.windows.lock().unwrap();
        let window = map.entry(model.to_string()).or_default();

        let current_rpm = window.request_count();
        let current_tpm = window.token_sum();

        // RPM check
        if current_rpm >= quota.rpm_limit {
            let wait = window.ms_until_slot_free();
            println!(
                "\x1b[33m[LEDGER] RPM ceiling hit for {} ({}/{}). Backoff {}ms\x1b[0m",
                model, current_rpm, quota.rpm_limit, wait
            );
            return Err(wait);
        }

        // TPM check
        if current_tpm + estimated_tokens > quota.tpm_limit {
            let wait = window.ms_until_slot_free();
            println!(
                "\x1b[33m[LEDGER] TPM ceiling hit for {} ({}/{}). Backoff {}ms\x1b[0m",
                model, current_tpm + estimated_tokens, quota.tpm_limit, wait
            );
            return Err(wait);
        }

        // All clear — record the reservation
        window.add(estimated_tokens);
        Ok(())
    }

    /// record_actual: Call AFTER a successful response with the real token count.
    /// This corrects the estimate recorded by check_and_reserve.
    pub fn record_actual(&self, model: &str, actual_tokens: u32) {
        let mut map = self.windows.lock().unwrap();
        let window = map.entry(model.to_string()).or_default();
        // Replace the last entry's token count with the actual
        if let Some(last) = window.entries.last_mut() {
            last.tokens = actual_tokens;
        }
    }

    /// status: Returns a human-readable status string for the dashboard / metrics endpoint.
    pub fn status(&self) -> String {
        let mut map = self.windows.lock().unwrap();
        let mut lines = vec!["[TOKEN LEDGER STATUS]".to_string()];
        for (model, window) in map.iter_mut() {
            let quota = ModelQuota::for_model(model);
            lines.push(format!(
                "  {} → RPM: {}/{} | TPM: {}/{}",
                model,
                window.request_count(),
                quota.rpm_limit,
                window.token_sum(),
                quota.tpm_limit,
            ));
        }
        lines.join("\n")
    }

    /// percent_tpm_used: 0.0–1.0 saturation for a given model.
    pub fn percent_tpm_used(&self, model: &str) -> f32 {
        let quota = ModelQuota::for_model(model);
        let mut map = self.windows.lock().unwrap();
        let window = map.entry(model.to_string()).or_default();
        window.token_sum() as f32 / quota.tpm_limit as f32
    }
}

/// Global ledger — shared across all tasks via Arc
pub static LEDGER: Lazy<TokenLedger> = Lazy::new(TokenLedger::new);

/// wait_for_quota: Blocking async backoff helper.
/// Waits for the ledger to clear, then reserves.
/// Adds exponential backoff with jitter on repeated 429s.
pub async fn wait_for_quota(model: &str, estimated_tokens: u32) {
    let mut attempts = 0u32;
    loop {
        match LEDGER.check_and_reserve(model, estimated_tokens) {
            Ok(()) => return,
            Err(wait_ms) => {
                // Exponential backoff with jitter: base wait + (2^attempts * 200ms) + jitter
                let jitter = (rand_u64() % 300) as u64;
                let backoff = wait_ms + (2u64.saturating_pow(attempts.min(6)) * 200) + jitter;
                println!(
                    "\x1b[33m[LEDGER] Waiting {}ms before retry (attempt {})\x1b[0m",
                    backoff, attempts + 1
                );
                tokio::time::sleep(tokio::time::Duration::from_millis(backoff)).await;
                attempts += 1;
            }
        }
    }
}

/// Tiny seedless xorshift RNG to avoid pulling in the rand crate
fn rand_u64() -> u64 {
    use std::time::SystemTime;
    let mut x = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(12345) as u64;
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    x
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn respects_rpm_limit() {
        let ledger = TokenLedger::new();
        // gemma-4-31b-it has rpm=2
        ledger.check_and_reserve("gemma-4-31b-it", 100).unwrap();
        ledger.check_and_reserve("gemma-4-31b-it", 100).unwrap();
        let result = ledger.check_and_reserve("gemma-4-31b-it", 100);
        assert!(result.is_err(), "Third request should be blocked by RPM limit");
    }

    #[test]
    fn different_models_independent_windows() {
        let ledger = TokenLedger::new();
        ledger.check_and_reserve("gemma-4-31b-it", 100).unwrap();
        ledger.check_and_reserve("gemma-4-31b-it", 100).unwrap();
        // MoE model has its own window — should still pass
        assert!(ledger.check_and_reserve("gemma-4-26b-a4b-it", 100).is_ok());
    }
}
