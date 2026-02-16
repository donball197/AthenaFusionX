use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub struct Mission {
    pub path: PathBuf,
    pub objective: String,
}

pub fn get_unclaimed_mission(node_id: &str) -> Option<Mission> {
    let missions = fs::read_dir("missions").ok()?;
    
    for entry in missions.flatten() {
        let path = entry.path();
        
        // Skip directories or non-mission files
        if path.is_dir() { continue; }

        let file_name = path.file_name()?.to_str()?;
        let claim_path = format!("claims/{}.lock", file_name);

        // STRENGTHENED CHECK: If the lock file exists, skip immediately.
        if !Path::new(&claim_path).exists() {
            // Attempt to write our ID to the lock
            if fs::write(&claim_path, node_id).is_ok() {
                let objective = fs::read_to_string(&path).unwrap_or_default();
                println!(">> [{}] SUCCESSFULLY CLAIMED: {}", node_id, file_name);
                return Some(Mission { path, objective });
            }
        }
    }
    None
}

pub fn resolve_mission(mission: Mission) {
    let file_name = mission.path.file_name().unwrap().to_str().unwrap().to_string();
    
    // Clean up both the mission and the lock
    let _ = fs::remove_file(&mission.path);
    let _ = fs::remove_file(format!("claims/{}.lock", file_name));
    println!(">> [BROKER] MISSION RESOLVED AND DELETED: {}", file_name);
}
