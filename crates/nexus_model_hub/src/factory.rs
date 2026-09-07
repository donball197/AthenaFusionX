use std::env;
use std::fs;

/// Checks for manual runtime pin (.active_model)
fn get_pinned_model() -> Option<String> {
    if let Ok(content) = fs::read_to_string(".active_model") {
        let trimmed = content.trim().to_string();
        if !trimmed.is_empty() {
            return Some(trimmed);
        }
    }
    None
}

/// Reads directly from .env file live on disk; falls back to process env::var
fn get_config_var(key: &str) -> Option<String> {
    if let Ok(content) = fs::read_to_string(".env") {
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            if let Some((k, v)) = trimmed.split_once('=') {
                if k.trim() == key {
                    let val = v.trim().trim_matches('"').trim_matches('\'').to_string();
                    if !val.is_empty() {
                        return Some(val);
                    }
                }
            }
        }
    }
    env::var(key).ok()
}

/// Fallback to generic system model defined in .env
fn get_global_model() -> String {
    get_config_var("NEXUS_MODEL")
        .or_else(|| get_config_var("MODEL"))
        .unwrap_or_else(|| {
            panic!("FATAL: No model configured. Define NEXUS_MODEL in .env or create .active_model")
        })
}

/// Parse comma-delimited cascade lists dynamically
fn parse_cascade(env_var: &str) -> Vec<String> {
    get_config_var(env_var)
        .map(|val| {
            val.split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

pub fn get_flash_lite_model() -> String {
    if let Some(pinned) = get_pinned_model() {
        return pinned;
    }
    get_config_var("NEXUS_LIGHT_MODEL")
        .or_else(|| parse_cascade("NEXUS_LIGHT_CASCADE").into_iter().next())
        .unwrap_or_else(get_global_model)
}

pub fn get_heavy_model() -> String {
    if let Some(pinned) = get_pinned_model() {
        return pinned;
    }
    get_config_var("NEXUS_HEAVY_MODEL")
        .or_else(|| parse_cascade("NEXUS_HEAVY_CASCADE").into_iter().next())
        .unwrap_or_else(get_global_model)
}

pub struct AgentFactory;

impl AgentFactory {
    pub fn new() -> Self {
        Self
    }

    /// Selects model strictly from .active_model, .env cascades, or .env tiers
    pub fn get_model_for_turn(turn: usize, error_count: usize) -> String {
        if let Some(pinned) = get_pinned_model() {
            return pinned;
        }

        let light_cascade = parse_cascade("NEXUS_LIGHT_CASCADE");
        let heavy_cascade = parse_cascade("NEXUS_HEAVY_CASCADE");

        let max_light_turns: usize = get_config_var("NEXUS_LIGHT_MAX_TURNS")
            .and_then(|v| v.parse().ok())
            .unwrap_or_else(|| {
                if light_cascade.is_empty() {
                    1
                } else {
                    light_cascade.len()
                }
            });

        // Error or threshold exceeded -> Heavy Cascade
        if error_count > 0 || turn >= max_light_turns {
            if !heavy_cascade.is_empty() {
                let idx = error_count.saturating_sub(1) % heavy_cascade.len();
                heavy_cascade[idx].clone()
            } else {
                get_heavy_model()
            }
        } else {
            // Standard Tier -> Light Cascade
            if !light_cascade.is_empty() {
                let idx = turn % light_cascade.len();
                light_cascade[idx].clone()
            } else {
                get_flash_lite_model()
            }
        }
    }

    /// Failover: advance through the active cascade or fallback to NEXUS_MODEL
    pub fn failover_model(current_model: &str) -> String {
        if let Some(pinned) = get_pinned_model() {
            return pinned;
        }

        let light_cascade = parse_cascade("NEXUS_LIGHT_CASCADE");
        let heavy_cascade = parse_cascade("NEXUS_HEAVY_CASCADE");

        if let Some(pos) = light_cascade.iter().position(|m| m == current_model) {
            if pos + 1 < light_cascade.len() {
                return light_cascade[pos + 1].clone();
            } else if !heavy_cascade.is_empty() {
                return heavy_cascade[0].clone();
            }
        }

        if let Some(pos) = heavy_cascade.iter().position(|m| m == current_model) {
            if pos + 1 < heavy_cascade.len() {
                return heavy_cascade[pos + 1].clone();
            }
        }

        get_global_model()
    }
}

impl Default for AgentFactory {
    fn default() -> Self {
        Self::new()
    }
}
