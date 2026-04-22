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


// src/bin/ui_audit.rs - Native UI Build Interrogator
fn main() {
    println!(">> [INQUISITOR] Initiating UI Build Audit...");
    match nexus_singularity::inquisitor::Inquisitor::audit_ui() {
        Ok(logs) => {
            println!("--- RAW VITE/SVELTE LOGS START ---");
            println!("{}", logs);
            println!("--- RAW VITE/SVELTE LOGS END ---");
        }
        Err(e) => {
            eprintln!("!! [FRACTURE] UI Audit failed: {}", e);
        }
    }
}
