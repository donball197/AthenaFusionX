use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::fs;
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

    pub async fn auto_snapshot(path: &str, _tag: &str) -> anyhow::Result<Snapshot> {
        let content = fs::read(path).await?;
        let mut hasher = Sha256::new();
        hasher.update(&content);
        let hash = hasher.finalize().iter().map(|b| format!("{:02x}", b)).collect::<String>();

        let vault = Path::new(Self::VAULT_DIR);
        if !vault.exists() { fs::create_dir_all(vault).await?; }

        let backup_path = vault.join(format!("{}.bak", &hash[..12]));
        fs::write(&backup_path, &content).await?;

        let snap = Snapshot {
            hash: hash.clone(),
            path: path.to_string(),
            backup_path: backup_path.to_string_lossy().to_string(),
            timestamp: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_secs(),
        };

        let mut snaps = Self::list_snapshots().await.unwrap_or_default();
        snaps.retain(|s| s.hash != hash);
        snaps.push(snap.clone());
        if snaps.len() > 50 { snaps.remove(0); }

        let serialized = serde_json::to_string_pretty(&snaps)?;
        fs::write(Self::LEDGER, serialized).await?;

        // ⚡ SOVEREIGN LOG: Cloud sync disabled to maintain air-gapped integrity.
        println!("\x1b[32m>> [LAZARUS] Local Snapshot Secured: {}\x1b[0m", &hash[..12]);

        Ok(snap)
    }

    pub async fn list_snapshots() -> anyhow::Result<Vec<Snapshot>> {
        let p = Path::new(Self::LEDGER);
        if !p.exists() { return Ok(vec![]); }
        let data = fs::read_to_string(p).await?;
        match serde_json::from_str::<Vec<Snapshot>>(&data) {
            Ok(snaps) => Ok(snaps),
            Err(_) => Ok(vec![]),
        }
    }

    pub async fn restore_snapshot(hash_prefix: &str) -> anyhow::Result<String> {
        let snaps = Self::list_snapshots().await?;
        let snap = snaps.iter().find(|s| s.hash.starts_with(hash_prefix))
            .ok_or_else(|| anyhow::anyhow!("Snapshot not found"))?;
        let content = fs::read(&snap.backup_path).await?;
        fs::write(&snap.path, content).await?;
        Ok(format!(">> [LAZARUS] Restored {} from {}", snap.path, &snap.hash[..8]))
    }
}
