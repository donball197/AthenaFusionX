// src/lazarus_vault.rs — v1.5 [DETERMINISTIC_LEDGER]
use crate::fnv_hasher::FnvHasher as DefaultHasher;
use serde::{Deserialize, Serialize};
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::Path;
use std::time::SystemTime;

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
    const VAULT_DIR: &'static str = ".nexus/vault";
    const LEDGER_PATH: &'static str = ".nexus/vault/ledger.json";

    pub async fn auto_snapshot(path: &str, tag: &str) -> Result<Snapshot, String> {
        let metadata = fs::metadata(path).map_err(|e| format!("PATH_NOT_FOUND: {}", e))?;
        if metadata.is_dir() { return Err("DIRECTORY_SNAPSHOT_NOT_SUPPORTED".into()); }
        let content = fs::read_to_string(path).map_err(|e| format!("FILE_READ_FAULT: {}", e))?;

        let mut hasher = DefaultHasher::new();
        content.hash(&mut hasher);
        let hash = format!("{:x}", hasher.finish());

        fs::create_dir_all(Self::VAULT_DIR).map_err(|e| format!("DIR_FAULT: {}", e))?;
        let dest = format!("{}/{}", Self::VAULT_DIR, hash);
        fs::write(&dest, &content).map_err(|e| format!("FS_WRITE_FAULT: {}", e))?;

        let snapshot = Snapshot {
            id: hash.clone(),
            path: path.to_string(),
            hash: hash.clone(),
            timestamp: format!("{:?}", SystemTime::now()),
            directive: tag.to_string(),
        };

        Self::append_to_ledger(&snapshot)?;
        println!("\x1b[32m[LAZARUS] Snapshot {} → {} [tag: {}]\x1b[0m", &hash[..8], path, tag);
        Ok(snapshot)
    }

    fn append_to_ledger(snapshot: &Snapshot) -> Result<(), String> {
        let mut entries = Self::list_snapshots();
        if entries.iter().any(|e| e.hash == snapshot.hash) { return Ok(()); }
        entries.push(snapshot.clone());
        let json = serde_json::to_string_pretty(&entries).map_err(|e| e.to_string())?;
        fs::write(Self::LEDGER_PATH, json).map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn list_snapshots() -> Vec<Snapshot> {
        if !Path::new(Self::LEDGER_PATH).exists() { return vec![]; }
        let data = fs::read_to_string(Self::LEDGER_PATH).unwrap_or_default();
        let mut entries: Vec<Snapshot> = serde_json::from_str(&data).unwrap_or_default();
        entries.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
        entries
    }
}
