use anyhow::Result;
use reqwest::Client;
use base64::{Engine as _, engine::general_purpose};
use std::fs;
use crate::models::{GemmaRequest, Content, Part, InlineData, GemmaResponse};
use crate::context;

pub struct MicroAgent;

impl MicroAgent {
    pub fn init() -> Result<Self> { Ok(MicroAgent) }

    pub async fn evolve_kernel(&self, client: &Client, api_key: &str, image_path: &str) -> Result<String> {
        let b64_img = general_purpose::STANDARD.encode(fs::read(image_path)?);
        let vitals = context::get_system_vitals();
        let source = fs::read_to_string("src/hiram_core.rs")?;

        let url = format!("https://generativelanguage.googleapis.com/v1beta/models/gemma-3-27b-it:generateContent?key={}", api_key);

        let payload = GemmaRequest {
            contents: vec![Content {
                role: "user".to_string(),
                parts: vec![
                    Part::Text {
                        text: format!("STRICT MOTOROLA MODE. VITALS: {}. SOURCE: {}. REFACTOR logic. Output format: REASONING: [intent] CODE: [source]. NO MARKDOWN.", vitals, source),
                    },
                    Part::InlineData {
                        inline_data: InlineData { mime_type: "image/jpeg".to_string(), data: b64_img },
                    },
                ],
            }],
        };

        let res = client.post(&url).json(&payload).send().await?;
        let gemma_res: GemmaResponse = res.json().await?;
        
        // FIXED: Direct access because text is a String in your models.rs
        let raw = gemma_res.candidates[0].content.parts[0].text.clone();
        
        Ok(raw.replace("```rust", "").replace("```", "").trim().to_string())
    }
}
