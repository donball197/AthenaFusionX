use serde::{Serialize, Deserialize};
use std::fs;
use std::sync::Arc;
use walkdir::WalkDir;
use regex::Regex;
use tokio::sync::RwLock;
use once_cell::sync::Lazy;
use chrono;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AtlasEntry {
    pub name: String,
    pub kind: String,
    pub path: String,
    pub signature: String,
}

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct ProjectAtlas {
    pub entries: Vec<AtlasEntry>,
    pub timestamp: i64,
}

static CACHED_ATLAS: Lazy<Arc<RwLock<ProjectAtlas>>> =
    Lazy::new(|| Arc::new(RwLock::new(ProjectAtlas::default())));

pub struct KnowledgeBroker;

impl KnowledgeBroker {
    pub async fn refresh_atlas() {
        let mut entries = Vec::new();
        let re_fn = Regex::new(r"pub (?:async )?fn ([a-zA-Z0-9_]+)").unwrap();

        if let Ok(it) = WalkDir::new("src")
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
        {
            for entry in it
                .into_iter()
                .filter(|e| e.path().extension().is_some_and(|ext| ext == "rs"))
            {
                let path = entry.path().to_string_lossy().to_string();
                if let Ok(content) = fs::read_to_string(entry.path()) {
                    for cap in re_fn.captures_iter(&content) {
                        entries.push(AtlasEntry {
                            name: cap[1].to_string(),
                            kind: "function".into(),
                            path: path.clone(),
                            signature: cap[0].to_string(),
                        });
                    }
                }
            }
        }

        let mut cache = CACHED_ATLAS.write().await;
        *cache = ProjectAtlas {
            entries,
            timestamp: chrono::Utc::now().timestamp(),
        };
    }

    pub async fn get_atlas() -> ProjectAtlas {
        let cache = CACHED_ATLAS.read().await;
        (*cache).clone()
    }

    pub async fn get_compressed_context() -> String {
        let cache = CACHED_ATLAS.read().await;
        let mut summary = String::from("PROJECT_ATLAS:\n");
        for e in cache.entries.iter().take(30) {
            summary.push_str(&format!("[{}] {} in {}\n", e.kind, e.name, e.path));
        }
        summary
    }
}
