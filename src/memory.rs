use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use chrono::Utc;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Memory {
    pub timestamp: String,
    pub prompt: String,
    pub solution_code: String,
}

pub struct MemoryPalace {
    file_path: String,
}

impl MemoryPalace {
    pub fn new() -> Self {
        let file_path = "nexus_memories.json".to_string();
        if !Path::new(&file_path).exists() { fs::write(&file_path, "[]").unwrap(); }
        Self { file_path }
    }

    pub fn load_all(&self) -> Vec<Memory> {
        let content = fs::read_to_string(&self.file_path).unwrap_or_else(|_| "[]".to_string());
        serde_json::from_str(&content).unwrap_or_default()
    }

    pub fn remember(&self, prompt: String, code: String, _logs: String, _v_hash: Option<String>) {
        let mut memories = self.load_all();
        memories.push(Memory {
            timestamp: Utc::now().to_string(),
            prompt,
            solution_code: code,
        });
        let _ = fs::write(&self.file_path, serde_json::to_string_pretty(&memories).unwrap());
    }

    pub fn recall(&self) -> String {
        self.load_all().last().map(|m| m.solution_code.clone()).unwrap_or_default()
    }

    pub fn purge(&self) {
        let _ = fs::write(&self.file_path, "[]");
    }

    // NEW: The method the Janitor was looking for
    pub fn purge_to_archive(&self, summary: String) {
        let _ = fs::create_dir_all("archives");
        let timestamp = Utc::now().format("%Y%m%d_%H%M%S");
        let archive_path = format!("archives/dream_cycle_{}.md", timestamp);
        let _ = fs::write(archive_path, format!("# Dream Cycle Summary\n\n{}", summary));
        self.purge();
    }
}
