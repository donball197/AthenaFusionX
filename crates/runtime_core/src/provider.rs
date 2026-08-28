#![allow(clippy::doc_overindented_list_items)]
//! Unified LLM provider resolution + cascade construction.
//!
//! SINGLE SOURCE OF TRUTH for:
//!   - resolve_provider(model) → (endpoint, api_key, auth_style)
//!   - get_light_models() / get_heavy_models()  (pure, no cross-tier injection)
//!   - cascade helpers
//!
//! All other crates MUST call into this module (or re-export it).
//! Do NOT re-implement provider detection elsewhere.

use std::env;
use std::sync::atomic::{AtomicU64, Ordering};

/// Circuit-breaker timestamps (unix secs). 0 = healthy.
static GEMINI_DISABLED_UNTIL: AtomicU64 = AtomicU64::new(0);
static NVIDIA_DISABLED_UNTIL: AtomicU64 = AtomicU64::new(0);
const CIRCUIT_COOLDOWN_SECS: u64 = 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthStyle {
    /// Authorization: Bearer <key>
    Bearer,
    /// x-goog-api-key: <key>
    GoogleApiKey,
}

#[derive(Debug, Clone)]
pub struct ProviderResolved {
    pub endpoint: String,
    pub api_key: String,
    pub auth: AuthStyle,
    pub provider_name: &'static str,
}

/// Record a rate-limit / 429 so the cascade skips this provider briefly.
pub fn trip_circuit(model_name: &str) {
    let now = now_secs();
    let until = now + CIRCUIT_COOLDOWN_SECS;
    let lower = model_name.to_ascii_lowercase();
    if lower.contains("gemini") || lower.contains("gemma") {
        GEMINI_DISABLED_UNTIL.store(until, Ordering::Relaxed);
        tracing::warn!("[CIRCUIT] Google API rate-limited; bypassing for {}s", CIRCUIT_COOLDOWN_SECS);
    } else if lower.contains("nvidia") || lower.contains("nemotron") || lower.contains("deepseek") {
        NVIDIA_DISABLED_UNTIL.store(until, Ordering::Relaxed);
        tracing::warn!(
            "[CIRCUIT] NVIDIA/NIM rate-limited; bypassing for {}s",
            CIRCUIT_COOLDOWN_SECS
        );
    }
}

fn now_secs() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs()
}

fn provider_healthy(kind: &str) -> bool {
    let now = now_secs();
    match kind {
        "gemini" | "gemma" => now > GEMINI_DISABLED_UNTIL.load(Ordering::Relaxed),
        "nvidia" => now > NVIDIA_DISABLED_UNTIL.load(Ordering::Relaxed),
        _ => true,
    }
}

/// Resolve the correct endpoint + key for a model string.
pub fn resolve_provider(model_name: &str) -> ProviderResolved {
    let model = model_name.to_ascii_lowercase();

    // Google Gemini & Gemma
    if model.contains("gemini") || model.contains("gemma") {
        return ProviderResolved {
            endpoint: "https://generativelanguage.googleapis.com/v1beta/openai/chat/completions"
                .to_string(),
            api_key: env::var("GEMINI_API_KEY").unwrap_or_default(),
            auth: AuthStyle::GoogleApiKey, // also accept Bearer on the openai path
            provider_name: "google",
        };
    }

    // NVIDIA NIM / hosted models (explicit prefixes only)
    if model.contains("nvidia")
        || model.contains("nemotron")
        || model.starts_with("deepseek-ai/")
        || model.contains("deepseek")
    {
        return ProviderResolved {
            endpoint: "https://integrate.api.nvidia.com/v1/chat/completions".to_string(),
            api_key: env::var("NVIDIA_API_KEY").unwrap_or_default(),
            auth: AuthStyle::Bearer,
            provider_name: "nvidia",
        };
    }

    // Configured Nexus / custom OpenAI-compatible gateway
    ProviderResolved {
        endpoint: env::var("NEXUS_API_ENDPOINT")
            .unwrap_or_else(|_| "https://api.example.com/v1/chat/completions".to_string()),
        api_key: env::var("NEXUS_AUTH_KEY").unwrap_or_default(),
        auth: AuthStyle::Bearer,
        provider_name: "nexus",
    }
}

/// Pure light-model list. Never injects heavy models.
pub fn get_light_models() -> Vec<String> {
    let mut out = Vec::new();

    // Explicit cascade first
    if let Ok(cascade) = env::var("NEXUS_LIGHT_CASCADE") {
        for m in cascade.split(',') {
            let m = m.trim();
            if !m.is_empty() && !out.iter().any(|x| x == m) {
                out.push(m.to_string());
            }
        }
    }

    // Single light model
    if let Ok(m) = env::var("NEXUS_LIGHT_MODEL") {
        let m = m.trim();
        if !m.is_empty() && !out.iter().any(|x| x == m) {
            out.push(m.to_string());
        }
    }

    // Filter by circuit breaker
    out.retain(|m| {
        let lower = m.to_ascii_lowercase();
        if lower.contains("gemini") || lower.contains("gemma") {
            provider_healthy("gemini")
        } else if lower.contains("nvidia")
            || lower.contains("nemotron")
            || lower.contains("deepseek")
        {
            provider_healthy("nvidia")
        } else {
            true
        }
    });

    if out.is_empty() {
        // Last-resort safe defaults (light capacity only)
        out.push("gemini-2.0-flash".to_string());
    }
    out
}

/// Pure heavy-model list. Never injects light models.
pub fn get_heavy_models() -> Vec<String> {
    let mut out = Vec::new();

    if let Ok(cascade) = env::var("NEXUS_HEAVY_CASCADE") {
        for m in cascade.split(',') {
            let m = m.trim();
            if !m.is_empty() && !out.iter().any(|x| x == m) {
                out.push(m.to_string());
            }
        }
    }

    if let Ok(m) = env::var("NEXUS_HEAVY_MODEL") {
        let m = m.trim();
        if !m.is_empty() && !out.iter().any(|x| x == m) {
            out.push(m.to_string());
        }
    }

    out.retain(|m| {
        let lower = m.to_ascii_lowercase();
        if lower.contains("gemini") || lower.contains("gemma") {
            provider_healthy("gemini")
        } else if lower.contains("nvidia")
            || lower.contains("nemotron")
            || lower.contains("deepseek")
        {
            provider_healthy("nvidia")
        } else {
            true
        }
    });

    if out.is_empty() {
        out.push("gemini-2.5-pro".to_string());
    }
    out
}

/// Planner model list (planner first, then light cascade).
pub fn get_planner_models() -> Vec<String> {
    let mut out = Vec::new();
    if let Ok(m) = env::var("NEXUS_PLANNER_MODEL") {
        let m = m.trim();
        if !m.is_empty() {
            out.push(m.to_string());
        }
    }
    for m in get_light_models() {
        if !out.iter().any(|x| x == &m) {
            out.push(m);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gemini_routes_to_google() {
        let r = resolve_provider("gemini-2.0-flash");
        assert!(r.endpoint.contains("generativelanguage.googleapis.com"));
        assert_eq!(r.provider_name, "google");
    }

    #[test]
    fn gemma_routes_to_google() {
        let r = resolve_provider("gemma-4-31b-it");
        assert!(r.endpoint.contains("generativelanguage.googleapis.com"));
        assert_eq!(r.provider_name, "google");
    }

    #[test]
    fn nemotron_routes_to_nvidia() {
        let r = resolve_provider("nvidia/nemotron-3-ultra-550b-a55b");
        assert!(r.endpoint.contains("integrate.api.nvidia.com"));
        assert_eq!(r.provider_name, "nvidia");
        assert_eq!(r.auth, AuthStyle::Bearer);
    }

    #[test]
    fn deepseek_routes_to_nvidia() {
        let r = resolve_provider("deepseek-ai/deepseek-v4-flash-0731");
        assert!(r.endpoint.contains("integrate.api.nvidia.com"));
        assert_eq!(r.provider_name, "nvidia");
    }

    #[test]
    fn unknown_family_uses_nexus() {
        // "llama" without provider prefix must NOT go to Google or NVIDIA by name alone
        let r = resolve_provider("llama-3-8b");
        assert_eq!(r.provider_name, "nexus");
    }
}
