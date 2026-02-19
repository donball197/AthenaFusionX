use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use chrono::Utc;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Memory {
    pub timestamp: String,
    pub prompt: String,
    pub solution_code: String,
    pub terminal_snapshot: String,
    pub vision_hash: Option<String>,
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

    pub fn remember(&self, prompt: String, code: String, logs: String, v_hash: Option<String>) {
        let mut memories = self.load_all();
        if memories.len() >= 50 { memories.remove(0); }
        memories.push(Memory {
            timestamp: Utc::now().to_string(),
            prompt,
            solution_code: code,
            terminal_snapshot: logs,
            vision_hash: v_hash,
        });
        let json = serde_json::to_string_pretty(&memories).unwrap();
        let _ = fs::write(&self.file_path, json);
    }

    pub fn recall(&self) -> String {
        let memories = self.load_all();
        let recent = memories.iter().rev().take(3).collect::<Vec<_>>();
        if recent.is_empty() { return String::new(); }
        let mut context = String::from("\n=== NEURAL RECALL ===\n");
        for mem in recent { context.push_str(&format!("TASK: {}\n---\n", mem.prompt)); }
        context
    }

    pub fn load_all(&self) -> Vec<Memory> {
        let content = fs::read_to_string(&self.file_path).unwrap_or_else(|_| "[]".to_string());
        serde_json::from_str(&content).unwrap_or_default()
    }

    pub fn purge(&self) { let _ = fs::write(&self.file_path, "[]"); }
}
