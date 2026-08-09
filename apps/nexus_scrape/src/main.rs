use clap::Parser;
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
    
    println!("\x1b[36m>> Sending request to Nexus Drone...\x1b[0m");
    let res = client.post(endpoint).json(&payload).send().await;
    
    match res {
        Ok(resp) => {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            
            if let Ok(json_resp) = serde_json::from_str::<serde_json::Value>(&text) {
                if let Some(content) = json_resp.get("content").and_then(|c| c.as_str()) {
                    println!("\n\x1b[1;32m================ EXTRACTION RESULT ================\x1b[0m\n");
                    println!("{}", content);
                    println!("\n\x1b[1;32m===================================================\x1b[0m");
                    return;
                }
            }
            // If we make it here, the JSON parse failed OR there was no 'content' field
            println!("\x1b[33m[DEBUG] Drone returned HTTP {}\x1b[0m", status);
            println!("\x1b[31m[RAW DRONE OUTPUT]:\n{}\x1b[0m", text);
        }
        Err(e) => println!("\x1b[31m[ERROR] Could not connect to drone: {}. Is the drone running?\x1b[0m", e),
    }
}
