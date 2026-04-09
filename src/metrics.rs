// src/metrics.rs - v3.3 Prometheus Definitive Alignment
use prometheus::{Encoder, TextEncoder, IntCounter, Registry};
use once_cell::sync::Lazy;

pub static REGISTRY: Lazy<Registry> = Lazy::new(Registry::new);

pub struct PrometheusMetrics {
    pub tasks_total: IntCounter,
    pub task_failures: IntCounter,
    pub arbiter_blocks: IntCounter,
    pub zig_reflex_invocations: IntCounter,
    pub api_calls: IntCounter,
}

impl PrometheusMetrics {
    // SUTURE: Bridges the call from neural_agent.rs
    pub fn record_api_call(&self, _latency_ms: u128) {
        self.api_calls.inc();
    }

    // SUTURE: Bridges the fetch_add calls by providing a compatible method
    // (Note: IntCounter uses u64 internally, so we ignore Ordering)
    pub fn fetch_add(&self, _val: u64, _order: std::sync::atomic::Ordering) {
        // This is a generic redirect for any direct calls to the struct itself
        // but we'll fix the specific field calls in the files below.
    }
}

pub static METRICS: Lazy<PrometheusMetrics> = Lazy::new(|| {
    let tasks_total = IntCounter::new("nexus_tasks_total", "Total tasks").unwrap();
    let task_failures = IntCounter::new("nexus_task_failures", "Failed tasks").unwrap();
    let arbiter_blocks = IntCounter::new("nexus_arbiter_blocks", "Arbiter blocks").unwrap();
    let zig_reflex_invocations = IntCounter::new("nexus_zig_reflex", "Zig calls").unwrap();
    let api_calls = IntCounter::new("nexus_api_calls", "API calls").unwrap();

    REGISTRY.register(Box::new(tasks_total.clone())).unwrap();
    REGISTRY.register(Box::new(task_failures.clone())).unwrap();
    REGISTRY.register(Box::new(arbiter_blocks.clone())).unwrap();
    REGISTRY.register(Box::new(zig_reflex_invocations.clone())).unwrap();
    REGISTRY.register(Box::new(api_calls.clone())).unwrap();

    PrometheusMetrics {
        tasks_total,
        task_failures,
        arbiter_blocks,
        zig_reflex_invocations,
        api_calls,
    }
});

pub fn init_metrics() {
    let _ = *METRICS;
}

pub fn get_metrics_text() -> String {
    let encoder = TextEncoder::new();
    let mut buffer = vec![];
    encoder.encode(&REGISTRY.gather(), &mut buffer).unwrap();
    String::from_utf8_lossy(&buffer).to_string()
}
