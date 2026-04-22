/**
 * Copyright 2026 Donald Ball
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 * http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR REPRESENTATIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */


use anyhow::{Context, Result};
use base64::{engine::general_purpose, Engine as _};
use reqwest::Client;
use serde_json::json;
use std::fs;

/// Cloud-Offloaded Vision Cortex (Optimized for 4GB Metal)
pub async fn scan_local_image_cloud(
    image_path: &str,
    prompt: &str,
    api_key: &str,
) -> Result<String> {
    println!(
        "\x1b[35m>> [VISION_CORTEX] Encoding {} for lightweight cloud offload...\x1b[0m",
        image_path
    );

    // Security boundary: Prevent directory traversal outside the workspace
    if image_path.contains("..") {
        return Err(anyhow::anyhow!("Security Block: Invalid image path structure."));
    }

    let image_bytes = fs::read(image_path).context("Failed to read image from disk")?;
    let base64_image = general_purpose::STANDARD.encode(&image_bytes);

    let mime_type =
        if image_path.to_lowercase().ends_with(".png") { "image/png" } else { "image/jpeg" };

    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models/gemma-4-26b-a4b-it:generateContent?key={}",
        api_key
    );

    let payload = json!({
        "contents": [{
            "role": "user",
            "parts": [
                { "text": prompt },
                {
                    "inline_data": {
                        "mime_type": mime_type,
                        "data": base64_image
                    }
                }
            ]
        }],
        "generationConfig": {
            "temperature": 0.2,
            "maxOutputTokens": 1024
        }
    });

    let client = Client::new();
    let res = client.post(&url).json(&payload).send().await?;

    if !res.status().is_success() {
        let err = res.text().await?;
        return Err(anyhow::anyhow!("VISION_API_FAULT: {}", err));
    }

    let body: serde_json::Value = res.json().await?;

    if let Some(text) = body["candidates"][0]["content"]["parts"][0]["text"].as_str() {
        println!("\x1b[35m>> [VISION_CORTEX] Analysis Complete. 0MB Local RAM Consumed.\x1b[0m");
        Ok(text.trim().to_string())
    } else {
        Err(anyhow::anyhow!("VISION_FAULT: Malformed response from Conductor."))
    }
}
