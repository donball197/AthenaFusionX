#!/bin/bash
set -e

echo "=================================================="
echo ">> ATHENAFUSIONX REFACTOR MASTER SCRIPT"
echo ">> Phases: 1(cleanup) → 3(api) → 2(verify) → 4(types) → 5(guards) → 6(tests)"
echo "=================================================="

# ---------------------------------------------------------
# PHASE 1: Dead Code Purge
# ---------------------------------------------------------
echo -e "\n=== Phase 1: Dead Code Purge ==="

echo ">> Checking for lingering dependencies..."
DEAD_CRATES=("cognitive_core" "nexus_telemetry" "nexus_script_parser" "executor" "identity" "memory_graph" "mesh" "protocol")
SAFE_TO_DELETE=()

for crate in "${DEAD_CRATES[@]}"; do
    COUNT=$(rg -c "nexus_${crate}|crate::${crate}|${crate}::" --type rust -g '!target' -g '!Cargo.toml' 2>/dev/null | wc -l || true)
    if [ "$COUNT" -gt "0" ]; then
        echo "⚠️  crates/${crate} has ${COUNT} references — preserving."
    else
        echo "✅ crates/${crate} — safe to delete."
        SAFE_TO_DELETE+=("crates/${crate}")
    fi
done

# 1b. Remove from Cargo.toml via pure Python
python3 << 'PYEOF'
import re
from pathlib import Path

path = Path("Cargo.toml")
content = path.read_text()

crates_to_remove = [
    "crates/cognitive_core",
    "crates/nexus_telemetry",
    "crates/nexus_script_parser",
    "crates/executor",
    "crates/identity",
    "crates/memory_graph",
    "crates/mesh",
    "crates/protocol"
]

for crate in crates_to_remove:
    content = re.sub(rf'\s*"{re.escape(crate)}",?', '', content)

path.write_text(content)
print("✅ Removed dead crates from Cargo.toml")
PYEOF

# 1c. Delete dead crate directories
for crate in "${SAFE_TO_DELETE[@]}"; do
    if [ -d "$crate" ]; then
        rm -rf "$crate"
        echo "✅ Deleted $crate/"
    fi
done

# 1d. Delete dead belief files
python3 << 'PYEOF'
from pathlib import Path

dead_belief_files = [
    "belief_audit.rs", "belief_causality.rs", "belief_delta.rs",
    "belief_feedback.rs", "belief_policy.rs", "belief_prediction.rs",
    "belief_transition.rs", "belief_update.rs", "belief_validation.rs",
    "cognition_loop.rs",
]

base = Path("crates/runtime_core/src")
for f in dead_belief_files:
    p = base / f
    if p.exists():
        p.unlink()
        print(f"✅ Deleted {p}")

lib_path = base / "lib.rs"
if lib_path.exists():
    code = lib_path.read_text()
    for mod_name in dead_belief_files:
        mod_name = mod_name.replace(".rs", "")
        code = code.replace(f"pub mod {mod_name};\n", "")
    lib_path.write_text(code)
    print("✅ Cleaned lib.rs")
PYEOF

if [ -f "builder.rs" ] && grep -q "404" builder.rs 2>/dev/null; then
    rm builder.rs
    echo "✅ Deleted builder.rs"
fi

cargo check --workspace 2>&1 | tail -3
echo ">> Phase 1 Complete."

# ---------------------------------------------------------
# PHASE 3: Fix safe_api_call (Status + Logging)
# ---------------------------------------------------------
echo -e "\n=== Phase 3: Fix safe_api_call ==="

python3 << 'PYEOF'
from pathlib import Path

path = Path("crates/runtime_core/src/epistemic_engine.rs")
code = path.read_text()

# Ensure Duration is imported
if "use tokio::time::Duration;" not in code and "use std::time::Duration;" not in code:
    lines = code.split('\n')
    last_use_idx = 0
    for i, line in enumerate(lines):
        if line.startswith("use ") and not line.startswith("use crate"):
            last_use_idx = i
    lines.insert(last_use_idx + 1, "use tokio::time::Duration;")
    code = '\n'.join(lines)
    print("✅ Added Duration import")

new_block = """                tracing::debug!("[API] POST {} model={}", url, model_endpoint);

                let res = req.send().await?;
                let status = res.status();
                let text_body = match res.text().await {
                    Ok(t) => t,
                    Err(e) => {
                        last_err = anyhow::anyhow!("Network Read Error: {}", e);
                        retries += 1;
                        continue;
                    }
                };

                if !status.is_success() {
                    let preview = &text_body[..std::cmp::min(500, text_body.len())];
                    tracing::warn!(
                        "[API] {} {} model={} status={} body={}",
                        "POST", url, model_endpoint, status, preview
                    );

                    let should_retry = status.as_u16() == 429
                        || status.as_u16() == 503
                        || status.is_server_error();

                    last_err = anyhow::anyhow!(
                        "HTTP {} from {} (model={}): {}",
                        status, url, model_endpoint, preview
                    );

                    if should_retry && retries < 12 {
                        retries += 1;
                        let backoff = std::cmp::min(2_u64.pow(retries as u32) * 2, 45);
                        tokio::time::sleep(Duration::from_secs(backoff)).await;
                        continue;
                    }
                    break;
                }

                let body: Value = match serde_json::from_str(&text_body) {
                    Ok(b) => b,
                    Err(e) => {
                        eprintln!("[API GATEWAY RAW ERROR BODY]:\\n{}", text_body);
                        last_err = anyhow::anyhow!("JSON Parse Error: {} | Raw Response: {}", e, &text_body[..std::cmp::min(200, text_body.len())]);
                        retries += 1;
                        if retries > 2 {
                            break;
                        }
                        tokio::time::sleep(Duration::from_secs(2)).await;
                        continue;
                    }
                };"""

if "let res = req.send().await?;" in code and "text_body" in code:
    start_idx = code.find("                let res = req.send().await?;")
    end_marker = "                };\n\n                // Error Handling"
    end_idx = code.find(end_marker, start_idx)
    if end_idx == -1:
        end_marker = "                };\n\n                // ⚡ NATIVE"
        end_idx = code.find(end_marker, start_idx)
    if end_idx == -1:
        end_marker = "                };\n"
        end_idx = code.find(end_marker, start_idx)
    
    if end_idx > start_idx:
        end_idx += len("                };")
        code = code[:start_idx] + new_block + code[end_idx:]
        path.write_text(code)
        print("✅ Phase 3: safe_api_call patched successfully")
    else:
        print("❌ Phase 3: Could not locate block boundaries")
else:
    print("❌ Phase 3: Block not found at all")
PYEOF

cargo check -p nexus_runtime_core 2>&1 | tail -3
echo ">> Phase 3 Complete."

# ---------------------------------------------------------
# PHASE 2: Language-Aware Post-Mutation Verify
# ---------------------------------------------------------
echo -e "\n=== Phase 2: Language-Aware Post-Mutation Verify ==="

python3 << 'PYEOF'
from pathlib import Path

path = Path("crates/runtime_core/src/cortical_loop.rs")
code = path.read_text()

# 1. Add imports
if "use std::os::unix::process::ExitStatusExt;" not in code:
    lines = code.split('\n')
    last_use_idx = 0
    for i, line in enumerate(lines):
        if line.startswith("use ") and not line.startswith("use crate"):
            last_use_idx = i
    lines.insert(last_use_idx + 1, "use std::os::unix::process::ExitStatusExt;")
    lines.insert(last_use_idx + 2, "use std::process::ExitStatus;")
    code = '\n'.join(lines)
    print("✅ Added ExitStatusExt + ExitStatus imports")

# 2. Replace the gate
old_gate = "        // ⚡ COMPILER AS A CRITIC + DETERMINISTIC AUTO-HEALER\n        if rust_files_mutated {"

new_gate = """        // ⚡ LANGUAGE-AWARE POST-MUTATION VERIFY
        let is_rust = mutation_paths.iter().any(|p| p.ends_with(".rs"));
        let is_python = mutation_paths.iter().any(|p| p.ends_with(".py"));
        let is_js = mutation_paths.iter().any(|p| p.ends_with(".js") || p.ends_with(".ts"));
        let needs_verification = is_rust || is_python || is_js;
        
        if needs_verification {"""

if old_gate in code:
    code = code.replace(old_gate, new_gate)
    print("✅ Replaced gate with language-aware dispatch")

# 3. Replace cargo check invocation
old_cargo = """                let compile_output = tokio::process::Command::new("cargo")
                    .arg("check")
                    .arg("--message-format=json")
                    .output()
                    .await;"""

new_cargo = """                let compile_output = if is_rust {
                    tokio::process::Command::new("cargo")
                        .arg("check")
                        .arg("--message-format=json")
                        .output()
                        .await
                } else if is_python {
                    let mut all_ok = true;
                    let mut combined_stderr = String::new();
                    for p in &mutation_paths {
                        if p.ends_with(".py") {
                            match tokio::process::Command::new("python3")
                                .arg("-m").arg("py_compile")
                                .arg(p)
                                .output()
                                .await
                            {
                                Ok(o) if !o.status.success() => {
                                    all_ok = false;
                                    combined_stderr.push_str(&format!("{}: {}\\n",
                                        p, String::from_utf8_lossy(&o.stderr)));
                                }
                                Ok(_) => {}
                                Err(e) => {
                                    all_ok = false;
                                    combined_stderr.push_str(&format!("{}: python3 not found: {}\\n", p, e));
                                }
                            }
                        }
                    }
                    Ok(std::process::Output {
                        status: unsafe { ExitStatus::from_raw(if all_ok { 0 } else { 1 }) },
                        stdout: Vec::new(),
                        stderr: combined_stderr.into_bytes(),
                    })
                } else {
                    let mut all_ok = true;
                    let mut combined_stderr = String::new();
                    for p in &mutation_paths {
                        if p.ends_with(".js") || p.ends_with(".ts") {
                            match tokio::process::Command::new("node")
                                .arg("--check")
                                .arg(p)
                                .output()
                                .await
                            {
                                Ok(o) if !o.status.success() => {
                                    all_ok = false;
                                    combined_stderr.push_str(&format!("{}: {}\\n",
                                        p, String::from_utf8_lossy(&o.stderr)));
                                }
                                Ok(_) => {}
                                Err(e) => {
                                    all_ok = false;
                                    combined_stderr.push_str(&format!("{}: node not found: {}\\n", p, e));
                                }
                            }
                        }
                    }
                    Ok(std::process::Output {
                        status: unsafe { ExitStatus::from_raw(if all_ok { 0 } else { 1 }) },
                        stdout: Vec::new(),
                        stderr: combined_stderr.into_bytes(),
                    })
                };"""

if old_cargo in code:
    code = code.replace(old_cargo, new_cargo)
    print("✅ Replaced cargo check with language-aware dispatch")

# 4. Update success message
old_success = 'latest_evidence.push_str("\\n[COMPILE_OK] Workspace compiles successfully.");'
new_success = """let verifier_name = if is_rust { "cargo check" }
                    else if is_python { "py_compile" }
                    else { "node --check" };
                    latest_evidence.push_str(&format!("\\n[VERIFY_OK] {} passed.", verifier_name));"""

if old_success in code:
    code = code.replace(old_success, new_success)
    print("✅ Updated success message to [VERIFY_OK]")

# 5. Guard auto-heal block
old_heal_start = """                        let mut applied_heals = 0;
                        
                        // Parse cargo JSON output line by line
                        for line in stdout.lines() {"""

new_heal_start = """                        let mut applied_heals = 0;
                        
                        // Auto-heal only works for Rust (cargo JSON diagnostics)
                        if is_rust {
                        for line in stdout.lines() {"""

if old_heal_start in code:
    code = code.replace(old_heal_start, new_heal_start)
    print("✅ Guarded auto-heal with if is_rust")

# 6. Close auto-heal block
old_heal_end = """                        if applied_heals > 0 {
                            latest_evidence.push_str(&format!("\\n[AUTO_HEAL] Applied {} deterministic fixes. Re-checking...", applied_heals));
                            session.local_evidence.push_str(&latest_evidence);
                            continue; // Re-run cargo check
                        }"""

new_heal_end = """                        if applied_heals > 0 {
                            latest_evidence.push_str(&format!("\\n[AUTO_HEAL] Applied {} deterministic fixes. Re-checking...", applied_heals));
                            session.local_evidence.push_str(&latest_evidence);
                            continue; // Re-run cargo check
                        }
                        } // end if is_rust"""

if old_heal_end in code:
    code = code.replace(old_heal_end, new_heal_end)
    print("✅ Closed if is_rust block")

path.write_text(code)
PYEOF

cargo check -p nexus_runtime_core 2>&1 | tail -3
echo ">> Phase 2 Complete."

# ---------------------------------------------------------
# PHASE 4: ExecutionResult + EvidenceLog (Additive)
# ---------------------------------------------------------
echo -e "\n=== Phase 4: ExecutionResult Types ==="

cat << 'EOF' > crates/runtime_core/src/execution_result.rs
//! Structured execution results — replaces raw String returns from RoboticExecutor.
//! Additive module: does not break existing code. Wire incrementally.

use crate::agent_protocol::AgentAction;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub enum ExecutionResult {
    Success { marker: EvidenceMarker, output: String },
    CompilerFault { stderr: String, auto_healed: bool },
    Reverted { reason: String, critique: String },
    ActionFailed { error: String },
    Blocked { reason: String },
    Halted { reasoning: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EvidenceMarker {
    ReadOk, ReadChunkOk, SearchCodeOk, SearchOk, CacheHit, CmdOk,
    WriteOk, WriteNoop, DiffPatchOk, PatchFunctionOk, ReplaceLinesOk,
    VerifyOk, CompileOk, CompileFail, AutoHeal, MutationReverted,
    ActionFailed, ActionGateBlocked, Halt,
}

impl EvidenceMarker {
    pub fn as_bracketed(&self) -> &'static str {
        match self {
            Self::ReadOk => "[READ_OK]", Self::ReadChunkOk => "[READ_CHUNK_OK]",
            Self::SearchCodeOk => "[SEARCH_CODE_OK]", Self::SearchOk => "[SEARCH_OK]",
            Self::CacheHit => "[CACHE_HIT]", Self::CmdOk => "[CMD_OK]",
            Self::WriteOk => "[WRITE_OK]", Self::WriteNoop => "[WRITE_NOOP]",
            Self::DiffPatchOk => "[DIFF_PATCH_OK]", Self::PatchFunctionOk => "[PATCH_FUNCTION_OK]",
            Self::ReplaceLinesOk => "[REPLACE_LINES_OK]", Self::VerifyOk => "[VERIFY_OK]",
            Self::CompileOk => "[COMPILE_OK]", Self::CompileFail => "[COMPILE_FAIL]",
            Self::AutoHeal => "[AUTO_HEAL]", Self::MutationReverted => "[MUTATION_REVERTED]",
            Self::ActionFailed => "[ACTION_FAILED]", Self::ActionGateBlocked => "[ACTION_GATE_BLOCKED]",
            Self::Halt => "[HALT]",
        }
    }
    
    pub fn is_mutation_success(&self) -> bool {
        matches!(self, Self::WriteOk | Self::DiffPatchOk | Self::PatchFunctionOk | Self::ReplaceLinesOk)
    }
    pub fn is_mutation_failure(&self) -> bool {
        matches!(self, Self::MutationReverted | Self::CompileFail | Self::ActionFailed)
    }
    pub fn is_recon(&self) -> bool {
        matches!(self, Self::ReadOk | Self::ReadChunkOk | Self::SearchCodeOk | Self::SearchOk | Self::CacheHit | Self::CmdOk)
    }
}

impl ExecutionResult {
    pub fn as_evidence_text(&self) -> String {
        match self {
            Self::Success { marker, output } => {
                if output.trim().is_empty() { marker.as_bracketed().to_string() }
                else { format!("{} {}", marker.as_bracketed(), output) }
            }
            Self::CompilerFault { stderr, auto_healed } => {
                if *auto_healed { "[AUTO_HEAL] Compiler error auto-fixed. Re-checking...".to_string() }
                else { format!("[COMPILE_FAIL] Mutation caused compilation error:\n{}", stderr) }
            }
            Self::Reverted { reason, critique } => format!("[MUTATION_REVERTED] {} -- Evaluator rejected: {}", reason, critique),
            Self::ActionFailed { error } => format!("[ACTION_FAILED] OS rejected action: {}", error),
            Self::Blocked { reason } => format!("[ACTION_GATE_BLOCKED] {}", reason),
            Self::Halted { reasoning } => format!("[HALT] {}", reasoning),
        }
    }
    
    pub fn marker(&self) -> Option<EvidenceMarker> {
        match self {
            Self::Success { marker, .. } => Some(*marker),
            Self::CompilerFault { auto_healed, .. } => if *auto_healed { Some(EvidenceMarker::AutoHeal) } else { Some(EvidenceMarker::CompileFail) },
            Self::Reverted { .. } => Some(EvidenceMarker::MutationReverted),
            Self::ActionFailed { .. } => Some(EvidenceMarker::ActionFailed),
            Self::Blocked { .. } => Some(EvidenceMarker::ActionGateBlocked),
            Self::Halted { .. } => Some(EvidenceMarker::Halt),
        }
    }
    
    pub fn is_mutation_success(&self) -> bool { self.marker().map(|m| m.is_mutation_success()).unwrap_or(false) }
    pub fn is_mutation_failure(&self) -> bool { self.marker().map(|m| m.is_mutation_failure()).unwrap_or(false) }
    pub fn is_recon(&self) -> bool { self.marker().map(|m| m.is_recon()).unwrap_or(false) }
}
EOF

python3 -c "
from pathlib import Path
p = Path('crates/runtime_core/src/lib.rs')
c = p.read_text()
if 'pub mod execution_result;' not in c:
    c += '\npub mod execution_result;\n'
    p.write_text(c)
    print('✅ Added execution_result to lib.rs')
"

cargo check -p nexus_runtime_core 2>&1 | tail -3
echo ">> Phase 4 Complete."

# ---------------------------------------------------------
# PHASE 5: ActionGuard Pipeline (Additive)
# ---------------------------------------------------------
echo -e "\n=== Phase 5: ActionGuard Pipeline ==="

cat << 'EOF' > crates/runtime_core/src/guards.rs
//! ActionGuard pipeline — additive module.

use crate::agent_protocol::AgentAction;
use crate::cognition::epistemic_session::EpistemicSession;

#[derive(Debug, Clone)]
pub struct GuardVeto {
    pub guard_name: String,
    pub reason: String,
    pub severity: VetoSeverity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VetoSeverity { Pivot, Warn }

pub trait ActionGuard: Send + Sync {
    fn name(&self) -> &'static str;
    fn validate(&self, action: &AgentAction, session: &EpistemicSession) -> Result<(), GuardVeto>;
}

pub struct AntiLoopGuard { pub max_recon: u32, pub max_recon_post_mutation: u32 }
impl Default for AntiLoopGuard { fn default() -> Self { Self { max_recon: 2, max_recon_post_mutation: 4 } } }

impl ActionGuard for AntiLoopGuard {
    fn name(&self) -> &'static str { "AntiLoop" }
    fn validate(&self, action: &AgentAction, session: &EpistemicSession) -> Result<(), GuardVeto> {
        let (action_key, is_recon) = extract_action_key(action);
        let mutation_done = session.local_evidence.contains("[WRITE_OK]")
            || session.local_evidence.contains("[DIFF_PATCH_OK]")
            || session.local_evidence.contains("[PATCH_FUNCTION_OK]")
            || session.local_evidence.contains("[REPLACE_LINES_OK]");
        let limit: u32 = if is_recon && mutation_done { self.max_recon_post_mutation } else { self.max_recon };
        let attempt_count = session.action_attempt_count(&action_key).unwrap_or(0);
        if attempt_count >= limit {
            return Err(GuardVeto {
                guard_name: self.name().to_string(),
                reason: format!("Action [{}] attempted {} times (limit {}). FORCING STRATEGY PIVOT.", action_key, attempt_count, limit),
                severity: VetoSeverity::Pivot,
            });
        }
        Ok(())
    }
}

pub struct SecurityGuard;
impl ActionGuard for SecurityGuard {
    fn name(&self) -> &'static str { "Security" }
    fn validate(&self, action: &AgentAction, _session: &EpistemicSession) -> Result<(), GuardVeto> {
        match action {
            AgentAction::ExecuteCommand { command, .. } => {
                let lower = command.to_lowercase();
                let dangerous = ["rm -rf /", "rm -rf ~", "mkfs", "chmod 777 /", "/etc/shadow", "mv /etc/", ":(){ :|:& };:", "dd if=/dev/zero", "fork bomb", "sudo su", "curl | bash", "wget | bash"];
                for pat in &dangerous {
                    if lower.contains(pat) {
                        return Err(GuardVeto { guard_name: self.name().to_string(), reason: format!("Dangerous pattern '{}' detected.", pat), severity: VetoSeverity::Pivot });
                    }
                }
            }
            AgentAction::WriteFile { path, content, .. } | AgentAction::DiffPatch { path, .. } | AgentAction::PatchFunction { path, .. } => {
                if path.contains("..") {
                    return Err(GuardVeto { guard_name: self.name().to_string(), reason: format!("Path traversal detected: '{}'", path), severity: VetoSeverity::Pivot });
                }
                if let AgentAction::WriteFile { content, .. } = action {
                    if content.contains("GEMINI_API_KEY=") || content.contains("NVIDIA_API_KEY=") {
                        return Err(GuardVeto { guard_name: self.name().to_string(), reason: "Plaintext API key leak blocked.".to_string(), severity: VetoSeverity::Pivot });
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }
}

fn extract_action_key(action: &AgentAction) -> (String, bool) {
    match action {
        AgentAction::ReadFile { path, .. } => (format!("read_file:{}", path), true),
        AgentAction::ReadFileChunk { path, start_line, .. } => (format!("read_file_chunk:{}:{}", path, start_line), true),
        AgentAction::SearchCode { path, query, .. } => (format!("search_code:{}:{}", path, query), true),
        AgentAction::SearchMemory { query, .. } => (format!("search_memory:{}", query), true),
        AgentAction::ExecuteCommand { command, .. } => (format!("execute_command:{}", command), false),
        AgentAction::WriteFile { path, .. } => (format!("write_file:{}", path), false),
        AgentAction::DiffPatch { path, .. } => (format!("diff_patch:{}", path), false),
        AgentAction::PatchFunction { path, function_name, .. } => (format!("patch_function:{}:{}", path, function_name), false),
        AgentAction::ReplaceLines { path, start_line, end_line, .. } => (format!("replace_lines:{}:{}:{}", path, start_line, end_line), false),
        _ => (format!("{:?}", action), false),
    }
}

pub struct GuardPipeline { guards: Vec<Box<dyn ActionGuard>> }
impl GuardPipeline {
    pub fn new() -> Self { Self { guards: vec![Box::new(SecurityGuard), Box::new(AntiLoopGuard::default())] } }
    pub fn validate(&self, action: &AgentAction, session: &EpistemicSession) -> Result<(), GuardVeto> {
        for guard in &self.guards {
            if let Err(veto) = guard.validate(action, session) { return Err(veto); }
        }
        Ok(())
    }
}
impl Default for GuardPipeline { fn default() -> Self { Self::new() } }
EOF

python3 -c "
from pathlib import Path
p = Path('crates/runtime_core/src/lib.rs')
c = p.read_text()
if 'pub mod guards;' not in c:
    c += '\npub mod guards;\n'
    p.write_text(c)
    print('✅ Added guards to lib.rs')
"

cargo check -p nexus_runtime_core 2>&1 | tail -3
echo ">> Phase 5 Complete."

# ---------------------------------------------------------
# PHASE 6: E2E Test Fixtures
# ---------------------------------------------------------
echo -e "\n=== Phase 6: E2E Test Fixtures ==="

mkdir -p crates/runtime_core/tests/fixtures/sqli_php/www
mkdir -p crates/runtime_core/tests/fixtures/cmdi_python

cat << 'EOF' > crates/runtime_core/tests/fixtures/sqli_php/www/login.php
<?php
$con = mysqli_connect("localhost", "root", "", "test");
if (!$con) { die("Connection failed"); }
if (!empty($_REQUEST['uid'])) {
    $username = $_REQUEST['uid'];
    $pass = $_REQUEST['password'];
    $q = "SELECT * FROM users where username='".$username."' AND password = '".md5($pass)."'" ;
    if (!mysqli_query($con,$q)) {
        echo 'Error: ' . mysqli_error($con);
    }else{
        $result = mysqli_query($con,$q);
        $row = mysqli_fetch_array($result);
        if ($row){ $_SESSION["username"] = $row[1]; }
    }
}
?>
EOF

cat << 'EOF' > crates/runtime_core/tests/fixtures/cmdi_python/server.py
import os
from flask import Flask, request

app = Flask(__name__)

@app.route('/ping')
def ping_ip():
    ip = request.args.get('ip', '')
    command = "ping -c 1 " + ip
    result = os.popen(command).read()
    return f"<pre>{result}</pre>"

@app.route('/backup')
def backup_files():
    folder = request.args.get('folder', '')
    os.system(f"tar -czvf backup.tar.gz /var/backups/{folder}")
    return "Done"

if __name__ == '__main__':
    app.run()
EOF

cat << 'EOF' > crates/runtime_core/tests/e2e_auto_heal.rs
//! End-to-end tests for the auto-heal pipeline.
//! All tests are #[ignore] — they need API keys + network.
//! Run with: cargo test -p nexus_runtime_core --test e2e_auto_heal -- --ignored --nocapture

use std::path::Path;
use std::process::Command;

fn run_ast_tool(args: &[&str]) -> bool {
    let status = Command::new("cargo")
        .args(["run", "--bin", "nexus_ast_tool", "--"])
        .args(args)
        .status()
        .expect("Failed to execute nexus_ast_tool");
    status.success()
}

#[test]
#[ignore = "needs GEMINI/NVIDIA keys + network"]
fn test_php_sqli_auto_heal() {
    let fixture_dir = Path::new("tests/fixtures/sqli_php");
    assert!(fixture_dir.exists(), "SQLi fixture directory missing");
    let target_file = fixture_dir.join("www/login.php");
    let original = std::fs::read_to_string(&target_file).unwrap();

    let success = run_ast_tool(&[
        "audit", "--path", fixture_dir.to_str().unwrap(),
        "--ai-triage", "--auto-heal", "--generate-report",
    ]);
    assert!(success, "nexus_ast_tool audit failed");

    let patched = std::fs::read_to_string(&target_file).unwrap_or_default();
    assert!(
        patched.contains("mysqli_prepare") || patched.contains("mysqli_stmt_bind_param"),
        "File was not patched with prepared statements. Content:\n{}", patched
    );
    std::fs::write(&target_file, original).ok();
}

#[test]
#[ignore = "needs GEMINI/NVIDIA keys + network"]
fn test_python_cmdi_auto_heal() {
    let fixture_dir = Path::new("tests/fixtures/cmdi_python");
    assert!(fixture_dir.exists(), "cmdi fixture directory missing");
    let target_file = fixture_dir.join("server.py");
    let original = std::fs::read_to_string(&target_file).unwrap();

    let success = run_ast_tool(&[
        "audit", "--path", fixture_dir.to_str().unwrap(),
        "--ai-triage", "--auto-heal", "--generate-report",
    ]);
    assert!(success, "nexus_ast_tool audit failed");

    let patched = std::fs::read_to_string(&target_file).unwrap_or_default();
    assert!(
        patched.contains("subprocess.run")
            && !patched.contains("os.popen")
            && !patched.contains("os.system"),
        "File was not patched with safe subprocess. Content:\n{}", patched
    );
    std::fs::write(&target_file, original).ok();
}
EOF

echo "✅ Phase 6: E2E fixtures + #[ignore] tests created"
echo ">> Phase 6 Complete."

echo -e "\n=================================================="
echo ">> ALL PHASES COMPLETE"
echo ">> Run 'cargo check --workspace' to verify."
echo ">> Run 'cargo test -p nexus_runtime_core --test e2e_auto_heal -- --ignored --nocapture' for E2E tests."
echo "=================================================="
