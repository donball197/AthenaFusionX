use std::env;
use std::sync::atomic::{AtomicU64, Ordering};

static GEMINI_DISABLED_UNTIL: AtomicU64 = AtomicU64::new(0);
static NVIDIA_DISABLED_UNTIL: AtomicU64 = AtomicU64::new(0);

pub fn report_rate_limit(model_name: &str) {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let cooldown = 60;

    if model_name.contains("gemini") || model_name.contains("gemma") {
        GEMINI_DISABLED_UNTIL.store(now + cooldown, Ordering::Relaxed);
        tracing::warn!("[CIRCUIT BREAKER] Gemini rate-limited (429). Bypassing for 60s.");
    } else if model_name.contains("nvidia") || model_name.contains("nemotron") {
        NVIDIA_DISABLED_UNTIL.store(now + cooldown, Ordering::Relaxed);
        tracing::warn!("[CIRCUIT BREAKER] NVIDIA rate-limited (429). Bypassing for 60s.");
    }
}

pub fn get_light_models() -> Vec<String> {
    let mut healthy = Vec::new();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    if let Ok(m) = env::var("NEXUS_LIGHT_MODEL") {
        if !m.trim().is_empty() && !m.contains("mistral-nemo") {
            healthy.push(m.trim().to_string());
        }
    }
    if let Ok(m) = env::var("NEXUS_PLANNER_MODEL") {
        if !m.trim().is_empty()
            && !m.contains("mistral-nemo")
            && !healthy.contains(&m.trim().to_string())
        {
            healthy.push(m.trim().to_string());
        }
    }

    let nv_key = env::var("NVIDIA_API_KEY").unwrap_or_default();
    if !nv_key.trim().is_empty() && now > NVIDIA_DISABLED_UNTIL.load(Ordering::Relaxed) {
        let heavy = env::var("NEXUS_HEAVY_MODEL")
            .unwrap_or_else(|_| "nvidia/nemotron-3-ultra-550b-a55b".to_string());
        if !healthy.contains(&heavy) {
            healthy.push(heavy);
        }
    }

    let gemini_key = env::var("GEMINI_API_KEY").unwrap_or_default();
    if !gemini_key.trim().is_empty() && now > GEMINI_DISABLED_UNTIL.load(Ordering::Relaxed) {
        let light =
            env::var("NEXUS_LIGHT_MODEL").or_else(|_| env::var("GEMINI_MODEL")).unwrap_or_else(|_| {
    eprintln!("[!] WARNING: No NEXUS_LIGHT_MODEL or GEMINI_MODEL set in environment. Defaulting to Gemini 3.7 Flash.");
    "gemini-3.7-flash".to_string()
});
        if !healthy.contains(&light) {
            healthy.push(light);
        }
    }

    if healthy.is_empty() {
        healthy.push("nvidia/nemotron-3-ultra-550b-a55b".to_string());
        healthy.push("gemini-3.5-flash-lite".to_string());
    }

    healthy
}

pub fn get_heavy_models() -> Vec<String> {
    let mut healthy = Vec::new();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    if let Ok(m) = env::var("NEXUS_HEAVY_MODEL") {
        if !m.trim().is_empty() && !m.contains("mistral-nemo") {
            healthy.push(m.trim().to_string());
        }
    }
    if let Ok(m) = env::var("NEXUS_PLANNER_MODEL") {
        if !m.trim().is_empty()
            && !m.contains("mistral-nemo")
            && !healthy.contains(&m.trim().to_string())
        {
            healthy.push(m.trim().to_string());
        }
    }

    let nv_key = env::var("NVIDIA_API_KEY").unwrap_or_default();
    if !nv_key.trim().is_empty() && now > NVIDIA_DISABLED_UNTIL.load(Ordering::Relaxed) {
        let heavy = env::var("NEXUS_HEAVY_MODEL")
            .unwrap_or_else(|_| "nvidia/nemotron-3-ultra-550b-a55b".to_string());
        if !healthy.contains(&heavy) {
            healthy.push(heavy);
        }
    }

    if healthy.is_empty() {
        healthy.push("nvidia/nemotron-3-ultra-550b-a55b".to_string());
    }

    healthy
}
