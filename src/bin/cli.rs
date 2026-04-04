use rustyline::{DefaultEditor, Config, EditMode};
use std::sync::Arc;
use nexus_singularity::{
    neural_agent::MicroAgent, 
    task_manager, 
    system_monitor::SystemMonitor,
    lazarus_vault::LazarusVault
};
use std::io::{self, Write};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    println!("\x1b[38;2;0;255;255m>> [CLI] NEXUS COMMAND INTERFACE ONLINE (v2.0.0-Sovereign)\x1b[0m");
    println!("\x1b[90m>> Available Local Commands: status, history, restore <hash>, clear\x1b[0m");
    println!("\x1b[90m>> AI Execution: task <directive>\x1b[0m\n");

    let api_key = std::env::var("GEMINI_API_KEY").expect("GEMINI_API_KEY missing");
    let (tx, _) = tokio::sync::broadcast::channel(128);
    let agent = Arc::new(MicroAgent::init(tx.clone()).unwrap());
    let monitor = SystemMonitor::new();

    let mut rx = tx.subscribe();
    tokio::spawn(async move {
        while let Ok(msg) = rx.recv().await {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&msg) {
                if val["type"] == "ghost_token" {
                    if let Some(token) = val["payload"].as_str() {
                        print!("\x1b[90m{}\x1b[0m", token);
                        io::stdout().flush().ok();
                    }
                }
            }
        }
    });

    let mut rl = DefaultEditor::with_config(Config::builder().edit_mode(EditMode::Emacs).build())?;
    
    loop {
        let readline = rl.readline("\x1b[32mnexus_cli>\x1b[0m ");
        let line = match readline { Ok(l) => l, Err(_) => break };
        let trimmed = line.trim();
        if trimmed.is_empty() { continue; }

        let _ = rl.add_history_entry(trimmed);
        let parts: Vec<&str> = trimmed.split_whitespace().collect();
        let cmd = parts[0].to_lowercase();

        match cmd.as_str() {
            "exit" | "quit" => break,
            "clear" => {
                print!("{}[2J", 27 as char);
                io::stdout().flush()?;
            }
            "status" => {
                let v = monitor.get_vitals();
                println!("\n\x1b[1;36m=== SYSTEM VITALS ===\x1b[0m");
                println!("CPU Load:  {:.1}%", v.cpu_load);
                println!("RAM Usage: {}", v.ram_usage);
                println!("Thermal:   {:.1}°C", v.temperature);
                println!("State:     {}", v.status);
                println!();
            }
            "history" | "lazarus" => {
                match LazarusVault::list_snapshots() {
                    Ok(snaps) if !snaps.is_empty() => {
                        println!("\n\x1b[1;36m=== LAZARUS VAULT (Last 10) ===\x1b[0m");
                        for s in snaps.iter().rev().take(10) { 
                            println!("\x1b[33m{}\x1b[0m | {} | \x1b[90m{}\x1b[0m", &s.hash[..8], s.path, s.author); 
                        }
                        println!();
                    }
                    _ => println!(">> [VAULT] Ledger is empty.\n"),
                }
            }
            "restore" => {
                if let Some(hash) = parts.get(1) {
                    match LazarusVault::restore_snapshot(hash) {
                        Ok(msg) => println!("\x1b[32m{}\x1b[0m\n", msg),
                        Err(e) => println!("\x1b[31m{}\x1b[0m\n", e),
                    }
                } else {
                    println!(">> Usage: restore <hash>\n");
                }
            }
            "task" => {
                let directive = parts[1..].join(" ");
                let client = reqwest::Client::new();
                let _ = task_manager::send_task(agent.clone(), &client, &api_key, &directive).await;
                println!();
            }
            _ => {
                println!(">> Unrecognized command. Use 'task <directive>' for AI, or 'status', 'history', 'restore', 'clear'.\n");
            }
        }
    }
    Ok(())
}
