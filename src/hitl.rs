use crate::agent_protocol::AgentAction;
use std::io::{self, Write};

pub fn check_interception(action: &AgentAction) -> Result<Option<String>, String> {
    let mode = std::env::var("NEXUS_MODE").unwrap_or_else(|_| "ghost".to_string());
    if mode != "interactive" { return Ok(None); }

    match action {
        AgentAction::ExecuteCommand { command, reasoning } => {
            println!("\n\x1b[1;33m[⚠️ HITL INTERCEPT] Action: ExecuteCommand\x1b[0m");
            println!("\x1b[1;36mReasoning:\x1b[0m {}", reasoning);
            println!("\x1b[1;36mCommand:\x1b[0m {}", command);
        }
        AgentAction::WriteFile { path, content, reasoning } => {
            println!("\n\x1b[1;33m[⚠️ HITL INTERCEPT] Action: WriteFile\x1b[0m");
            println!("\x1b[1;36mReasoning:\x1b[0m {}", reasoning);
            println!("\x1b[1;36mPath:\x1b[0m {}", path);
            println!("\x1b[1;36mContent Preview:\x1b[0m {}...", content.chars().take(100).collect::<String>().replace('\n', " "));
        }
        _ => return Ok(None),
    }

    loop {
        print!("\x1b[1;35mAllow execution? [y/N/f (feedback)]: \x1b[0m");
        io::stdout().flush().unwrap();
        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();
        
        match input.trim().to_lowercase().as_str() {
            "y" | "yes" => return Ok(None),
            "n" | "no" | "" => return Err("Execution denied by Operator.".to_string()),
            "f" | "feedback" => {
                print!("\x1b[1;35mProvide correction: \x1b[0m");
                io::stdout().flush().unwrap();
                let mut f = String::new();
                io::stdin().read_line(&mut f).unwrap();
                return Err(format!("Execution denied. Feedback: {}", f.trim()));
            }
            _ => println!("Invalid input."),
        }
    }
}
