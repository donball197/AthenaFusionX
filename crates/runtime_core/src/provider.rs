use std::env;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthStyle {
    Bearer,
    GoogleApiKey,
}

#[derive(Debug, Clone)]
pub struct ProviderResolved {
    pub endpoint: String,
    pub api_key: String,
    pub auth: AuthStyle,
    pub provider_name: &'static str,
}

pub fn trip_circuit(_model_name: &str) {}

pub fn resolve_provider(model_name: &str) -> ProviderResolved {
    let model = model_name.to_ascii_lowercase();

    // 1. Local S23 Edge LiteRT-LM (Strictly e4b or explicit local names)
    if model.contains("e4b") || model.contains("litert") || model == "nexus" {
        let gw = env::var("OPENAI_BASE_URL")
            .or_else(|_| env::var("NEXUS_API_GATEWAY"))
            .unwrap_or_else(|_| "http://192.168.4.223:9379/v1".to_string());
        let clean = gw.trim_end_matches('/');
        let endpoint = if clean.ends_with("/chat/completions") {
            clean.to_string()
        } else {
            format!("{}/chat/completions", clean)
        };
        let key = env::var("OPENAI_API_KEY")
            .or_else(|_| env::var("NEXUS_AUTH_KEY"))
            .unwrap_or_else(|_| "local-s23-device".to_string());
        return ProviderResolved {
            provider_name: "nexus",
            endpoint,
            api_key: key,
            auth: AuthStyle::Bearer,
        };
    }

    // 2. Google Cloud (Gemini models, plus the known Cloud Gemma default gemma-4-31b-it).
    // Bare/unknown Gemma family names must NOT guess Google -- fall through to Nexus default.
    if model.contains("gemini") || model == "gemma-4-31b-it" {
        let api_key =
            env::var("GEMINI_API_KEY").or_else(|_| env::var("GOOGLE_API_KEY")).unwrap_or_default();
        return ProviderResolved {
            endpoint: "https://generativelanguage.googleapis.com/v1beta/openai/chat/completions"
                .to_string(),
            api_key,
            auth: AuthStyle::GoogleApiKey,
            provider_name: "google",
        };
    }

    // 3. NVIDIA NIM Cloud (nvidia/*, meta/*, deepseek/*, google/*)
    if model.contains("nvidia")
        || model.contains("nemotron")
        || model.contains("deepseek")
        || model.contains("meta/")
        || model.starts_with("google/")
    {
        let api_key = env::var("NVIDIA_API_KEY").unwrap_or_default();
        return ProviderResolved {
            endpoint: "https://integrate.api.nvidia.com/v1/chat/completions".to_string(),
            api_key,
            auth: AuthStyle::Bearer,
            provider_name: "nvidia",
        };
    }

    // Fallback default: Local S23
    ProviderResolved {
        endpoint: "http://192.168.4.223:9379/v1/chat/completions".to_string(),
        api_key: "local-s23-device".to_string(),
        auth: AuthStyle::Bearer,
        provider_name: "nexus",
    }
}

pub fn get_light_models() -> Vec<String> {
    let mut out = Vec::new();
    out.extend(collect_cascade("NEXUS_LIGHT_MODEL", "NEXUS_LIGHT_CASCADE"));
    out
}

/// Builds a de-duplicated model list from a primary env var plus an optional
/// comma-separated cascade env var. No hardcoded model defaults: if nothing
/// is configured, this returns an empty vec, and callers (safe_api_call) must
/// surface that as a clear config error rather than silently guessing a model.
fn collect_cascade(primary_var: &str, cascade_var: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut seen = std::collections::HashSet::new();

    if let Ok(m) = env::var(primary_var) {
        let m = m.trim().to_string();
        if !m.is_empty() && seen.insert(m.clone()) {
            out.push(m);
        }
    }

    if let Ok(cascade) = env::var(cascade_var) {
        for entry in cascade.split(',') {
            let entry = entry.trim().to_string();
            if !entry.is_empty() && seen.insert(entry.clone()) {
                out.push(entry);
            }
        }
    }

    out
}

pub fn get_heavy_models() -> Vec<String> {
    collect_cascade("NEXUS_HEAVY_MODEL", "NEXUS_HEAVY_CASCADE")
}

pub fn get_planner_models() -> Vec<String> {
    collect_cascade("NEXUS_PLANNER_MODEL", "NEXUS_PLANNER_CASCADE")
}
