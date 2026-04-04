use axum::{extract::State, response::IntoResponse, Json};
use serde_json::json;
use std::sync::Arc;
use std::fs;
use crate::server::AppState;
use crate::lazarus_vault::LazarusVault;
use crate::diff::DiffEngine;

pub async fn terminal_command(
    State(state): State<Arc<AppState>>, 
    Json(payload): Json<serde_json::Value>
) -> impl IntoResponse {
    let cmd_line = payload["command"].as_str().unwrap_or("").trim();
    if cmd_line.is_empty() { return "".into_response(); }

    let parts: Vec<&str> = cmd_line.split_whitespace().collect();
    let command = parts[0].to_lowercase();

    let response = match command.as_str() {
        "open" => {
            let file = parts.get(1).unwrap_or(&"");
            json!({"type": "action", "action": "OPEN_FILE", "file": file}).to_string()
        },
        "close" => {
            let file = parts.get(1).unwrap_or(&"");
            json!({"type": "action", "action": "CLOSE_FILE", "file": file}).to_string()
        },
        "ls" | "dir" => {
            let target = parts.get(1).unwrap_or(&".");
            let mut out = format!(">> DIRECTORY LISTING [{}]:\n", target);
            if let Ok(rd) = std::fs::read_dir(target) {
                for entry in rd.flatten() {
                    let name = entry.file_name().to_string_lossy().to_string();
                    if !name.starts_with('.') && name != "target" { 
                        let suffix = if entry.path().is_dir() { "/" } else { "" };
                        out.push_str(&format!("   {}{}\n", name, suffix)); 
                    }
                }
            } else {
                out.push_str("   [Directory not found or inaccessible]\n");
            }
            json!({"type": "output", "data": out}).to_string()
        },
        "status" => {
            let v = state.monitor.get_vitals();
            json!({"type": "output", "data": format!(">> STATUS | TEMP: {:.1}C | CPU: {:.1}% | RAM: {:.2}GB", v.temperature, v.cpu_load, v.ram_used_gb)}).to_string()
        },
        "diff" => {
            if let Some(hash) = parts.get(1) {
                let snapshots = LazarusVault::list_snapshots().unwrap_or_default();
                if let Some(snap) = snapshots.iter().find(|s| s.hash.starts_with(hash)) {
                    if let Ok(current) = fs::read_to_string(&snap.path) {
                        let snapshot_content = fs::read_to_string(&snap.backup_path).unwrap_or_default();
                        let unified = DiffEngine::unified_diff(&snapshot_content, &current, &snap.path);
                        json!({"type": "diff_view", "diff": unified, "hash": hash}).to_string()
                    } else {
                        json!({"type": "output", "data": format!(">> ERR: File {} missing.", snap.path)}).to_string()
                    }
                } else {
                    json!({"type": "output", "data": format!(">> ERR: Hash {} not found.", hash)}).to_string()
                }
            } else {
                json!({"type": "output", "data": ">> Usage: diff <hash>"}).to_string()
            }
        },
        "lazarus" | "history" => {
            match LazarusVault::list_snapshots() {
                Ok(snaps) => {
                    let mut out = format!(">> ARCHIVES: {} items\n", snaps.len());
                    for s in snaps.iter().rev().take(5) { out.push_str(&format!("   {} | {}\n", &s.hash[..8], s.path)); }
                    json!({"type": "output", "data": out}).to_string()
                },
                _ => json!({"type": "output", "data": ">> Vault Empty"}).to_string()
            }
        },
        "clear" => json!({"type": "action", "action": "CLEAR_TERMINAL"}).to_string(),
        _ => json!({"type": "output", "data": format!(">> Directive '{}' not recognized.", command)}).to_string(),
    };

    format!("{}\n", response).into_response()
}
