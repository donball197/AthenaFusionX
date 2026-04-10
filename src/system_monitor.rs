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


// src/system_monitor.rs - v3.1 Real /proc readers restored
use crate::resonator::ResonatorCore;
use serde::Serialize;
use std::fs;

#[derive(Serialize, Clone, Debug)]
pub struct Vitals {
    pub temperature: f32,
    pub cpu_load: f32,
    pub ram_gb: f32,
}

#[derive(Clone, Debug, Default)]
pub struct SystemMonitor;

impl SystemMonitor {
    pub fn new() -> Self {
        SystemMonitor
    }

    pub fn get_vitals(&self) -> Vitals {
        let temperature = ResonatorCore::read_thermal_temp();

        // Real RAM from /proc/meminfo — available in GB
        let ram_gb = fs::read_to_string("/proc/meminfo")
            .ok()
            .and_then(|s| {
                s.lines()
                    .find(|l| l.starts_with("MemAvailable:"))
                    .and_then(|l| l.split_whitespace().nth(1))
                    .and_then(|v| v.parse::<f32>().ok())
                    .map(|kb| kb / 1024.0 / 1024.0)
            })
            .unwrap_or(1.4);

        // Real CPU load from /proc/loadavg — normalized to 4-core Duet
        let cpu_load = fs::read_to_string("/proc/loadavg")
            .ok()
            .and_then(|s| s.split_whitespace().next().and_then(|v| v.parse::<f32>().ok()))
            .map(|load| (load / 4.0).min(1.0))
            .unwrap_or(0.18);

        Vitals { temperature, cpu_load, ram_gb }
    }
}
