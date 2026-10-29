use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tokio::fs;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    pub id: String,
    pub path: String,
    pub hash: String,
    pub directive: String,
    pub timestamp: String,
}

const CATALOG: &str = ".nexus_snapshots.json";

pub async fn list_snapshots() -> Result<Vec<Snapshot>> {
    if !Path::new(CATALOG).exists() {
        return Ok(vec![]);
    }
    let raw_json = fs::read_to_string(CATALOG).await?;
    let snapshots: Vec<Snapshot> = serde_json::from_str(&raw_json)?;
    Ok(snapshots)
}

pub async fn take_snapshot(path: &str, directive: &str) -> Result<Snapshot> {
    let content = fs::read(path).await?;
    let hash = fnv_hex(&content);
    let snap_path = format!("{}.{}.snap", path, &hash[..8]);
    fs::write(&snap_path, &content).await?;

    let snap = Snapshot {
        id: uuid_v4(),
        path: path.to_string(),
        hash: hash.clone(),
        directive: directive.to_string(),
        timestamp: epoch_secs().to_string(),
    };

    let mut catalog = list_snapshots().await.unwrap_or_default();
    catalog.push(snap.clone());
    if catalog.len() > 200 {
        catalog.remove(0);
    }
    fs::write(CATALOG, serde_json::to_string_pretty(&catalog)?).await?;
    Ok(snap)
}

fn fnv_hex(data: &[u8]) -> String {
    let mut h: u64 = 14695981039346656037;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(1099511628211);
    }
    format!("{:016x}", h)
}

fn epoch_secs() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs()
}

fn uuid_v4() -> String {
    format!(
        "{:x}",
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
    )
}
