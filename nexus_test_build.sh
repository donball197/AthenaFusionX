#!/bin/bash

# Project Nexus Singularity v1.9 - Deep Validation & Build Suite
# Target: Root Setup Diagnostics

set -e

echo -e "\033[1;36m>> INITIATING ZERO FRAGMENTATION BUILD PROTOCOL [NEXUS v1.9]\033[0m"

# Phase 1: The Muscle Reset
# Critical when moving between Native Linux/Crostini and Motorola/Termux
echo -e "\n\033[1;33m[PHASE 1] Purging Device-Specific Build Artifacts...\033[0m"
rm -rf target/
rm -f *.so
echo -e "\033[1;32mWorkspace sterilized.\033[0m"

# Phase 2: Universal Source Validation
# Enforces strict formatting and linting across all src/ directories
echo -e "\n\033[1;33m[PHASE 2] Validating Universal src/ Logic...\033[0m"
cargo fmt --all -- --check || { echo -e "\033[31m[ERROR] Code formatting failed. Run 'cargo fmt' to fix.\033[0m"; exit 1; }
cargo clippy --all-targets --all-features -- -D warnings || { echo -e "\033[31m[ERROR] Clippy found issues. Please resolve warnings.\033[0m"; exit 1; }
echo -e "\033[1;32mFormatting and Clippy checks passed.\033[0m"

# Phase 3: Testing & Safety
# Runs all unit and integration tests across the workspace
echo -e "\n\033[1;33m[PHASE 3] Executing Workspace Test Suite...\033[0m"
cargo test --workspace || { echo -e "\033[31m[ERROR] Tests failed.\033[0m"; exit 1; }
echo -e "\033[1;32mAll tests passed.\033[0m"

# Phase 4: Release Compilation
# Deep compile of all binaries and crates optimized for the current ARM64 host
echo -e "\n\033[1;33m[PHASE 4] Compiling Release Binaries...\033[0m"
cargo build --release --workspace || { echo -e "\033[31m[ERROR] Release build failed.\033[0m"; exit 1; }
echo -e "\033[1;32mCompilation complete.\033[0m"

# Phase 5: CLI Initialization Test
# Dry run of the command-line interface to verify execution
echo -e "\n\033[1;33m[PHASE 5] Probing nexus_cli Engine...\033[0m"
if [ -f "./target/release/nexus_cli" ]; then
    ./target/release/nexus_cli --help || echo -e "\033[90m[NOTE] CLI help flag not yet implemented or returned non-zero.\033[0m"
    echo -e "\n\033[1;35m>> BUILD SUITE COMPLETE. CLI READY FOR GEMMA 4 31B-IT ROUTING.\033[0m"
else
    echo -e "\033[31m[ERROR] nexus_cli binary not found in target/release/.\033[0m"
    exit 1

