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
