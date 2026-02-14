use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

pub fn find_target_file() -> PathBuf {
    // 1. Load the ignore list
    let ignore_list: Vec<String> = fs::read_to_string(".forgeignore")
        .unwrap_or_default()
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .collect();

    let paths = fs::read_dir("src/").unwrap();
    let mut target = PathBuf::from("src/hiram_core.rs");
    let mut oldest_time = SystemTime::now();

    for entry in paths.flatten() {
        let path = entry.path();
        let path_str = path.to_str().unwrap_or("");

        // 2. SKIP if the file is in the ignore list
        if ignore_list.iter().any(|ignored| path_str.contains(ignored)) {
            continue;
        }

        // 3. Process only valid extensions
        if let Some(ext) = path.extension() {
            if ext == "rs" || ext == "py" || ext == "js" {
                if let Ok(metadata) = fs::metadata(&path) {
                    if let Ok(modified) = metadata.modified() {
                        if modified < oldest_time {
                            oldest_time = modified;
                            target = path;
                        }
                    }
                }
            }
        }
    }
    target
}

pub fn get_manual_override() -> Option<PathBuf> {
    let output = std::process::Command::new("git")
        .args(&["log", "-1", "--pretty=%B"])
        .output()
        .ok()?;

    let message = String::from_utf8_lossy(&output.stdout);
    
    if message.contains("[FORCE:") {
        let start = message.find("[FORCE:").unwrap() + 7;
        let end = message[start..].find(']').unwrap() + start;
        let target = message[start..].trim_end_matches(']').trim();
        let path = PathBuf::from(target);
        if path.exists() {
            return Some(path);
        }
    }
    None
}
