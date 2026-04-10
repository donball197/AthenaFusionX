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


use anyhow::{Context, Result, anyhow};
use std::path::PathBuf;
use crate::cli::{detect_host_muscle, HostMuscle};

pub struct DynamicBroker;

impl DynamicBroker {
    /// seat_library: Determines the correct library path based on detected Host Muscle.
    /// This prevents binary fragmentation between Motorola (Termux) and Chromebook (Linux).
    pub fn get_library_path(lib_name: &str) -> Result<PathBuf> {
        let muscle = detect_host_muscle();
        let workspace_root = std::env::current_dir().context("Failed to get workspace root")?;
        
        let sub_dir = match muscle {
            HostMuscle::MotorolaTermux => "lib/motorola",
            HostMuscle::ChromebookCrostini => "lib/chromebook",
            HostMuscle::GenericARM64 => "lib/generic",
        };

        let lib_path = workspace_root.join(sub_dir).join(lib_name);

        if lib_path.exists() {
            println!(">> [BROKER] Library seated: {:?}", lib_path);
            Ok(lib_path)
        } else {
            Err(anyhow!(
                "FRAGMENTATION_FAULT: Library '{}' not found for seated muscle in {:?}", 
                lib_name, 
                sub_dir
            ))
        }
    }
}
