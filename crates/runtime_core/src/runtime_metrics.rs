use std::sync::atomic::{AtomicU64, Ordering};

pub struct RuntimeMetrics {
    pub active_tasks: AtomicU64,
    pub crashed_agents: AtomicU64,
    pub emitted_events: AtomicU64,
}

impl RuntimeMetrics {
    pub fn new() -> Self {
        Self {
            active_tasks: AtomicU64::new(0),
            crashed_agents: AtomicU64::new(0),
            emitted_events: AtomicU64::new(0),
        }
    }

    pub fn inc_tasks(&self) {
        self.active_tasks.fetch_add(1, Ordering::Relaxed);
    }
}

impl Default for RuntimeMetrics {
    fn default() -> Self {
        Self::new()
    }
}
