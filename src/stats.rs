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


use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;

#[derive(Serialize, Deserialize, Default, Debug)]
pub struct ForgeStats {
    pub file_metrics: HashMap<String, FileMetric>,
}

#[derive(Serialize, Deserialize, Default, Debug, Clone)]
pub struct FileMetric {
    pub successes: u32,
    pub failures: u32,
}

impl ForgeStats {
    pub fn load() -> Self {
        fs::read_to_string(".forge_stats.json")
            .ok()
            .and_then(|content| serde_json::from_str(&content).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) {
        if let Ok(content) = serde_json::to_string_pretty(self) {
            let _ = fs::write(".forge_stats.json", content);
        }
    }

    pub fn record(&mut self, file: &str, success: bool) {
        let metric = self.file_metrics.entry(file.to_string()).or_default();
        if success {
            metric.successes += 1;
        } else {
            metric.failures += 1;
        }
        self.save();
    }

    pub fn get_success_rate(&self, file: &str) -> f32 {
        if let Some(m) = self.file_metrics.get(file) {
            let total = m.successes + m.failures;
            if total == 0 {
                return 100.0;
            }
            return (m.successes as f32 / total as f32) * 100.0;
        }
        100.0
    }
}
