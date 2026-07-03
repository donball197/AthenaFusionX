use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::LazyLock;

#[derive(Debug)]
pub struct NexusMetrics {
    pub tasks_total: AtomicU64,
    pub task_failures: AtomicU64,
    pub arbiter_blocks: AtomicU64,
    pub heal_successes: AtomicU64,
    pub heal_failures: AtomicU64,
    pub api_latency_ms_sum: AtomicU64,
    pub api_calls: AtomicU64,
    pub circuit_opens: AtomicU64,
}

impl Default for NexusMetrics {
    fn default() -> Self {
        Self::new()
    }
}

impl NexusMetrics {
    pub const fn new() -> Self {
        Self {
            tasks_total: AtomicU64::new(0),
            task_failures: AtomicU64::new(0),
            arbiter_blocks: AtomicU64::new(0),
            heal_successes: AtomicU64::new(0),
            heal_failures: AtomicU64::new(0),
            api_latency_ms_sum: AtomicU64::new(0),
            api_calls: AtomicU64::new(0),
            circuit_opens: AtomicU64::new(0),
        }
    }

    pub fn inc_task(&self) {
        tracing::info!("Task counter incremented");
        self.tasks_total.fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_failure(&self) {
        self.task_failures.fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_arbiter_block(&self) {
        self.arbiter_blocks.fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_heal_success(&self) {
        self.heal_successes.fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_heal_failure(&self) {
        self.heal_failures.fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_circuit_open(&self) {
        self.circuit_opens.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_api_call(&self, latency_ms: u128) {
        self.api_calls.fetch_add(1, Ordering::Relaxed);
        self.api_latency_ms_sum.fetch_add(latency_ms as u64, Ordering::Relaxed);
    }

    pub fn avg_latency_ms(&self) -> u64 {
        let calls = self.api_calls.load(Ordering::Relaxed);
        self.api_latency_ms_sum.load(Ordering::Relaxed).checked_div(calls).unwrap_or(0)
    }
}

pub static METRICS: LazyLock<NexusMetrics> = LazyLock::new(NexusMetrics::new);

pub fn get_metrics_text() -> String {
    let m = &*METRICS;
    format!(
        "# HELP nexus_tasks_total Total tasks processed by the NEXUS Cortex\n\
         # TYPE nexus_tasks_total counter\n\
         nexus_tasks_total {}\n\
         # HELP nexus_task_failures Total task execution failures\n\
         # TYPE nexus_task_failures counter\n\
         nexus_task_failures {}\n\
         # HELP nexus_arbiter_blocks Total arbiter safety interventions\n\
         # TYPE nexus_arbiter_blocks counter\n\
         nexus_arbiter_blocks {}\n\
         # HELP nexus_heal_successes Self-healing suture successes\n\
         # TYPE nexus_heal_successes counter\n\
         nexus_heal_successes {}\n\
         # HELP nexus_heal_failures Self-healing suture failures\n\
         # TYPE nexus_heal_failures counter\n\
         nexus_heal_failures {}\n\
         # HELP nexus_api_calls Total calls to Gemini API backend\n\
         # TYPE nexus_api_calls counter\n\
         nexus_api_calls {}\n\
         # HELP nexus_api_avg_latency_ms Average API latency in milliseconds\n\
         # TYPE nexus_api_avg_latency_ms gauge\n\
         nexus_api_avg_latency_ms {}\n\
         # HELP nexus_circuit_opens Circuit breaker trips\n\
         # TYPE nexus_circuit_opens counter\n\
         nexus_circuit_opens {}\n",
        m.tasks_total.load(Ordering::Relaxed),
        m.task_failures.load(Ordering::Relaxed),
        m.arbiter_blocks.load(Ordering::Relaxed),
        m.heal_successes.load(Ordering::Relaxed),
        m.heal_failures.load(Ordering::Relaxed),
        m.api_calls.load(Ordering::Relaxed),
        m.avg_latency_ms(),
        m.circuit_opens.load(Ordering::Relaxed),
    )
}
