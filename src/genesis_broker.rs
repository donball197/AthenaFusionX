// src/genesis_broker.rs — v1.0 GENESIS Payload Broker
// ROOT CAUSE FIX: The GENESIS_PAYLOAD base64 string (~1.5KB) was being
// appended raw to session_obs. On subsequent turns the model saw it in
// context but had no consistent way to reference it without copying the
// entire string — causing either truncation or endless re-synthesis loops.
//
// Solution: strip GENESIS_PAYLOAD lines from session_obs and park the
// b64 in a separate field. Inject a short alias into context instead.

pub const ALIAS: &str = "WASM_CELL_READY";

/// extract: Scans a result vector for any GENESIS_PAYLOAD lines.
/// Returns (clean_results, Option<base64_string>).
/// If multiple synthesis results exist, the last one wins.
pub fn extract(results: &[String]) -> (Vec<String>, Option<String>) {
    let mut clean = Vec::with_capacity(results.len());
    let mut b64: Option<String> = None;

    for r in results {
        if let Some(payload) = r.strip_prefix("GENESIS_PAYLOAD:") {
            b64 = Some(payload.to_string());
            // Replace the raw b64 line with a short alias so the model
            // knows synthesis succeeded without seeing the giant string.
            clean.push(format!("WASM_SYNTHESIS_COMPLETE → alias={}", ALIAS));
        } else {
            clean.push(r.clone());
        }
    }

    (clean, b64)
}

/// inject_alias: Adds a short context hint so the model knows it can
/// call execute_distributed_wasm without embedding the raw b64.
pub fn inject_alias(obs: &mut Vec<String>, func_hint: &str) {
    obs.push(format!(
        "SYSTEM_HINT: WASM cell is ready. Use execute_distributed_wasm with \
         wasm_b64=\"{}\" func_name=\"{}\" input=<your_value> to run it.",
        ALIAS, func_hint
    ));
}

/// resolve_b64: Expands the WASM_CELL_READY alias back to the real b64
/// before dispatching to the executor. Called by executor.rs.
pub fn resolve_b64(alias_or_real: &str, stored_b64: Option<&str>) -> Option<String> {
    if alias_or_real == ALIAS || alias_or_real.is_empty() {
        stored_b64.map(|s| s.to_string())
    } else {
        Some(alias_or_real.to_string())
    }
}
