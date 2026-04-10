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


use std::fs;

pub fn is_fit_for_work() -> bool {
    // Check Load Average to prevent overheating on the Duet
    if let Ok(load_str) = fs::read_to_string("/proc/loadavg") {
        let load: f32 =
            load_str.split_whitespace().next().and_then(|s| s.parse().ok()).unwrap_or(0.0);

        // Throttle if load > 4.0 (Quad Core saturation)
        if load > 4.0 {
            return false;
        }
    }
    true
}
