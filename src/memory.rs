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
        if !Path::new(&file_path).exists() {
            fs::write(&file_path, "[]").unwrap();
        }
        Self { file_path }
    }

    pub fn remember(&self, prompt: String, code: String) {
        let mut memories = self.load_all();
        memories.push(Memory {
            timestamp: Utc::now().to_string(),
            prompt,
            solution_code: code,
        });
        let json = serde_json::to_string_pretty(&memories).unwrap();
        fs::write(&self.file_path, json).unwrap();
        println!(">> [MEMORY] Solution stored in Long-Term Memory.");
    }

    pub fn recall(&self) -> String {
        let memories = self.load_all();
        let recent = memories.iter().rev().take(3).collect::<Vec<_>>();
        
        if recent.is_empty() {
            return String::new();
        }

        let mut context = String::from("\n\n=== LONG-TERM MEMORY (PREVIOUS FIXES) ===\n");
        for mem in recent {
            context.push_str(&format!("User asked: '{}'\nWe wrote code corrections.\n", mem.prompt));
        }
        context.push_str("=========================================\n");
        context
    }

    pub fn load_all(&self) -> Vec<Memory> {
        let content = fs::read_to_string(&self.file_path).unwrap_or_else(|_| "[]".to_string());
        serde_json::from_str(&content).unwrap_or_default()
    }

    // NEW: The missing purge method that was causing the error
    pub fn purge(&self) {
        if let Ok(_) = fs::write(&self.file_path, "[]") {
            println!(">> [MEMORY] Long-Term Memory Purged Successfully.");
        } else {
            eprintln!(">> [ERROR] Failed to purge Memory Palace.");
        }
    }
}
