use std::fs;
use std::path::{Path, PathBuf};

pub struct Mission {
    pub path: PathBuf,
    pub objective: String,
}

pub fn get_unclaimed_mission(node_id: &str) -> Option<Mission> {
    let missions = fs::read_dir("missions").ok()?;
    
    for entry in missions.flatten() {
        let path = entry.path();
        let file_name = path.file_name()?.to_str()?;
        let claim_path = format!("claims/{}.lock", file_name);

        // If no one has claimed it, try to claim it
        if !Path::new(&claim_path).exists() {
            let _ = fs::write(&claim_path, node_id);
            let objective = fs::read_to_string(&path).unwrap_or_default();
            return Some(Mission { path, objective });
        }
    }
    None
}

pub fn resolve_mission(mission: Mission) {
    let file_name = mission.path.file_name().unwrap().to_str().unwrap();
    let _ = fs::remove_file(mission.path);
    let _ = fs::remove_file(format!("claims/{}.lock", file_name));
}
