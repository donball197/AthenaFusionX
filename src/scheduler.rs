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


// src/scheduler.rs - v2.1 (Fixed cpu_load threshold: 0.0-1.0 scale)
use crate::memory::MemoryPalace;
use crate::orchestrator::Orchestrator;
use crate::system_monitor::SystemMonitor;
use serde_json::json;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc, Mutex};

#[derive(Debug, Clone)]
pub struct NexusTask {
    pub payload: String,
    pub priority: u8,
}

pub struct MasterSupervisor;

impl MasterSupervisor {
    pub fn ignite(
        tx: broadcast::Sender<String>,
        client: reqwest::Client,
        api_key: String,
        orchestrator: Arc<Orchestrator>,
        resonance_meter: Arc<AtomicU32>,
        mut monitor: SystemMonitor,
        memory: Arc<Mutex<MemoryPalace>>,
    ) -> mpsc::Sender<NexusTask> {
        let (task_tx, mut task_rx) = mpsc::channel::<NexusTask>(32);

        tokio::spawn(async move {
            let _ = tx.send(
                json!({
                    "type": "LOG_EVENT",
                    "payload": { "level": "INFO", "message": "Master Supervisor ignited." }
                })
                .to_string(),
            );

            while let Some(task) = task_rx.recv().await {
                let _current_resonance = resonance_meter.load(Ordering::Relaxed);
                let vitals = monitor.get_vitals();

                // FIX: cpu_load is 0.0-1.0 (normalized over 4 cores in system_monitor.rs)
                // The old check `> 75.0` would NEVER trigger. Correct threshold is 0.75.
                if vitals.cpu_load > 0.75 || vitals.temperature > 68.0 {
                    println!(
                        "\x1b[33m>> [SUPERVISOR] Throttling — CPU: {:.0}% Temp: {:.1}°C\x1b[0m",
                        vitals.cpu_load * 100.0,
                        vitals.temperature
                    );
                    tokio::time::sleep(tokio::time::Duration::from_secs(15)).await;
                    continue;
                }

                let cl = client.clone();
                let ak = api_key.clone();
                let orch = orchestrator.clone();
                let mem = memory.clone();
                let task_tx_clone = tx.clone();

                tokio::spawn(async move {
                    let _ = crate::task_manager::send_task(
                        orch.agent.clone(),
                        &cl,
                        &ak,
                        &task.payload,
                        mem,
                        task_tx_clone,
                    )
                    .await;
                });
            }
        });

        task_tx
    }
}
