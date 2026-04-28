use anyhow::{Context, Result};
use base64::{engine::general_purpose, Engine as _};
use reqwest::Client;
use serde_json::json;
use std::fs;

pub async fn scan_local_image_cloud(
    image_path: &str,
    prompt: &str,
    api_key: &str,
) -> Result<String> {
    println!("\x1b[35m>> [VISION_CORTEX] Validating and encoding {} for cloud offload...\x1b[0m", image_path);

    let safe_path = crate::sandbox::normalize_and_check(image_path)
        .context("VISION_SECURITY: Path validation failed")?;

    let image_bytes = fs::read(&safe_path).context("Failed to read image from disk")?;
    let base64_image = general_purpose::STANDARD.encode(&image_bytes);

    let mime_type = if image_path.to_lowercase().ends_with(".png") {
        "image/png"
    } else {
        "image/jpeg"
    };

    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models/gemma-4-26b-a4b-it:generateContent?key={}",
        api_key
    );

    let payload = json!({
        "contents": [{
            "role": "user",
            "parts": [
                { "text": prompt },
                { "inline_data": { "mime_type": mime_type, "data": base64_image } }
            ]
        }],
        "generationConfig": { "temperature": 0.2, "maxOutputTokens": 1024 }
    });

    let res = Client::new().post(&url).json(&payload).send().await?;

    if !res.status().is_success() {
        let err = res.text().await?;
        return Err(anyhow::anyhow!("VISION_API_FAULT: {}", err));
    }

    let body: serde_json::Value = res.json().await?;

    if let Some(text) = body["candidates"][0]["content"]["parts"][0]["text"].as_str() {
        println!("\x1b[35m>> [VISION_CORTEX] Analysis complete.\x1b[0m");
        Ok(text.trim().to_string())
    } else {
        Err(anyhow::anyhow!("VISION_FAULT: Malformed response."))
    }
}
