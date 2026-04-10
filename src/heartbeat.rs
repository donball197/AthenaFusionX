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


use chrono::{DateTime, Duration, Utc};
use std::process::Command;

pub fn check_cluster_sync() -> bool {
    let output = Command::new("gh")
        .args(&["issue", "view", "1", "--json", "comments", "-q", ".comments[-1].createdAt"])
        .output();

    if let Ok(out) = output {
        let timestamp_str = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if let Ok(last_heartbeat) = timestamp_str.parse::<DateTime<Utc>>() {
            let drift = Utc::now() - last_heartbeat;
            return drift < Duration::try_minutes(5).unwrap();
        }
    }
    false
}
