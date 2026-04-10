/**
 * Copyright 2026 Donald Ball
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 * http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR REPRESENTATIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */


use anyhow::{anyhow, Context, Result};
use std::process::Command;

pub struct Validator;

impl Validator {
    pub fn run_rust_checks() -> Result<()> {
        println!(">> [VALIDATOR] Enforcing Rust Syntax...");
        let check = Command::new("cargo")
            .args(["check", "--target", "aarch64-unknown-linux-gnu", "-j", "2"])
            .status()
            .context("Failed to run cargo check toolchain")?;

        if !check.success() {
            return Err(anyhow!(
                "SYNTAX_FAULT: Code failed cargo check. Check your braces and types."
            ));
        }

        println!(">> [VALIDATOR] Enforcing Clippy Lints...");
        let clippy = Command::new("cargo")
            .args(["clippy", "--target", "aarch64-unknown-linux-gnu", "--", "-D", "warnings"])
            .status()
            .context("Failed to run cargo clippy")?;

        if !clippy.success() {
            return Err(anyhow!("LINT_FAULT: Code rejected by Clippy (Strict Mode)."));
        }
        Ok(())
    }

    pub fn run_ui_checks() -> Result<()> {
        println!(">> [VALIDATOR] Enforcing UI Linting...");
        let status = Command::new("npm")
            .current_dir("ui")
            .args(["run", "lint"])
            .status()
            .context("Failed to run NPM linting toolchain")?;

        if !status.success() {
            return Err(anyhow!("UI_FAULT: Svelte/Vite linting failed."));
        }
        Ok(())
    }
}
