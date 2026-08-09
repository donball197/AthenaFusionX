import os

print(">> Deploying Smart Scraper & CLI Tool...")

# --- 1. UPDATE DRONE DEPENDENCIES ---
drone_cargo = "drones/data_scraper/Cargo.toml"
with open(drone_cargo, "r") as f:
    content = f.read()
if "nexus_model_hub" not in content:
    content += 'nexus_model_hub = { path = "../../crates/nexus_model_hub" }\n'
    with open(drone_cargo, "w") as f:
        f.write(content)
    print("[OK] Linked nexus_model_hub to data_scraper.")

# --- 2. UPGRADE DRONE LOGIC ---
drone_main = "drones/data_scraper/src/main.rs"
new_drone_logic = """use axum::{extract::{Json, State}, response::IntoResponse, routing::post, Router};
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct ScrapeRequest { pub url: String }

#[derive(Deserialize)]
pub struct ExtractRequest { pub url: String, pub prompt: String }

#[derive(Serialize)]
pub struct ScrapeResponse { pub content: String }

async fn scrape(State(_state): State<()>, Json(payload): Json<ScrapeRequest>) -> impl IntoResponse {
    let client = reqwest::Client::new();
    let content = match client.get(&payload.url).send().await {
        Ok(resp) => resp.text().await.unwrap_or_else(|_| "Failed to decode body".to_string()),
        Err(e) => format!("Failed to fetch URL: {}", e),
    };
    Json(ScrapeResponse { content })
}

async fn extract(State(_state): State<()>, Json(payload): Json<ExtractRequest>) -> impl IntoResponse {
    let client = reqwest::Client::new();
    let html = match client.get(&payload.url).send().await {
        Ok(resp) => resp.text().await.unwrap_or_default(),
        Err(_) => return Json(ScrapeResponse { content: "Failed to fetch HTML".to_string() }),
    };
    
    let api_key = std::env::var("GEMINI_API_KEY").unwrap_or_default();
    let safe_html: String = html.chars().take(20000).collect(); // Prevent context overflow
    let user_prompt = format!("Extract data from this HTML based on the following prompt:\\n{}\\n\\nHTML:\\n{}", payload.prompt, safe_html);
    
    let content = match nexus_model_hub::universal_provider::UniversalProvider::generate(
        &client, &api_key, "gemini-3.5-flash-lite", Some("You are a strict data extraction tool. Return only the requested data in a clean format."), &user_prompt, 0.1, 4096
    ).await {
        Ok(res) => res,
        Err(e) => format!("LLM Extraction Error: {}", e),
    };
    
    Json(ScrapeResponse { content })
}

#[tokio::main]
async fn main() {
    let app = Router::new()
        .route("/scrape", post(scrape))
        .route("/extract", post(extract))
        .with_state(());
    let listener = tokio::net::TcpListener::bind("0.0.0.0:5000").await.unwrap();
    println!(">> [DRONE] Smart Scraper ignited on port 5000");
    axum::serve(listener, app).await.unwrap();
}
"""
with open(drone_main, "w") as f:
    f.write(new_drone_logic)
print("[OK] Smart Extraction logic injected into Drone.")

# --- 3. SCAFFOLD CLI TOOL ---
os.makedirs("apps/nexus_scrape/src", exist_ok=True)

cli_cargo = "apps/nexus_scrape/Cargo.toml"
with open(cli_cargo, "w") as f:
    f.write("""[package]
name = "nexus_scrape"
version = "0.1.0"
edition = "2021"

[dependencies]
clap = { version = "4.4", features = ["derive"] }
reqwest = { workspace = true, features = ["json"] }
tokio = { workspace = true, features = ["full"] }
serde_json = { workspace = true }
""")

cli_main = "apps/nexus_scrape/src/main.rs"
with open(cli_main, "w") as f:
    f.write("""use clap::Parser;
use serde_json::json;

#[derive(Parser)]
#[command(name = "nexus-scrape", about = "AthenaFusionX CLI Web Scraper & Extractor")]
struct Args {
    /// The URL to scrape
    url: String,

    /// Optional LLM prompt to extract specific data from the HTML
    #[arg(short, long)]
    extract: Option<String>,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    let client = reqwest::Client::new();
    
    let (endpoint, payload) = if let Some(prompt) = args.extract {
        ("http://127.0.0.1:5000/extract", json!({"url": args.url, "prompt": prompt}))
    } else {
        ("http://127.0.0.1:5000/scrape", json!({"url": args.url}))
    };
    
    println!("\\x1b[36m>> Sending request to Nexus Drone...\\x1b[0m");
    let res = client.post(endpoint).json(&payload).send().await;
    
    match res {
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
    }
}
""")
print("[OK] CLI Tool built.")

# --- 4. REGISTER CLI IN WORKSPACE ---
root_cargo = "Cargo.toml"
with open(root_cargo, "r") as f:
    content = f.read()
if '"apps/nexus_scrape"' not in content:
    content = content.replace('"drones/data_scraper"', '"drones/data_scraper",\n    "apps/nexus_scrape"')
    with open(root_cargo, "w") as f:
        f.write(content)
    print("[OK] CLI Tool registered to workspace.")

print(">> Deployment Complete! Compiling workspace...")
