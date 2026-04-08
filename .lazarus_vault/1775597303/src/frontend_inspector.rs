use crate::fs_utils;
use anyhow::{Result, Context};

pub struct UIInspector;

impl UIInspector {
    pub fn scan_svelte_logic(path: &str) -> Result<String> {
        let content = fs_utils::read_file_safe(path)
            .context(format!("Failed to ingest Svelte DNA at {}", path))?;

        println!(">> [INSPECTOR] Analyzing Svelte 5 Runes in {}...", path);
        
        // Identify Svelte 5 specific structures for the Agent
        let has_state = content.contains("$state");
        let has_derived = content.contains("$derived");
        let has_effect = content.contains("$effect");
        let has_props = content.contains("$props");

        let mut analysis = format!("--- TARGET: {} ---\n", path);
        analysis.push_str(&format!("REACTIVE_PROFILE: [State: {}, Derived: {}, Effect: {}, Props: {}]\n", 
            has_state, has_derived, has_effect, has_props));
        analysis.push_str("CONTENT:\n");
        analysis.push_str(&content);

        Ok(analysis)
    }
}
