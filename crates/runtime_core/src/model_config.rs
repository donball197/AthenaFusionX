//! Model cascade helpers — thin re-export of the single source of truth.
//! Prefer `crate::provider::{get_light_models, get_heavy_models, resolve_provider}`.

pub use crate::provider::{
    get_heavy_models, get_light_models, get_planner_models, resolve_provider, trip_circuit,
    AuthStyle, ProviderResolved,
};

// Keep old circuit-breaker entry points working for any remaining callers.
pub fn mark_rate_limited(model_name: &str) {
    trip_circuit(model_name);
}

/// Backward-compatible alias used by epistemic_engine rate-limit paths.
pub fn report_rate_limit(model_name: &str) {
    trip_circuit(model_name);
}
