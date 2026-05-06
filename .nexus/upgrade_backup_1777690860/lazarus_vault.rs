// src/lazarus_vault.rs — v1.4 [LEDGER_FIX]
// FIX: auto_snapshot() now appends every snapshot to ledger.json.
//      Previously it wrote the file to .nexus/vault/<hash> but never
//      touched the ledger, so list_snapshots() returned [] permanently
//      and SelfHealingEngine rollback was always blind.
// FIX: list_snapshots() returns entries sorted oldest→newest so
//      .last() in the rollback path gives the most recent stable state.
use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::Path;
use std::time::SystemTime;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Snapshot {
    pub id:        String,
    pub path:      String,
    pub hash:      String,
    pub timestamp: String,
    pub directive: String,
}

pub struct LazarusVault;

impl LazarusVault {
    const VAULT_DIR:  &'static str = ".nexus/vault";
    const LEDGER_PATH: &'static str = ".nexus/vault/ledger.json";

    pub async fn auto_snapshot(path: &str, tag: &str) -> Result<Snapshot, String> {
        // Reject directories — vault only snapshots files
        let metadata = fs::metadata(path)
            .map_err(|e| format!("PATH_NOT_FOUND: {}", e))?;
        if metadata.is_dir() {
            return Err("DIRECTORY_SNAPSHOT_NOT_SUPPORTED: Specify a file.".into());
        }

        let content = fs::read_to_string(path)
            .map_err(|e| format!("FILE_READ_FAULT: {}", e))?;

        // Hash the content for deduplication
        let mut hasher = DefaultHasher::new();
        content.hash(&mut hasher);
        let hash = format!("{:x}", hasher.finish());

        // Ensure vault directory exists
        fs::create_dir_all(Self::VAULT_DIR)
            .map_err(|e| format!("DIR_FAULT: {}", e))?;

        // Write snapshot file
        let dest = format!("{}/{}", Self::VAULT_DIR, hash);
        fs::write(&dest, &content)
            .map_err(|e| format!("FS_WRITE_FAULT: {}", e))?;

        let snapshot = Snapshot {
            id:        hash.clone(),
            path:      path.to_string(),
            hash:      hash.clone(),
            timestamp: format!("{:?}", SystemTime::now()),
            directive: tag.to_string(),
        };

        // FIX: Persist snapshot to ledger.json
        Self::append_to_ledger(&snapshot)?;

        println!(
            "\x1b[32m[LAZARUS] Snapshot {} → {} [tag: {}]\x1b[0m",
            &hash[..8], path, tag
        );

        Ok(snapshot)
    }

    /// Appends a snapshot entry to ledger.json (creates it if missing).
    fn append_to_ledger(snapshot: &Snapshot) -> Result<(), String> {
        let mut entries = Self::list_snapshots();

        // Deduplicate by hash — don't write the same content twice
        if entries.iter().any(|e| e.hash == snapshot.hash) {
            return Ok(());
        }

        entries.push(snapshot.clone());

        let json = serde_json::to_string_pretty(&entries)
            .map_err(|e| format!("LEDGER_SERIALIZE_FAULT: {}", e))?;

        fs::write(Self::LEDGER_PATH, json)
            .map_err(|e| format!("LEDGER_WRITE_FAULT: {}", e))?;

        Ok(())
    }

    /// Returns all snapshots sorted oldest → newest.
    /// .last() gives the most recent stable snapshot for rollback.
    pub fn list_snapshots() -> Vec<Snapshot> {
        if !Path::new(Self::LEDGER_PATH).exists() {
            return vec![];
        }
        let data = fs::read_to_string(Self::LEDGER_PATH).unwrap_or_default();
        let mut entries: Vec<Snapshot> = serde_json::from_str(&data).unwrap_or_default();
        // Sort by timestamp string ascending (ISO-like strings sort correctly)
        entries.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
        entries
    }

    /// Returns the most recent snapshot for a given file path.
    pub fn latest_for_path(path: &str) -> Option<Snapshot> {
        Self::list_snapshots()
            .into_iter()
            .filter(|s| s.path == path)
            .last()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[tokio::test]
    async fn test_snapshot_and_ledger() {
        let test_file = "/tmp/nexus_test_snapshot.rs";
        fs::write(test_file, "fn main() {}").unwrap();

        let snap = LazarusVault::auto_snapshot(test_file, "test_tag")
            .await
            .expect("snapshot failed");

        assert!(!snap.hash.is_empty());
        assert_eq!(snap.path, test_file);

        let ledger = LazarusVault::list_snapshots();
        assert!(ledger.iter().any(|e| e.hash == snap.hash));

        // Cleanup
        fs::remove_file(test_file).ok();
    }
}
