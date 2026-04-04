use std::path::PathBuf;
use std::fs;

pub fn validate_path(user_input: &str) -> Result<PathBuf, String> {
    let base = std::env::current_dir()
        .map_err(|_| "Failed to get current dir")?;
    
    // Join and Canonicalize (Resolves all ../ and symlinks)
    let full_path = base.join(user_input);
    
    // If the file exists, we can canonicalize it directly. 
    // If not, we validate the parent directory safely.
    let absolute = if full_path.exists() {
        full_path.canonicalize()
    } else {
        let parent = full_path.parent().ok_or("Invalid path structure")?;
        if !parent.exists() { let _ = fs::create_dir_all(parent); }
        
        // SHIELD: Safe extraction to prevent unwrap() panic on malformed paths
        let file_name = full_path.file_name()
            .ok_or_else(|| "Invalid path: Missing target filename".to_string())?;
            
        parent.canonicalize().map(|p| p.join(file_name))
    }.map_err(|e| format!("Path error: {}", e))?;

    // ENFORCE THE JAIL: Path MUST start with project root
    if !absolute.starts_with(&base) {
        return Err("SECURITY ALERT: Path Traversal Blocked".to_string());
    }

    // BLOCK SENSITIVE FILES
    if let Some(name) = absolute.file_name() {
        let n = name.to_string_lossy().to_lowercase();
        if n == ".env" || n == "cargo.toml" || n.contains("audit") {
            return Err("RESERVED FILE: Access Denied".to_string());
        }
    }

    Ok(absolute)
}
