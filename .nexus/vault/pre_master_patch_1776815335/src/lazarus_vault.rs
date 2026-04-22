// src/lazarus_vault.rs — v3.4 SOVEREIGN TEMPORAL VAULT
use std::fs;
use std::path::Path;
use std::time::SystemTime;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use serde::{Serialize, Deserialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Snapshot {
    pub id: String,
    pub path: String,
    pub hash: String,
    pub timestamp: String,
    pub directive: String,
}

pub struct LazarusVault;

impl LazarusVault {
    /// auto_snapshot: Async wrapper for the executor FSM.
    pub async fn auto_snapshot(path: &str, tag: &str) -> Result<Snapshot, String> {
        Self::snapshot_file(path, tag)
    }

    pub fn snapshot_file(filepath: &str, directive_tag: &str) -> Result<Snapshot, String> {
        let content = fs::read_to_string(filepath).map_err(|e| format!("FS_READ_FAULT: {}", e))?;
        
        // ⚡ NATIVE DNA: Using standard library hasher to avoid external dependencies
        let mut hasher = DefaultHasher::new();
        content.hash(&mut hasher);
        let hash = format!("{:x}", hasher.finish());

        let vault_dir = ".nexus/vault";
        fs::create_dir_all(vault_dir).map_err(|e| format!("DIR_FAULT: {}", e))?;

        let dest_path = format!("{}/{}", vault_dir, hash);
        fs::write(&dest_path, &content).map_err(|e| format!("FS_WRITE_FAULT: {}", e))?;

        let snap = Snapshot {
            id: hash.clone(),
            path: filepath.to_string(),
            hash: hash.clone(),
            timestamp: format!("{:?}", SystemTime::now()),
            directive: directive_tag.to_string(),
        };
        
        let ledger_path = format!("{}/ledger.json", vault_dir);
        let mut ledger: Vec<Snapshot> = vec![];
        if Path::new(&ledger_path).exists() {
            if let Ok(data) = fs::read_to_string(&ledger_path) {
                ledger = serde_json::from_str(&data).unwrap_or_default();
            }
        }
        ledger.push(snap.clone());
        fs::write(ledger_path, serde_json::to_string_pretty(&ledger).unwrap()).ok();

        Ok(snap)
    }

    pub fn list_snapshots() -> Vec<Snapshot> {
        let ledger_path = ".nexus/vault/ledger.json";
        if Path::new(ledger_path).exists() {
            let data = fs::read_to_string(ledger_path).unwrap_or_default();
            serde_json::from_str(&data).unwrap_or_default()
        } else {
            vec![]
        }
    }
}
