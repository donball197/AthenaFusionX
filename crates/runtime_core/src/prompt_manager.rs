use std::fs;
use std::path::Path;

pub struct PromptManager;

impl PromptManager {
    pub fn get_router_persona() -> String {
        let path = Path::new(".nexus/prompts.toml");
        if let Ok(content) = fs::read_to_string(path) {
            // Simple string extraction to avoid heavy TOML dependencies for now
            if let Some(start) = content.find("persona = '''") {
                let text = &content[start + 13..];
                if let Some(end) = text.find("'''") {
                    return text[..end].to_string();
                }
            }
        }
        // Fallback if file doesn't exist or parsing fails
        "IDENTITY: NEXUS Sovereign Agent.\nSTRICT SYSTEM RULE: You MUST output EXACTLY ONE valid JSON array...".to_string()
    }
}
