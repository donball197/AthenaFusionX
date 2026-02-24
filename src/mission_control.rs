use std::fs;

pub struct MissionControl;

impl MissionControl {
    pub fn get_active_directives() -> String {
        let mut directives = String::from("\n>> [CURRENT DIRECTIVES]:\n");
        if let Ok(entries) = fs::read_dir("missions") {
            for entry in entries.flatten() {
                if let Ok(content) = fs::read_to_string(entry.path()) {
                    let filename = entry.file_name().to_string_lossy().to_string();
                    directives.push_str(&format!("- {}: {}\n", filename, content.lines().next().unwrap_or("No description")));
                }
            }
        }
        directives
    }
}
