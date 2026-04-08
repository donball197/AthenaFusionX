use clap::{Parser, Subcommand};
use std::path::PathBuf;

/// NEXUS SINGULARITY CLI — v1.5.0
#[derive(Parser, Debug)]
#[command(name = "nexus", about = "NEXUS SOVEREIGN INTERFACE", disable_help_subcommand = true)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// build: Orchestrates a target build.
    Build { #[arg(default_value = ".")] path: PathBuf },
    /// heal: Initiates neural repair on a specific file.
    Heal { #[arg(required = true)] file: PathBuf },
    /// open: Maps a file into the cortical workspace.
    Open { #[arg(required = true)] file: PathBuf },
    /// status: Polls system hardware resonance.
    Status,
    /// diff: Computes delta between two logic strands.
    Diff { #[arg(required = true)] original: PathBuf, #[arg(required = true)] against: PathBuf },
    /// restore: Reverts system state to a Lazarus snapshot.
    Restore { #[arg(required = true)] hash: String },
    /// close: Flushes a file from the workspace.
    Close { #[arg(required = true)] file: PathBuf },
}

// === SUTURE: EXPOSED CONTEXT INJECTOR FOR CLI & SERVER ===
pub fn inject_workspace_dna(cmd: &str) -> String {
    let lower_cmd = cmd.to_lowercase();
    let mut final_cmd = cmd.to_string();

    // If the user asks for a broad audit or review, attach the workspace DNA
    if lower_cmd.contains("audit") || lower_cmd.contains("review") || lower_cmd.contains("scan") {
        println!("\x1b[36m>> [CLI_INJECTOR] Broad analysis detected. Attaching local workspace DNA...\x1b[0m");
        let mut context = String::from("\n\n--- [SYSTEM WORKSPACE DNA] ---\n");
        
        if let Ok(entries) = std::fs::read_dir("src") {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("rs") {
                    if let Ok(content) = std::fs::read_to_string(&path) {
                        let preview = if content.len() > 15000 { &content[..15000] } else { &content };
                        context.push_str(&format!("\n// === FILE: {} ===\n{}\n", path.display(), preview));
                    }
                }
            }
        }
        
        // Also inject Cargo.toml if scanning for dependencies
        if lower_cmd.contains("cargo.toml") || lower_cmd.contains("depend") {
            if let Ok(cargo) = std::fs::read_to_string("Cargo.toml") {
                context.push_str(&format!("\n// === FILE: Cargo.toml ===\n{}\n", cargo));
            }
        }

        final_cmd.push_str(&context);
    }
    final_cmd
}

// === SUTURE: AUTONOMIC MUSCLE DETECTION ===
pub enum HostMuscle {
    MotorolaTermux,
    ChromebookCrostini,
    GenericARM64,
}

pub fn detect_host_muscle() -> HostMuscle {
    // Termux natively exports the PREFIX environment variable
    if std::env::var("PREFIX").unwrap_or_default().contains("com.termux") {
        return HostMuscle::MotorolaTermux;
    }
    
    // Chromebook/Crostini Linux kernels carry specific generic signatures
    if let Ok(kernel) = std::fs::read_to_string("/proc/version") {
        let k_lower = kernel.to_lowercase();
        if k_lower.contains("chrome") || k_lower.contains("cros") || k_lower.contains("penguin") {
            return HostMuscle::ChromebookCrostini;
        }
    }
    
    HostMuscle::GenericARM64
}

pub fn get_muscle_string(muscle: &HostMuscle) -> &'static str {
    match muscle {
        HostMuscle::MotorolaTermux => "MOTOROLA_EDGE [TERMUX SANDBOX]",
        HostMuscle::ChromebookCrostini => "CHROMEBOOK [NATIVE LINUX]",
        HostMuscle::GenericARM64 => "UNKNOWN [GENERIC ARM64]",
    }
}
