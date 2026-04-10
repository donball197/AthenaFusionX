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


use std::fs::File;
use std::io::Read;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

pub struct NanoAgent {
    pub load: Arc<AtomicU32>,
}

impl NanoAgent {
    pub fn inject() -> Self {
        let load = Arc::new(AtomicU32::new(0));
        let l_clone = load.clone();
        std::thread::spawn(move || {
            let (mut p_idle, mut p_total) = (0, 0);
            let mut buf = [0u8; 512];
            loop {
                if let Ok(mut f) = File::open("/proc/stat") {
                    let n = f.read(&mut buf).unwrap_or(0);
                    let s = String::from_utf8_lossy(&buf[..n]);
                    if let Some(line) = s.lines().next() {
                        let v: Vec<u64> = line.split_whitespace().filter_map(|s| s.parse().ok()).collect();
                        if v.len() >= 4 {
                            let total: u64 = v.iter().sum();
                            let idle = v[3];
                            let (td, id) = (total - p_total, idle - p_idle);
                            if td > 0 {
                                let usage = ((td - id) as f32 / td as f32) * 100.0;
                                l_clone.store(usage.to_bits(), Ordering::Relaxed);
                            }
                            p_total = total; p_idle = idle;
                        }
                    }
                }
                std::thread::sleep(std::time::Duration::from_millis(1000));
            }
        });
        Self { load }
    }

    pub async fn run_cycle(&self) -> anyhow::Result<String> {
        let usage = f32::from_bits(self.load.load(Ordering::Relaxed));
        Ok(format!("Telemetry Pulse: CPU at {:.2}%", usage))
    }
}

impl Default for NanoAgent {
    fn default() -> Self {
        Self::inject()
    }
}
