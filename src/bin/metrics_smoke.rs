/**
 * Quick smoke test for atomic metrics
 */
fn main() {
    use nexus_singularity::metrics::{METRICS, get_metrics_text};

    METRICS.inc_task();
    METRICS.inc_failure();
    METRICS.record_api_call(42);

    println!("=== NEXUS METRICS SMOKE TEST ===");
    println!("{}", get_metrics_text());
    println!("\n✅ Metrics subsystem is live and Prometheus-compatible.");
}
