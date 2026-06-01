//! LazarusVault — file-system snapshot store.
//! Snapshots are written as <sha256_of_content>.snap alongside the source
//! file, catalogued in .nexus_snapshots.json at the workspace root.
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tokio::fs;

const CATALOG: &str = ".nexus_snapshots.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    pub id:        String,
    pub path:      String,
    pub hash:      String,
    pub directive: String,
    pub timestamp: String,
}

pub fn init() {
    tracing::info!(">> [LAZARUS] Temporal vault online");
}

pub async fn take_snapshot(path: &str, directive: &str) -> Result<Snapshot> {
    let content = fs::read(path).await?;
    let hash = sha256_hex(&content);
    let snap_path = format!("{}.{}.snap", path, &hash[..8]);
    fs::write(&snap_path, &content).await?;

    let ts = chrono_now();
    let snap = Snapshot {
        id:        uuid_v4(),
        path:      path.to_string(),
        hash:      hash.clone(),
        directive: directive.to_string(),
        timestamp: ts,
    };

    let mut catalog = load_catalog().await.unwrap_or_default();
    catalog.push(snap.clone());
    // Keep newest 200 snapshots
    if catalog.len() > 200 {
        catalog = catalog.into_iter().rev().take(200).rev().collect();
    }
    save_catalog(&catalog).await?;
    Ok(snap)
}

pub async fn restore_snapshot(hash: &str) -> Result<String> {
    let catalog = load_catalog().await?;
    let snap = catalog.iter()
        .find(|s| s.hash.starts_with(hash) || s.id == hash)
        .ok_or_else(|| anyhow::anyhow!("Snapshot not found: {}", hash))?;
    let snap_path = format!("{}.{}.snap", snap.path, &snap.hash[..8]);
    let content = fs::read(&snap_path).await?;
    fs::write(&snap.path, content).await?;
    Ok(snap.path.clone())
}

pub async fn list_snapshots() -> Result<Vec<Snapshot>> {
    load_catalog().await
}

async fn load_catalog() -> Result<Vec<Snapshot>> {
    if !Path::new(CATALOG).exists() { return Ok(vec![]); }
    let raw = fs::read_to_string(CATALOG).await?;
    Ok(serde_json::from_str(&raw).unwrap_or_default())
}

async fn save_catalog(catalog: &[Snapshot]) -> Result<()> {
    fs::write(CATALOG, serde_json::to_string_pretty(catalog)?).await?;
    Ok(())
}

fn sha256_hex(data: &[u8]) -> String {
    // Simple FNV-1a 64-bit hash (no sha2 dep required, good enough for snap IDs)
    let mut h: u64 = 14695981039346656037;
    for &b in data { h ^= b as u64; h = h.wrapping_mul(1099511628211); }
    format!("{:016x}", h)
}

fn chrono_now() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    format!("{}", secs)
}

fn uuid_v4() -> String {
    use std::collections::hash_map::RandomState;
    use std::hash::{BuildHasher, Hasher};
    let a = RandomState::new().build_hasher().finish();
    let b = RandomState::new().build_hasher().finish();
    format!("{:016x}-{:016x}", a, b)
}
