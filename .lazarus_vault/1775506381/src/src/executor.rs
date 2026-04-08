use crate::nexus_error::NexusError;
use crate::agent_protocol::AgentAction;
use crate::component_runner::AgentHandle;
use serde_json::Value;
use tokio::sync::broadcast::Sender;
use comfy_table::{Table, Cell, Color, Attribute, presets::UTF8_FULL_CONDENSED};
use regex::Regex;
use std::process::Command;

pub struct Executor;

impl Executor {
    pub async fn execute_handle(
        _handle: &AgentHandle,
        actions: Vec<AgentAction>,
    ) -> Result<Vec<String>, NexusError> {
        let mut results = Vec::new();
        for action in actions {
            match action {
                AgentAction::Message { message } => results.push(format!(">> [MSG]: {}", message)),
                _ => results.push("!! [AGENT]: Legacy directive logged.".into()),
            }
        }
        Ok(results)
    }

    pub async fn execute_swarm_json(
        json_text: &str, 
        tx: &Sender<String>
    ) -> Result<(), NexusError> {
        let re = Regex::new(r"(?s)```json\s*(.*?)\s*```").unwrap_or_else(|_| Regex::new("").unwrap());
        let json_payload = if let Some(caps) = re.captures(json_text) {
            caps.get(1).map_or(json_text, |m| m.as_str())
        } else {
            let start = json_text.find('[').unwrap_or(0);
            let end = json_text.rfind(']').unwrap_or(json_text.len() - 1);
            if start < end { &json_text[start..=end] } else { json_text }
        };

        if let Ok(actions) = serde_json::from_str::<Vec<Value>>(json_payload) {
            let msg = ">> [EXECUTOR] Action Matrix intercepted. Mapping execution topology...";
            println!("\x1b[32m{}\x1b[0m", msg);
            let _ = tx.send(msg.to_string());
            
            let mut table = Table::new();
            table.load_preset(UTF8_FULL_CONDENSED);
            table.set_header(vec![
                Cell::new("OPCODE").add_attribute(Attribute::Bold).fg(Color::Cyan),
                Cell::new("TARGET").add_attribute(Attribute::Bold).fg(Color::Magenta),
                Cell::new("PAYLOAD / DIAGNOSTIC DATA").add_attribute(Attribute::Bold).fg(Color::DarkGrey),
            ]);

            for action in actions {
                let op = action.get("action").or_else(|| action.get("op")).or_else(|| action.get("task")).and_then(|a| a.as_str()).unwrap_or("UNKNOWN").to_lowercase();
                let target = action.get("target").or_else(|| action.get("file")).or_else(|| action.get("resource")).and_then(|a| a.as_str()).unwrap_or("GLOBAL");
                
                let mut details = action.clone();
                if let Some(obj) = details.as_object_mut() {
                    obj.remove("action"); obj.remove("op"); obj.remove("task");
                    obj.remove("target"); obj.remove("file"); obj.remove("resource");
                }
                
                let details_str = serde_json::to_string_pretty(&details).unwrap_or_default();
                table.add_row(vec![Cell::new(&op).fg(Color::Green), Cell::new(target).fg(Color::Yellow), Cell::new(details_str.clone())]);
                
                let _ = tx.send(format!(">> [EXECUTOR] {} -> {}", op, target));

                // === HELPER: UNIVERSAL COMMAND EXTRACTION ===
                let cmd = details.get("command")
                    .or_else(|| details.get("cmd"))
                    .or_else(|| details.get("bash_command"))
                    .or_else(|| details.get("bash-command"))
                    .or_else(|| details.get("payload").and_then(|p| p.get("command")))
                    .or_else(|| details.get("payload").and_then(|p| p.get("bash_command")))
                    .or_else(|| details.get("payload").and_then(|p| p.get("bash-command")))
                    .and_then(|c| c.as_str())
                    .unwrap_or("");

                // Trigger Lazarus for ANY mutation
                if !cmd.is_empty() || matches!(op.as_str(), "write_file" | "patch" | "modify" | "delete_file") {
                    let msg = ">> [LAZARUS PROTOCOL] Mutating action detected. Securing DNA snapshot...";
                    println!("\x1b[33m{}\x1b[0m", msg);
                    let timestamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
                    let _ = Command::new("sh").args(["-c", &format!("mkdir -p .lazarus_vault/{} && cp -r src Cargo.toml .lazarus_vault/{}/", timestamp, timestamp)]).output();
                    println!("\x1b[32m>> [LAZARUS PROTOCOL] Snapshot {} secured.\x1b[0m", timestamp);
                }

                // === SUTURE: THE UNIVERSAL TRIGGER (NO WHITELIST) ===
                if !cmd.is_empty() {
                    println!("\n\x1b[33m>> [SYS_EXEC] Spawning shell process: {}\x1b[0m", cmd);
                    match Command::new("sh").args(["-c", cmd]).output() {
                        Ok(output) => {
                            let stdout = String::from_utf8_lossy(&output.stdout);
                            let stderr = String::from_utf8_lossy(&output.stderr);
                            if !stdout.is_empty() { println!("\x1b[32m{}\x1b[0m", stdout.trim()); }
                            if !stderr.is_empty() { println!("\x1b[31m{}\x1b[0m", stderr.trim()); }
                            println!("\x1b[33m>> [SYS_EXEC] Process Terminated.\x1b[0m\n");
                        }
                        Err(e) => println!("\x1b[31m!! [SYS_EXEC FAULT] {}\x1b[0m", e),
                    }
                }
            }
            println!("\n{}", table);
        } else {
            println!("\x1b[31m!! [EXECUTOR FAULT] Failed to parse JSON matrix.\x1b[0m\n{}", json_text);
        }
        Ok(())
    }
}
