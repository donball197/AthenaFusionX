use anyhow::Result;
use reqwest::Client;
use base64::{Engine as _, engine::general_purpose};
use std::fs;
use crate::models::{GemmaRequest, Content, Part, InlineData, GemmaResponse};

pub struct MicroAgent;

impl MicroAgent {
    pub fn init() -> Result<Self> { Ok(MicroAgent) }

    pub async fn explain_vision(&self, client: &Client, api_key: &str, image_path: &str) -> Result<String> {
        let img_bytes = fs::read(image_path)?;
        let b64_img = general_purpose::STANDARD.encode(img_bytes);
        let manifest = fs::read_to_string("Cargo.toml").unwrap_or_default();

        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/gemma-3-27b-it:generateContent?key={}",
            api_key
        );

        let payload = GemmaRequest {
            contents: vec![Content {
                role: "user".to_string(),
                parts: vec![
                    Part::Text {
                        text: format!(
                            "Act as a Senior Systems Architect. Refactor 'src/hiram_core.rs'.\n\n\
                            EVOLUTION RULES:\n\
                            1. MUST include 'pub struct Hiram {{ pub integrity: i32, }}'.\n\
                            2. MUST include 'pub fn new() -> Self'.\n\
                            3. MUST use 'pub fn verify(&mut self)'. Note the MUTABLE reference.\n\
                            4. You ARE authorized to change the value of self.integrity.\n\
                            5. NO markdown, NO prose. Start with 'FIX_CORE:'.\n\n\
                            CONTEXT:\n{}", manifest)
                    },
                    Part::InlineData {
                        inline_data: InlineData {
                            mime_type: "image/jpeg".to_string(),
                            data: b64_img,
                        }
                    },
                ],
            }],
        };

        let resp = client.post(&url).json(&payload).send().await?;
        let body: GemmaResponse = resp.json().await?;
        let raw = body.candidates[0].content.parts[0].text.clone();
        Ok(raw.replace("```rust", "").replace("```", "").trim().to_string())
    }
}
