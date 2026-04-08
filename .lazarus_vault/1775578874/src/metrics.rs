use prometheus::{Encoder, TextEncoder};

/// Ignite the baseline metrics registry
pub fn init_metrics() {
    // Placeholder for registering custom Prometheus counters/gauges later.
    // Currently a safe no-op to unblock the server route baseline.
}

/// Gathers all registered metrics and formats them for the /api/metrics route
pub fn get_metrics_text() -> String {
    let encoder = TextEncoder::new();
    let mut buffer = vec![];
    
    // Gather from the default global registry
    let metric_families = prometheus::gather();
    
    if let Err(e) = encoder.encode(&metric_families, &mut buffer) {
        eprintln!(">> [METRICS] Encoding failure: {}", e);
    }
    
    String::from_utf8_lossy(&buffer).to_string()
}
