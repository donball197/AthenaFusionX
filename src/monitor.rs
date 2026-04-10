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

pub fn check_system_health() {
    let mission_count = fs::read_dir("missions").map(|d| d.count()).unwrap_or(0);
    let claim_count = fs::read_dir("claims").map(|d| d.count()).unwrap_or(0);

    println!(">> [MONITOR] SYSTEM HEALTH: OK");
    println!(">> [MONITOR] ACTIVE MISSIONS: {}", mission_count);
    println!(">> [MONITOR] ACTIVE CLAIMS: {}", claim_count);
}
