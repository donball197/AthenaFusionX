/**
 * Copyright 2026 Donald Ball
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 * http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR REPRESENTATIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */


// src/metrics.rs - v4.0 Atomic Sovereign (prometheus REMOVED permanently)
use once_cell::sync::Lazy;
use std::sync::atomic::{AtomicU64, Ordering};

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

impl NexusMetrics {
    const fn new() -> Self {
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

    pub fn record_api_call(&self, latency_ms: u128) {
        self.api_calls.fetch_add(1, Ordering::Relaxed);
        self.api_latency_ms_sum.fetch_add(latency_ms as u64, Ordering::Relaxed);
    }

    pub fn avg_latency_ms(&self) -> u64 {
        let calls = self.api_calls.load(Ordering::Relaxed);
        if calls == 0 {
            return 0;
        }
        self.api_latency_ms_sum.load(Ordering::Relaxed) / calls
    }

    pub fn inc(&self) {
        self.tasks_total.fetch_add(1, Ordering::Relaxed);
    }
}

pub static METRICS: Lazy<NexusMetrics> = Lazy::new(NexusMetrics::new);

pub fn init_metrics() {
    let _ = &*METRICS;
}

pub fn get_metrics_text() -> String {
    let m = &*METRICS;
    format!(
        "# NEXUS runtime metrics\n\
         nexus_tasks_total {}\n\
         nexus_task_failures {}\n\
         nexus_arbiter_blocks {}\n\
         nexus_heal_successes {}\n\
         nexus_heal_failures {}\n\
         nexus_circuit_opens {}\n\
         nexus_api_calls {}\n\
         nexus_api_avg_latency_ms {}\n",
        m.tasks_total.load(Ordering::Relaxed),
        m.task_failures.load(Ordering::Relaxed),
        m.arbiter_blocks.load(Ordering::Relaxed),
        m.heal_successes.load(Ordering::Relaxed),
        m.heal_failures.load(Ordering::Relaxed),
        m.circuit_opens.load(Ordering::Relaxed),
        m.api_calls.load(Ordering::Relaxed),
        m.avg_latency_ms(),
    )
}
