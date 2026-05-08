#!/usr/bin/env bash
set -euo pipefail
echo -e "\x1b[35m>> RESUMING NEXUS SINGULARITY v3 UPGRADE (STEPS 9-13)\x1b[0m"

echo "[ 9/13 ] server.rs — auto-injecting auth guard to /api/fs/write..."
python3 -c '
import os, re
path = "bins/ide_backend/src/server.rs"
if os.path.exists(path):
    with open(path, "r") as f: data = f.read()
    pattern = re.compile(r"pub async fn start_server\(state: Arc<AppState>\)\s*\{.*?\n\}", re.DOTALL)
    suture = """pub async fn start_server(state: Arc<AppState>) {
    use axum::middleware::from_fn;
    use crate::security::sovereign_auth_guard;

    let authed = Router::new()
        .route("/api/task", axum::routing::post(crate::unified_gateway))
        .route("/api/fs/write", axum::routing::post(crate::fs_write_handler))
        .layer(from_fn(sovereign_auth_guard));

    let app = Router::new()
        .merge(authed)
        .route("/api/metrics", axum::routing::get(crate::metrics_handler))
        .route("/api/fs/read", axum::routing::get(crate::fs_read_handler))
        .route("/api/fs/list", axum::routing::get(crate::fs_list_handler))
        .route("/api/vitals", axum::routing::get(crate::vitals_handler::handle_vitals))
        .route("/api/cluster/nodes", axum::routing::get(crate::cluster_api::list_nodes))
        .route("/ws/telemetry", axum::routing::get(crate::ws_bridge::ws_telemetry_handler))
        .fallback(crate::static_handler)
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    println!("\\x1b[32m>> [SERVER] Listening on http://0.0.0.0:3000\\x1b[0m");
    axum::serve(listener, app).await.unwrap();
}"""
    if pattern.search(data):
        # ⚡ FIX: Use a lambda to bypass regex template parsing on the ANSI escape codes
        with open(path, "w") as f: f.write(pattern.sub(lambda _: suture, data))
        print("    ✔ server.rs spliced successfully.")
    else:
        print("    [FAULT] Could not locate start_server in server.rs")
else:
    print("    [SKIP] server.rs not found.")
'

echo "[ 10/13 ] bins/cli/src/main.rs — removing unused import..."
cat > bins/cli/src/main.rs << 'RUST_EOF'
use std::io::{self, Write};
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};
use nexus_runtime_core::{
    cluster::ClusterMesh,
    distributed_scheduler::DistributedScheduler,
    orchestrator::Orchestrator,
    shared_state::VmoState,
    task_manager::send_task,
};
use nexus_substrate::memory::MemoryPalace;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt().with_env_filter(tracing_subscriber::EnvFilter::from_default_env().add_directive("nexus=info".parse().unwrap())).init();
    println!("\x1b[36m==================================================\x1b[0m");
    println!("\x1b[1;36m>> NEXUS SINGULARITY v3.0 — SOVEREIGN CLI ONLINE\x1b[0m");
    println!("\x1b[36m==================================================\x1b[0m");

    let api_key = std::env::var("GEMINI_API_KEY").expect("Fatal: GEMINI_API_KEY not found in .env file.");
    let (tx, mut rx) = broadcast::channel::<String>(256);
    let memory       = Arc::new(Mutex::new(MemoryPalace::new("nexus_cli.db").await?));
    let vmo_state    = Arc::new(Mutex::new(VmoState::default()));
    let cluster      = Arc::new(ClusterMesh::new());
    let dist         = Arc::new(DistributedScheduler::new(cluster));
    let orchestrator = Arc::new(Orchestrator::new());
    let client       = reqwest::Client::new();

    tokio::spawn(async move {
        while let Ok(msg) = rx.recv().await {
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&msg) {
                let msg_type = parsed["type"].as_str().unwrap_or("LOG");
                let payload  = parsed["payload"].as_str().unwrap_or("");
                match msg_type {
                    "NEURAL_THOUGHT" => println!("\n\x1b[35m[CORTEX]\x1b[0m {}", payload),
                    "TASK_STATUS"    => println!("\x1b[33m[SYS]\x1b[0m {}",    payload),
                    "LOG_EVENT"      => println!("\x1b[90m[EVENT]\x1b[0m {}",   payload),
                    _                => {}
                }
            } else {
                println!("\x1b[90m[RAW]\x1b[0m {}", msg);
            }
        }
    });

    println!("\x1b[32m>> Neural Uplink Established. Substrate Ready.\x1b[0m");
    println!("\x1b[90mType '/help' for commands, or enter a directive.\x1b[0m");
    let stdin = io::stdin();

    loop {
        print!("\n\x1b[1;32mnexus>\x1b[0m ");
        io::stdout().flush()?;
        let mut input = String::new();
        stdin.read_line(&mut input)?;
        let command = input.trim();
        if command.is_empty() { continue; }

        match command {
            "/exit" | "/quit" => { println!("\x1b[31m>> Severing Uplink. Goodbye.\x1b[0m"); break; }
            "/help" => {
                println!("\x1b[36mCommands:\x1b[0m");
                println!("  /status    — Active VMOs + RAM usage");
                println!("  /clear     — Purge SQLite Memory Palace");
                println!("  /exit      — Shutdown CLI");
                println!("  <text>     — Submit directive to the Neural Agent");
            }
            "/status" => {
                let active = unsafe { nexus_substrate::kernel::ffi::muscle_get_active_vmos() };
                println!("\x1b[36m[SUBSTRATE]\x1b[0m Active VMOs: {}", active);
                if let Ok(status) = std::fs::read_to_string("/proc/self/status") {
                    for line in status.lines() {
                        if line.starts_with("VmRSS:") || line.starts_with("VmPeak:") { println!("\x1b[36m[MEM]\x1b[0m {}", line.trim()); }
                    }
                }
            }
            "/clear" => {
                let mem = memory.lock().await;
                if let Err(e) = mem.purge_all().await { println!("\x1b[31m[ERROR]\x1b[0m Memory purge failed: {}", e); } 
                else { println!("\x1b[32m[MEMORY]\x1b[0m Neural context eradicated."); }
            }
            directive => {
                println!("\x1b[36m>> Engaging Neural Routing...\x1b[0m");
                let response = send_task(orchestrator.clone(), &client, &api_key, directive, memory.clone(), vmo_state.clone(), tx.clone(), dist.clone()).await;
                println!("\n\x1b[1;32m[MISSION COMPLETE]\x1b[0m {}", response);
            }
        }
    }
    Ok(())
}
RUST_EOF
echo "    ✔ bins/cli/src/main.rs"

echo "[ 11/13 ] logic_engine.rs — stub detection + hallucination risk flag..."
cat > crates/runtime_core/src/logic_engine.rs << 'RUST_EOF'
use std::collections::HashMap;
use tracing::warn;

pub struct LogicEngine;

impl LogicEngine {
    pub fn is_action_safe(directive: &str) -> bool {
        let forbidden = ["rm -rf /", "mkfs", "shutdown", "chmod 777", "dd if=/dev/zero", "> /dev/sda", "wget | bash", "curl | bash", "wget | sh", "curl | sh", "| bash", "| sh"];
        let d_lower = directive.to_lowercase();
        !forbidden.iter().any(|&p| d_lower.contains(p))
    }

    pub fn is_stub_response(obs: &str) -> bool { obs.contains("[STUB_OK]") || obs.contains("STUB_OK") }

    pub fn detect_hallucination_risk(agent_output: &str) -> Option<String> {
        let hallucination_phrases = ["successfully deployed", "has been fully integrated", "all targeted architectural goals", "all milestones have been met", "deployment was successful"];
        let lower = agent_output.to_lowercase();
        for phrase in hallucination_phrases {
            if lower.contains(phrase) { return Some(format!("[HALLUCINATION_RISK] Output contains presumptive success language: '{}'. Verify this is grounded in actual observation data, not fabricated.", phrase)); }
        }
        None
    }

    pub fn detect_deadlock(observations: &[String]) -> bool {
        if observations.len() < 3 { return false; }
        let mut read_counts: HashMap<String, usize> = HashMap::new();
        for obs in observations {
            if let Some(after) = obs.strip_prefix("[READ_OK] ") {
                let path = after.split_whitespace().next().unwrap_or("").to_string();
                if !path.is_empty() { *read_counts.entry(path).or_insert(0) += 1; }
            }
        }
        for (path, count) in &read_counts {
            if *count >= 3 {
                let file_progress = observations.iter().any(|obs| { (obs.contains("[WRITE_OK]") || obs.contains("[DIFF_PATCH_OK]") || obs.contains("[PATCH_OK]")) && obs.contains(path) });
                if !file_progress { warn!("[LOGIC] Localized deadlock: {} read {} times, zero write progress.", path, count); return true; }
            }
        }
        let mut last_cmd_sig  = String::new();
        let mut cmd_rep_count = 0usize;
        for obs in observations {
            if let Some(start) = obs.find("[CMD:") {
                let sig: String = obs[start..].lines().filter(|l| !l.contains("Finished") && !l.contains("Blocking waiting")).collect::<Vec<&str>>().join("\n");
                if sig == last_cmd_sig && !sig.is_empty() { cmd_rep_count += 1; } else { last_cmd_sig  = sig; cmd_rep_count = 1; }
            }
        }
        if cmd_rep_count >= 2 { warn!("[LOGIC] Deadlock: time-invariant command repetition detected."); return true; }
        false
    }
}
RUST_EOF
echo "    ✔ logic_engine.rs"

echo "[ 12/13 ] Cargo.toml — promoting shared deps to workspace to prevent drift..."
cat > Cargo.toml << 'TOML_EOF'
[workspace]
members = [
    "crates/runtime_core", "crates/identity", "crates/memory_graph",
    "crates/economy", "crates/protocol", "crates/mesh",
    "crates/payments", "crates/marketplace", "crates/billing",
    "bins/cli", "bins/node", "bins/ide_backend"
]
resolver = "2"

[workspace.package]
edition = "2021"
version = "3.0.0"

[workspace.dependencies]
tokio              = { version = "1.44", features = ["full", "io-util"] }
serde              = { version = "1.0",  features = ["derive"] }
serde_json         = "1.0"
anyhow             = "1.0"
reqwest            = { version = "0.12", features = ["json", "stream"] }
axum               = { version = "0.7",  features = ["ws", "macros"] }
sqlx               = { version = "0.8",  features = ["sqlite", "runtime-tokio", "tls-rustls"] }
tracing            = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "json"] }
once_cell          = "1.19"
parking_lot        = "0.12"
async-trait        = "0.1"
base64             = "0.22"
uuid               = { version = "1.8", features = ["v4", "serde"] }
futures-util       = "0.3"
tower-http         = { version = "0.5", features = ["fs", "cors", "trace"] }
dotenvy            = "0.15"
bitflags           = "2.4"
jsonwebtoken       = "9.3"
sysinfo            = "0.31"
mime_guess         = "2.0"
rust-embed         = "8.0"

[profile.dev]
opt-level     = 0
debug         = true
incremental   = true
codegen-units = 256
panic         = "abort"

[profile.release]
opt-level     = 3
lto           = "thin"
codegen-units = 1
strip         = "symbols"
TOML_EOF
echo "    ✔ Cargo.toml"

echo "[ 12.5/13 ] Sweeping child Cargo.toml files for workspace inheritance..."
python3 -c '
import os, re
deps = ["tokio", "serde", "serde_json", "anyhow", "reqwest", "axum", "sqlx", "tracing", "tracing-subscriber", "once_cell", "parking_lot", "async-trait", "base64", "uuid", "futures-util", "tower-http", "dotenvy", "bitflags", "jsonwebtoken", "sysinfo", "mime_guess", "rust-embed"]

for root_dir in ["crates", "bins"]:
    if not os.path.exists(root_dir): continue
    for subdir in os.listdir(root_dir):
        toml_path = os.path.join(root_dir, subdir, "Cargo.toml")
        if os.path.exists(toml_path):
            with open(toml_path, "r") as f: content = f.read()
            new_content = content
            for dep in deps:
                pattern = re.compile(rf"^{dep}\s*=\s*([\"\x27][^\"\x27]+[\"\x27]|\{{(?:(?!\bworkspace\b).)*\}})", re.MULTILINE)
                new_content = pattern.sub(f"{dep} = {{ workspace = true }}", new_content)
            if new_content != content:
                with open(toml_path, "w") as f: f.write(new_content)
                print(f"    ✔ Migrated dependencies in {toml_path}")
'

echo "[ 13/13 ] Aggressive FFI / Target Cleanup..."
rm -rf target/
rm -f *.so
echo "    ✔ Linker caches purged."

echo ""
echo -e "\x1b[32m>> RESUMPTION SEQUENCE COMPLETE.\x1b[0m"
