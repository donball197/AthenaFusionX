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
            m if m.contains("flash-lite") => Self { rpm_limit: 10, tpm_limit: 100_000 },
            m if m.contains("flash") => Self { rpm_limit: 5, tpm_limit: 60_000 },
            m if m.contains("31b") => Self { rpm_limit: 3, tpm_limit: 50_000 },
            m if m.contains("pro") => Self { rpm_limit: 2, tpm_limit: 30_000 },
            m if m.contains("research") => Self { rpm_limit: 1, tpm_limit: 10_000 },
            _ => Self { rpm_limit: 3, tpm_limit: 30_000 },
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
    fn token_sum(&self) -> u32 {
        self.entries.iter().map(|e| e.tokens).sum()
    }
    fn ms_until_slot_free(&self) -> u64 {
        if self.entries.is_empty() {
            return 0;
        }
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
        let win = map.entry(model.to_string()).or_default();
        win.purge_stale();
        if win.entries.len() as u32 >= quota.rpm_limit {
            return Err(win.ms_until_slot_free());
        }
        if win.token_sum() + estimated_tokens > quota.tpm_limit {
            return Err(win.ms_until_slot_free());
        }
        win.entries.push(WindowEntry { timestamp: Instant::now(), tokens: estimated_tokens });
        Ok(())
    }
}

pub static LEDGER: Lazy<TokenLedger> = Lazy::new(TokenLedger::new);

/// Returns Ok(()) when a quota slot is reserved.
/// Returns Err(message) after MAX_ATTEMPTS — caller MUST NOT fire the API on Err.
pub async fn wait_for_quota(model: &str, estimated_tokens: u32) -> Result<(), String> {
    const MAX_ATTEMPTS: usize = 10;
    for attempt in 0..MAX_ATTEMPTS {
        match LEDGER.check_and_reserve(model, estimated_tokens) {
            Ok(()) => return Ok(()),
            Err(wait_ms) => {
                let jitter = generate_seed() % 500;
                tracing::warn!(
                    "[LEDGER] {} ceiling hit — waiting {}ms (attempt {}/{}).",
                    model,
                    wait_ms + jitter,
                    attempt + 1,
                    MAX_ATTEMPTS
                );
                tokio::time::sleep(Duration::from_millis(wait_ms + jitter)).await;
            }
        }
    }
    Err(format!("[LEDGER] Quota for '{}' unavailable after {} attempts.", model, MAX_ATTEMPTS))
}

fn parse_rpm(raw: Option<&str>) -> u32 {
    // 0 would reject every request on an empty window (and used to panic on unwrap).
    raw.and_then(|v| v.parse::<u32>().ok()).unwrap_or(60).max(1)
}

fn http_rpm_limit() -> u32 {
    parse_rpm(std::env::var("NEXUS_HTTP_RPM").ok().as_deref())
}

static HTTP_WINDOWS: Lazy<Mutex<HashMap<String, Vec<Instant>>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

pub fn http_check(ip: &str) -> Result<(), u64> {
    let limit = http_rpm_limit();
    let cutoff = Instant::now() - Duration::from_secs(60);
    let mut map = HTTP_WINDOWS.lock().unwrap_or_else(|e| e.into_inner());
    let win = map.entry(ip.to_string()).or_default();
    win.retain(|&ts| ts > cutoff);
    if win.len() as u32 >= limit {
        let oldest = *win.first().unwrap();
        return Err(
            (Duration::from_secs(60).saturating_sub(oldest.elapsed())).as_millis() as u64 + 100
        );
    }
    win.push(Instant::now());
    Ok(())
}

fn generate_seed() -> u64 {
    use std::hash::{BuildHasher, Hasher};
    let t = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap_or_default().as_nanos()
        as u64;
    let mut h = std::collections::hash_map::RandomState::new().build_hasher();
    h.write_u64(t);
    h.finish()
}

impl Default for TokenLedger {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod http_rpm_tests {
    use super::parse_rpm;

    #[test]
    fn zero_is_clamped_to_one() {
        assert_eq!(parse_rpm(Some("0")), 1);
    }

    #[test]
    fn default_and_garbage_use_sixty() {
        assert_eq!(parse_rpm(None), 60);
        assert_eq!(parse_rpm(Some("abc")), 60);
        assert_eq!(parse_rpm(Some("-5")), 60);
    }

    #[test]
    fn valid_values_pass_through() {
        assert_eq!(parse_rpm(Some("120")), 120);
    }
}
