// src/lazarus_vault.rs - Hardened Backup System
use serde::{Serialize, Deserialize};
use std::fs;
use std::path::Path;
use std::collections::VecDeque;
use chrono::Utc;
use sha2::{Sha256, Digest};
use anyhow::{Result, Context};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Snapshot {
    pub timestamp: String,
    pub path: String,
    pub author: String,
    pub hash: String,
    pub backup_path: String,
}

pub struct LazarusVault;

impl LazarusVault {
    const MAX_HISTORY: usize = 50;

    pub fn list_snapshots() -> Result<VecDeque<Snapshot>> {
        let ledger_path = Path::new(".lazarus/ledger.json");
        if !ledger_path.exists() {
            return Ok(VecDeque::new());
        }
        let data = fs::read_to_string(ledger_path).context("Vault ledger read failed")?;
        serde_json::from_str(&data).context("Ledger corruption")
    }

    pub fn auto_snapshot(target: &str, reason: &str) -> Result<String> {
        let content = fs::read_to_string(target).unwrap_or_default();
        Self::seal_patch(target, &content, reason)
    }

    pub fn seal_patch(target: &str, content: &str, reason: &str) -> Result<String> {
        let root = Path::new(".lazarus/snapshots");
        fs::create_dir_all(root)?;

        let mut hasher = Sha256::new();
        hasher.update(content);
        let hash = format!("{:x}", hasher.finalize());

        let timestamp = Utc::now().to_rfc3339();
        let backup_path = root.join(format!("{}.snap", Utc::now().timestamp_micros()));

        fs::write(&backup_path, content)?;

        let mut ledger = Self::list_snapshots().unwrap_or_default();
        if ledger.len() >= Self::MAX_HISTORY {
            ledger.pop_front();
        }

        ledger.push_back(Snapshot {
            timestamp,
            path: target.to_string(),
            author: format!("AUTO: {}", reason),
            hash: hash.clone(),
            backup_path: backup_path.to_string_lossy().to_string(),
        });

        fs::write(".lazarus/ledger.json", serde_json::to_string_pretty(&ledger)?)?;
        Ok(hash)
    }

    pub fn restore_snapshot(hash_prefix: &str) -> Result<String> {
        let ledger = Self::list_snapshots()?;
        let target_snap = ledger.iter()
            .find(|s| s.hash.starts_with(hash_prefix))
            .context("Snapshot not found")?;

        let safe_content = fs::read_to_string(&target_snap.backup_path)?;
        fs::write(&target_snap.path, safe_content)?;

        Ok(format!(">> [LAZARUS] Restored {} from snapshot {}", target_snap.path, hash_prefix))
    }
}
