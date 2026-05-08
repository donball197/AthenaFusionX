// lazarus_vault.rs — v1.4 [LEDGER_FIX]
// FIX: auto_snapshot() now appends every snapshot to ledger.json.
//      Previously it wrote to .nexus/vault/<hash> but never touched the ledger,
//      so list_snapshots() returned [] and rollback was permanently blind.
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

    /// Snapshot a single file to the vault and append to ledger.
    pub async fn auto_snapshot(path: &str, tag: &str) -> Result<Snapshot, String> {
        // Reject directories — vault only snapshots files
        let metadata = fs::metadata(path).map_err(|e| format!("PATH_NOT_FOUND: {}", e))?;
        if metadata.is_dir() {
            return Err("DIRECTORY_SNAPSHOT_NOT_SUPPORTED: Specify a file.".into());
        }

        let content = fs::read_to_string(path).map_err(|e| format!("FILE_READ_FAULT: {}", e))?;

        // Content hash for deduplication
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

        // FIX: Actually persist to ledger so list_snapshots() can find it
        Self::append_to_ledger(&snapshot)?;

        println!(
            "\x1b[32m[LAZARUS] Snapshot {} → {} [tag: {}]\x1b[0m",
            &hash[..8.min(hash.len())],
            path,
            tag
        );

        Ok(snapshot)
    }

    /// Restore a snapshot by hash — writes the saved content back to its path.
    pub fn restore(hash: &str) -> Result<String, String> {
        let src = format!("{}/{}", Self::VAULT_DIR, hash);
        let content =
            fs::read_to_string(&src).map_err(|e| format!("VAULT_READ_FAULT [{}]: {}", hash, e))?;

        // Find the target path from the ledger
        let snap = Self::list_snapshots()
            .into_iter()
            .find(|s| s.hash == hash)
            .ok_or_else(|| format!("HASH_NOT_IN_LEDGER: {}", hash))?;

        fs::write(&snap.path, &content).map_err(|e| format!("RESTORE_WRITE_FAULT: {}", e))?;

        Ok(format!("[LAZARUS] Restored {} → {}", &hash[..8.min(hash.len())], snap.path))
    }

    /// Appends a snapshot to ledger.json. Deduplicates by hash.
    fn append_to_ledger(snapshot: &Snapshot) -> Result<(), String> {
        let mut entries = Self::list_snapshots();

        if entries.iter().any(|e| e.hash == snapshot.hash) {
            return Ok(()); // Already snapshotted identical content
        }

        entries.push(snapshot.clone());

        let json = serde_json::to_string_pretty(&entries)
            .map_err(|e| format!("LEDGER_SERIALIZE_FAULT: {}", e))?;

        fs::write(Self::LEDGER_PATH, json).map_err(|e| format!("LEDGER_WRITE_FAULT: {}", e))?;

        Ok(())
    }

    /// Returns all snapshots sorted oldest → newest.
    /// Use .last() for the most recent stable snapshot.
    pub fn list_snapshots() -> Vec<Snapshot> {
        if !Path::new(Self::LEDGER_PATH).exists() {
            return vec![];
        }
        let data = fs::read_to_string(Self::LEDGER_PATH).unwrap_or_default();
        let mut entries: Vec<Snapshot> = serde_json::from_str(&data).unwrap_or_default();
        entries.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
        entries
    }

    /// Returns the most recent snapshot for a given file path.
    pub fn latest_for_path(path: &str) -> Option<Snapshot> {
        Self::list_snapshots().into_iter().rfind(|s| s.path == path)
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

        let snap =
            LazarusVault::auto_snapshot(test_file, "test_tag").await.expect("snapshot failed");

        assert!(!snap.hash.is_empty());
        assert_eq!(snap.path, test_file);

        let ledger = LazarusVault::list_snapshots();
        assert!(ledger.iter().any(|e| e.hash == snap.hash));

        fs::remove_file(test_file).ok();
    }
}
