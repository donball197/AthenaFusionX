/**
 * Copyright 2026 Donald Ball
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 * http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR REPRESENTATIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */


// src/lazarus_vault.rs - v4.1 Restored listing + SHA256 snapshots
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::Path;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Snapshot {
    pub hash: String,
    pub path: String,
    pub backup_path: String,
    pub timestamp: u64,
}

pub struct LazarusVault;

impl LazarusVault {
    const VAULT_DIR: &'static str = ".lazarus_vault";
    const LEDGER: &'static str = ".lazarus_vault/ledger.json";

    pub fn auto_snapshot(path: &str, _tag: &str) -> anyhow::Result<Snapshot> {
        let content = fs::read(path)?;
        let mut hasher = Sha256::new();
        hasher.update(&content);
        let hash = hasher.finalize().iter().map(|b| format!("{:02x}", b)).collect::<String>();

        let vault = Path::new(Self::VAULT_DIR);
        if !vault.exists() {
            fs::create_dir_all(vault)?;
        }

        let backup_path = vault.join(format!("{}.bak", &hash[..12]));
        fs::write(&backup_path, &content)?;

        let snap = Snapshot {
            hash: hash.clone(),
            path: path.to_string(),
            backup_path: backup_path.to_string_lossy().to_string(),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_secs(),
        };

        // Append to ledger
        let mut snaps = Self::list_snapshots().unwrap_or_default();
        snaps.retain(|s| s.hash != hash); // deduplicate
        snaps.push(snap.clone());
        if snaps.len() > 50 {
            snaps.drain(0..snaps.len() - 50);
        }
        fs::write(Self::LEDGER, serde_json::to_string_pretty(&snaps)?)?;

        Ok(snap)
    }

    pub fn list_snapshots() -> anyhow::Result<Vec<Snapshot>> {
        let p = Path::new(Self::LEDGER);
        if !p.exists() {
            return Ok(vec![]);
        }
        let data = fs::read_to_string(p)?;
        Ok(serde_json::from_str(&data).unwrap_or_default())
    }

    pub fn restore_snapshot(hash_prefix: &str) -> anyhow::Result<String> {
        let snaps = Self::list_snapshots()?;
        let snap = snaps
            .iter()
            .find(|s| s.hash.starts_with(hash_prefix))
            .ok_or_else(|| anyhow::anyhow!("Snapshot not found: {}", hash_prefix))?;
        let content = fs::read(&snap.backup_path)?;
        fs::write(&snap.path, content)?;
        Ok(format!(">> [LAZARUS] Restored {} from {}", snap.path, &snap.hash[..8]))
    }
}
