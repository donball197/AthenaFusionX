use serde::{Serialize, Deserialize};
use std::collections::VecDeque;
use std::fs;
use std::path::Path;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct NexusMemory {
    pub short_term: VecDeque<String>,
    pub long_term: Vec<String>,
    pub experience_level: u32,
}

impl NexusMemory {
    pub fn load() -> Self {
        if Path::new("nexus_memory.json").exists() {
            let data = fs::read_to_string("nexus_memory.json").unwrap_or_default();
            serde_json::from_str(&data).unwrap_or_else(|_| NexusMemory::new())
        } else {
            NexusMemory::new()
        }
    }

    pub fn new() -> Self {
        Self {
            short_term: VecDeque::with_capacity(10),
            long_term: Vec::new(),
            experience_level: 1,
        }
    }

    pub fn remember(&mut self, entry: String) {
        // Add to short term
        if self.short_term.len() >= 10 {
            // Consolidation: Move oldest short-term to long-term if it was significant
            let old = self.short_term.pop_front().unwrap();
            if old.contains("SUCCESS") || old.contains("OPTIMIZED") {
                self.long_term.push(old);
            }
        }
        self.short_term.push_back(entry);
        self.save();
    }

    pub fn save(&self) {
        if let Ok(data) = serde_json::to_string_pretty(self) {
            let _ = fs::write("nexus_memory.json", data);
        }
    }

    // Returns a context string to feed into the AI
    pub fn get_context_window(&self) -> String {
        let recent = self.short_term.iter().cloned().collect::<Vec<String>>().join("\n");
        format!("PREVIOUS ACTIONS:\n{}\nEXPERIENCE LEVEL: {}", recent, self.experience_level)
    }
}
