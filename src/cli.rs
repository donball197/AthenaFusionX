use clap::{Parser, Subcommand};
use std::path::PathBuf;

/// NEXUS SINGULARITY CLI — v1.5.0
/// AUDITOR FIX: Using PathBuf for type-safe file handling.
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
