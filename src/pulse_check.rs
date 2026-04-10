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


use crate::resonator; // SUTURE: Changed from 'mod' to 'use crate' to resolve E0583

pub fn execute() {
    println!(">> [NEXUS] INITIATING HIVE PULSE CHECK...");

    // Zig NPU pulse via Resonator Core
    let _ = resonator::ResonatorCore::ignite_vault("vault/pulse");

    let thermal = resonator::ResonatorCore::read_thermal_temp();
    println!(">> [STATUS] THERMAL RESONANCE: {:.1}°C", thermal);

    if thermal > 0.0 {
        println!(">> [SUCCESS] THE HIVE IS ALIVE.");
    } else {
        println!(">> [WARNING] FRAGMENTATION DETECTED IN SENSORY STRANDS.");
    }
}
