import os

print(">> Upgrading Scraper Diagnostics...")

# 1. Give Drone access to .env
cargo_toml = "drones/data_scraper/Cargo.toml"
with open(cargo_toml, "a") as f:
    f.write('dotenvy = { workspace = true }\n')

drone_main = "drones/data_scraper/src/main.rs"
with open(drone_main, "r") as f:
    content = f.read()
if "dotenvy::dotenv().ok();" not in content:
    content = content.replace("async fn main() {", "async fn main() {\n    dotenvy::dotenv().ok();")
    with open(drone_main, "w") as f:
        f.write(content)

# 2. Upgrade CLI Error Logging
cli_main = "apps/nexus_scrape/src/main.rs"
with open(cli_main, "r") as f:
    content = f.read()

old_match = """    match res {
        Ok(resp) => {
            let json_resp: serde_json::Value = resp.json().await.unwrap_or_default();
            if let Some(content) = json_resp.get("content").and_then(|c| c.as_str()) {
                println!("\\n\\x1b[1;32m================ EXTRACTION RESULT ================\\x1b[0m\\n");
                println!("{}", content);
                println!("\\n\\x1b[1;32m===================================================\\x1b[0m");
            } else {
                println!("\\x1b[31m[ERROR] Failed to parse drone response.\\x1b[0m");
            }
        }
        Err(e) => println!("\\x1b[31m[ERROR] Could not connect to drone: {}. Is the drone running?\\x1b[0m", e),
    }"""

new_match = """    match res {
        Ok(resp) => {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            
            if let Ok(json_resp) = serde_json::from_str::<serde_json::Value>(&text) {
                if let Some(content) = json_resp.get("content").and_then(|c| c.as_str()) {
                    println!("\\n\\x1b[1;32m================ EXTRACTION RESULT ================\\x1b[0m\\n");
                    println!("{}", content);
                    println!("\\n\\x1b[1;32m===================================================\\x1b[0m");
                    return;
                }
            }
            // If we make it here, the JSON parse failed OR there was no 'content' field
            println!("\\x1b[33m[DEBUG] Drone returned HTTP {}\\x1b[0m", status);
            println!("\\x1b[31m[RAW DRONE OUTPUT]:\\n{}\\x1b[0m", text);
        }
        Err(e) => println!("\\x1b[31m[ERROR] Could not connect to drone: {}. Is the drone running?\\x1b[0m", e),
    }"""

if old_match in content:
    content = content.replace(old_match, new_match)
    with open(cli_main, "w") as f:
        f.write(content)

print(">> Diagnostics Upgrade Complete.")
