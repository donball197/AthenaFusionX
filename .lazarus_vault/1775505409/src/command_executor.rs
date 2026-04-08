use std::process::Command;
use crate::system_monitor::SystemMonitor;

pub async fn execute_allowed_command(_m: &SystemMonitor, cmd: &str) -> Result<serde_json::Value, String> {
    let allowed = ["cargo", "npm", "ls", "echo", "grep", "find"];
    let parts: Vec<&str> = cmd.split_whitespace().collect();
    
    if parts.is_empty() { return Err("Empty command".into()); }
    
    if !allowed.contains(&parts[0]) {
        return Err(format!("SECURITY: Command '{}' is not in the Sovereign Whitelist.", parts[0]));
    }
    
    let output = Command::new(parts[0])
        .args(&parts[1..])
        .output()
        .map_err(|e| format!("IO Error: {}", e))?;
        
    Ok(serde_json::json!({
        "success": output.status.success(),
        "stdout": String::from_utf8_lossy(&output.stdout),
        "stderr": String::from_utf8_lossy(&output.stderr)
    }))
}
