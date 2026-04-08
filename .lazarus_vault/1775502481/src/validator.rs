use std::process::Command;
use crate::nexus_error::NexusError;

pub struct Validator;

impl Validator {
    pub fn run_rust_checks() -> Result<(), NexusError> {
        println!(">> [VALIDATOR] Enforcing Rust Syntax & Linting...");
        
        // 1. Syntax Check
        let check = Command::new("cargo")
            .args(["check", "--target", "aarch64-unknown-linux-gnu", "-j", "2"])
            .status()
            .map_err(|e| NexusError::Other(format!("Toolchain missing: {}", e)))?;

        if !check.success() {
            return Err(NexusError::Other("SYNTAX_FAULT: Code failed cargo check.".into()));
        }

        // 2. Clippy Enforcement
        let clippy = Command::new("cargo")
            .args(["clippy", "--target", "aarch64-unknown-linux-gnu", "--", "-D", "warnings"])
            .status()
            .map_err(|e| NexusError::Other(format!("Clippy fault: {}", e)))?;

        if !clippy.success() {
            return Err(NexusError::Other("LINT_FAULT: Code rejected by Clippy.".into()));
        }

        Ok(())
    }

    pub fn run_ui_checks() -> Result<(), NexusError> {
        println!(">> [VALIDATOR] Enforcing UI Linting...");
        let status = Command::new("npm")
            .current_dir("ui")
            .args(["run", "lint"])
            .status()
            .map_err(|_| NexusError::Other("NPM missing".into()))?;

        if !status.success() {
            return Err(NexusError::Other("UI_FAULT: Svelte linting failed.".into()));
        }
        Ok(())
    }
}
