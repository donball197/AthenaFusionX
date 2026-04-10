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
use std::process::Command;

pub fn get_system_vitals() -> String {
    let ram = Command::new("free")
        .arg("-m")
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
        .unwrap_or_default();
    let disk = Command::new("df")
        .arg("-h")
        .arg("/")
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
        .unwrap_or_default();
    let cargo = fs::read_to_string("Cargo.toml").unwrap_or_default();

    format!(
        "--- HARDWARE LIMITS (MOTOROLA G PRIMARY) ---\n{}\n{}\n\n--- ENGINE LIMITS ---\n{}",
        ram, disk, cargo
    )
}
