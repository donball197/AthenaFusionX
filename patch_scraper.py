import os

path = "crates/runtime_core/src/executor.rs"
with open(path, "r") as f:
    data = f.read()

scrape_code = """
            AgentAction::ScrapeUrl { url, reasoning } => {
                let _ = self.tx.send(serde_json::json!({"type":"NEURAL_THOUGHT","payload":format!("Scraping: {} | {}", url, reasoning)}).to_string());
                match reqwest::get(&url).await {
                    Ok(resp) => {
                        match resp.text().await {
                            Ok(text) => {
                                // ⚡ Naive HTML strip to preserve LLM context tokens
                                let clean = text.replace("<", " <").split('<').map(|s| s.split('>').last().unwrap_or("")).collect::<String>();
                                let clean = clean.split_whitespace().collect::<Vec<_>>().join(" ");
                                let char_count = clean.chars().count();
                                const MAX_CHARS: usize = 4096;
                                let injected: String = clean.chars().take(MAX_CHARS).collect();
                                let note = if char_count > MAX_CHARS {
                                    format!("\\n[TRUNCATED: showing {}/{} chars]", MAX_CHARS, char_count)
                                } else { String::new() };
                                Ok(format!("[SCRAPE_OK] {}{}\\n--- CONTENT ---\\n{}\\n--- END CONTENT ---", url, note, injected))
                            }
                            Err(e) => Ok(format!("SCRAPE_TEXT_FAILED: {}", e)),
                        }
                    }
                    Err(e) => Ok(format!("SCRAPE_NETWORK_FAILED: {}", e)),
                }
            }
"""

target = "unimplemented_action => {"
if target in data and "AgentAction::ScrapeUrl {" not in data:
    data = data.replace(target, scrape_code + "            " + target)
    with open(path, "w") as f:
        f.write(data)
    print("\x1b[32m    ✔ executor.rs patched with native ScrapeUrl handler.\x1b[0m")
else:
    print("\x1b[31m    [!] Could not patch or already patched.\x1b[0m")
