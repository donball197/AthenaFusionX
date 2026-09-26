#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$REPO_ROOT"

echo "=========================================================="
echo ">> PATH-AWARE CONTRACT-ADVANCE DRY-RUN"
echo ">> Fix: action_advances_unmet_contract_condition"
echo ">> 6 Cases — isolated temporary Cargo project"
echo "=========================================================="

TEST_DIR="$(mktemp -d /tmp/athena_path_aware_XXXXXX)"
trap 'rm -rf "$TEST_DIR"' EXIT

cat << 'RUST_EOF' > "$TEST_DIR/Cargo.toml"
[package]
name = "path_aware_test"
version = "0.1.0"
edition = "2021"

[dependencies]
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
anyhow = "1.0"
RUST_EOF

mkdir -p "$TEST_DIR/src"

cat << 'RUST_EOF' > "$TEST_DIR/src/main.rs"
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AgentAction {
    ReadFile { path: String, reasoning: String },
    ReadFileChunk { path: String, start_line: u64, max_lines: u64, reasoning: String },
    WriteFile { path: String, content: String, reasoning: String },
    SynthesizeReport { output_path: String, source_files: Vec<String>, reasoning: String },
    ExecuteCommand { command: String, reasoning: String },
    Halt { reasoning: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompletionCondition {
    ReadMarkerAcquired,
    WriteMarkerAcquired,
    CommandMarkerAcquired,
}

/// Extract the first `<marker><target>` line's target from the mission text.
/// Same pattern as epistemic_engine::generate_hypotheses uses for its
/// deterministic bindings (READ_OK:, WRITE_OK:, etc.).
fn extract_named_target(mission: &str, marker: &str) -> Option<String> {
    let pos = mission.find(marker)?;
    let remainder = &mission[pos + marker.len()..];
    let target = remainder
        .lines()
        .next()?
        .trim()
        .trim_matches('`')
        .trim_matches('"');
    if target.is_empty() {
        None
    } else {
        Some(target.to_string())
    }
}

/// Current implementation: action-type only, path-blind.
fn advances_old(cond: &CompletionCondition, action: &AgentAction) -> bool {
    match cond {
        CompletionCondition::ReadMarkerAcquired => matches!(
            action,
            AgentAction::ReadFile { .. } | AgentAction::ReadFileChunk { .. }
        ),
        CompletionCondition::WriteMarkerAcquired => matches!(
            action,
            AgentAction::WriteFile { .. } | AgentAction::SynthesizeReport { .. }
        ),
        CompletionCondition::CommandMarkerAcquired => {
            matches!(action, AgentAction::ExecuteCommand { .. })
        }
    }
}

/// Corrected: when the mission names a concrete target, the action's
/// parameter must match that target. When the mission is generic (no named
/// target), the relaxation remains type-only — which is its intended role
/// as a multi-criteria safety net.
fn advances_new(cond: &CompletionCondition, action: &AgentAction, mission: &str) -> bool {
    match cond {
        CompletionCondition::ReadMarkerAcquired => {
            match extract_named_target(mission, "READ_OK:") {
                Some(target) => {
                    matches!(
                        action,
                        AgentAction::ReadFile { path, .. } if path.trim() == target
                    ) || matches!(
                        action,
                        AgentAction::ReadFileChunk { path, .. } if path.trim() == target
                    )
                }
                None => matches!(
                    action,
                    AgentAction::ReadFile { .. } | AgentAction::ReadFileChunk { .. }
                ),
            }
        }
        CompletionCondition::WriteMarkerAcquired => {
            match extract_named_target(mission, "WRITE_OK:") {
                Some(target) => {
                    matches!(
                        action,
                        AgentAction::WriteFile { path, .. } if path.trim() == target
                    ) || matches!(
                        action,
                        AgentAction::SynthesizeReport { output_path, .. }
                            if output_path.trim() == target
                    )
                }
                None => matches!(
                    action,
                    AgentAction::WriteFile { .. } | AgentAction::SynthesizeReport { .. }
                ),
            }
        }
        CompletionCondition::CommandMarkerAcquired => {
            matches!(action, AgentAction::ExecuteCommand { .. })
        }
    }
}

fn main() {
    println!("==========================================================");
    println!(">> RUNNING 6 PATH-AWARE CONTRACT-ADVANCE SCENARIOS");
    println!("==========================================================");

    let mission_named_read = "\
OBJECTIVE: inspect the loop
MANDATORY NODE SUCCESS CRITERIA:
READ_OK:crates/runtime_core/src/cortical_loop.rs
NODE COMPLETION RULE: ...";

    let mission_generic = "\
OBJECTIVE: inspect the code and report.";

    let mission_named_write = "\
OBJECTIVE: produce the report
MANDATORY NODE SUCCESS CRITERIA:
WRITE_OK:reports/final.md";

    let read_correct = AgentAction::ReadFile {
        path: "crates/runtime_core/src/cortical_loop.rs".into(),
        reasoning: "on-strategy".into(),
    };

    let read_wrong_path = AgentAction::ReadFile {
        path: "compiler_remediation_final_audit.md".into(),
        reasoning: "off-strategy but same action type".into(),
    };

    let write_correct = AgentAction::WriteFile {
        path: "reports/final.md".into(),
        content: "# ok".into(),
        reasoning: "on-strategy".into(),
    };

    let write_wrong_path = AgentAction::WriteFile {
        path: "other.md".into(),
        content: "# off".into(),
        reasoning: "off-strategy".into(),
    };

    let cmd = AgentAction::ExecuteCommand {
        command: "cargo check".into(),
        reasoning: "not a read".into(),
    };

    // CASE 1: correct path read against a named READ_OK — both old and new advance
    println!("\n>> [CASE 1] Named READ_OK + correct-path ReadFile");
    assert!(advances_old(&CompletionCondition::ReadMarkerAcquired, &read_correct));
    assert!(advances_new(
        &CompletionCondition::ReadMarkerAcquired,
        &read_correct,
        mission_named_read
    ));
    println!("   \x1b[32m✔ PASS\x1b[0m: Both old and new advance (behavior preserved on the happy path).");

    // CASE 2: wrong-path read against a named READ_OK — this is the bug fixed.
    println!("\n>> [CASE 2] Named READ_OK + wrong-path ReadFile");
    assert!(advances_old(&CompletionCondition::ReadMarkerAcquired, &read_wrong_path));
    assert!(!advances_new(
        &CompletionCondition::ReadMarkerAcquired,
        &read_wrong_path,
        mission_named_read
    ));
    println!("   \x1b[32m✔ PASS\x1b[0m: Old advanced (bug); new rejects. This is the turn-1 failure from the 2026-09-14 audit run.");

    // CASE 3: generic mission, no READ_OK — relaxation must be preserved.
    println!("\n>> [CASE 3] No READ_OK in mission + any ReadFile");
    assert!(advances_old(&CompletionCondition::ReadMarkerAcquired, &read_wrong_path));
    assert!(advances_new(
        &CompletionCondition::ReadMarkerAcquired,
        &read_wrong_path,
        mission_generic
    ));
    println!("   \x1b[32m✔ PASS\x1b[0m: Type-only relaxation retained when no path is named.");

    // CASE 4: correct-path write against a named WRITE_OK.
    println!("\n>> [CASE 4] Named WRITE_OK + correct-path WriteFile");
    assert!(advances_old(&CompletionCondition::WriteMarkerAcquired, &write_correct));
    assert!(advances_new(
        &CompletionCondition::WriteMarkerAcquired,
        &write_correct,
        mission_named_write
    ));
    println!("   \x1b[32m✔ PASS\x1b[0m: Both advance on the happy write path.");

    // CASE 5: wrong-path write against a named WRITE_OK.
    println!("\n>> [CASE 5] Named WRITE_OK + wrong-path WriteFile");
    assert!(advances_old(&CompletionCondition::WriteMarkerAcquired, &write_wrong_path));
    assert!(!advances_new(
        &CompletionCondition::WriteMarkerAcquired,
        &write_wrong_path,
        mission_named_write
    ));
    println!("   \x1b[32m✔ PASS\x1b[0m: Wrong-path write no longer advances the criterion.");

    // CASE 6: non-matching action type — both reject.
    println!("\n>> [CASE 6] Named READ_OK + ExecuteCommand");
    assert!(!advances_old(&CompletionCondition::ReadMarkerAcquired, &cmd));
    assert!(!advances_new(
        &CompletionCondition::ReadMarkerAcquired,
        &cmd,
        mission_named_read
    ));
    println!("   \x1b[32m✔ PASS\x1b[0m: Non-read actions do not satisfy a read criterion under either version.");

    println!("\n==========================================================");
    println!("\x1b[1;32m>> ALL 6 PATH-AWARE CASES PASSED!\x1b[0m");
    println!("==========================================================");
}
RUST_EOF

cargo run --manifest-path "$TEST_DIR/Cargo.toml" --quiet
