use serde::{Serialize, Deserialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Serialize, Deserialize, Default)]
pub struct AstCache {
    pub ledger: HashMap<String, CachedTriage>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct CachedTriage {
    pub state: String,
    pub severity: String,
    pub reason: String,
}

pub struct PersistentCache {
    path: PathBuf,
    cache: AstCache,
}

impl PersistentCache {
    pub fn init<P: AsRef<Path>>(db_path: P) -> Self {
        let path = db_path.as_ref().to_path_buf();
        
        // Ensure parent directories (including USB mount path) exist
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }

        let cache = if path.exists() {
            match fs::read_to_string(&path) {
                Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
                Err(_) => AstCache::default(),
            }
        } else {
            AstCache::default()
        };

        PersistentCache { path, cache }
    }

    pub fn get_triage(&self, finding_id: &str) -> Option<(String, String, String)> {
        self.cache.ledger.get(finding_id).map(|item| {
            (item.state.clone(), item.severity.clone(), item.reason.clone())
        })
    }

    pub fn set_triage(&mut self, finding_id: &str, state: &str, severity: &str, reason: &str) {
        self.cache.ledger.insert(
            finding_id.to_string(),
            CachedTriage {
                state: state.to_string(),
                severity: severity.to_string(),
                reason: reason.to_string(),
            },
        );
        let _ = self.save();
    }

    fn save(&self) -> std::io::Result<()> {
        let serialized = serde_json::to_string_pretty(&self.cache)?;
        fs::write(&self.path, serialized)
    }
}
