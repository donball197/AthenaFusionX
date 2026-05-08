pub const ALIAS: &str = "WASM_CELL_READY";

pub fn extract(results: &[String]) -> (Vec<String>, Option<String>) {
    let mut clean = Vec::with_capacity(results.len());
    let mut b64: Option<String> = None;

    for r in results {
        if let Some(payload) = r.strip_prefix("GENESIS_PAYLOAD:") {
            b64 = Some(payload.to_string());
            // Replace giant base64 with a lightweight alias in the conversation history
            clean.push(format!("WASM_SYNTHESIS_COMPLETE → alias={}", ALIAS));
        } else {
            clean.push(r.clone());
        }
    }

    (clean, b64)
}

pub fn resolve_b64(alias_or_real: &str, stored_b64: Option<&str>) -> Option<String> {
    if alias_or_real == ALIAS {
        stored_b64.map(|s| s.to_string())
    } else {
        Some(alias_or_real.to_string())
    }
}
