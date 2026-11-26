#!/usr/bin/env bash
# mirror_upgrade.sh — phased, rollback-safe mirror-agent integration
#
# Usage:
#   ./scripts/mirror_upgrade.sh discover     # verify preconditions (no patches)
#   ./scripts/mirror_upgrade.sh bridge       # create mirror_bridge.rs
#   ./scripts/mirror_upgrade.sh hook         # patch truncation gate (env-gated)
#   ./scripts/mirror_upgrade.sh launcher     # add sidecar to nexus.sh
#   ./scripts/mirror_upgrade.sh verify       # check all changes are consistent
#   ./scripts/mirror_upgrade.sh rollback     # revert everything
#   ./scripts/mirror_upgrade.sh all          # discover + bridge + hook + verify
#
# Prerequisites:
#   - git working tree clean (run: git status)
#   - mirror-agent binary built (run: cargo build --release -p mirror-agent)
#   - `NEXUS_MIRROR_DELEGATION` env var unset until you're ready to test
#
# The integration is DORMANT until you export NEXUS_MIRROR_DELEGATION=1.
# Without that env var, behavior is identical to current main.

set -euo pipefail
cd "$(dirname "$0")/.."

MODE="${1:-discover}"

MIRROR_BIN="./mirror/target/release/mirror-agent"
BRIDGE_FILE="crates/runtime_core/src/mirror_bridge.rs"
STRATEGIZE="crates/runtime_core/src/cortical_loop/strategize.rs"
LIB="crates/runtime_core/src/lib.rs"
NEXUS_SH="nexus.sh"

BACKUP_DIR="/tmp/mirror_upgrade_backups"
mkdir -p "$BACKUP_DIR"

log()  { printf "\x1b[36m[mirror]\x1b[0m %s\n" "$*"; }
ok()   { printf "\x1b[32m[+]\x1b[0m %s\n" "$*"; }
skip() { printf "\x1b[33m[=]\x1b[0m %s\n" "$*"; }
fail() { printf "\x1b[31m[!]\x1b[0m %s\n" "$*" >&2; exit 1; }

backup() {
    local f="$1"
    local ts
    ts=$(date +%s)
    cp "$f" "$BACKUP_DIR/$(basename "$f").$ts"
}

# ----------------------------------------------------------------
# PHASE: discover
# ----------------------------------------------------------------
phase_discover() {
    log "=== Phase: discover ==="

    # Git clean?
    if ! git diff --quiet || ! git diff --cached --quiet; then
        fail "Working tree is dirty. Run 'git status' and commit/stash first."
    fi
    ok "git working tree clean"

    # Bridge file should NOT exist yet
    if [ -f "$BRIDGE_FILE" ]; then
        skip "mirror_bridge.rs already exists — bridge phase likely done"
    else
        ok "mirror_bridge.rs absent (will be created)"
    fi

    # Strategize hook: check current gate state
    if grep -q "NEXUS_MIRROR_DELEGATION" "$STRATEGIZE"; then
        skip "strategize.rs already hooks mirror"
    else
        if grep -q "still has an unclosed" "$STRATEGIZE"; then
            ok "strategize.rs truncation gate anchor present"
        else
            fail "truncation gate anchor missing — inspect strategize.rs manually"
        fi
    fi

    # lib.rs hook
    if grep -q "pub mod mirror_bridge;" "$LIB"; then
        skip "lib.rs already declares mirror_bridge"
    else
        ok "lib.rs needs mirror_bridge line"
    fi

    # nexus.sh sidecar
    if grep -q "mirror-agent serve" "$NEXUS_SH"; then
        skip "nexus.sh already launches mirror-agent"
    else
        ok "nexus.sh needs sidecar launch"
    fi

    # mirror-agent binary
    if [ ! -x "$MIRROR_BIN" ]; then
        fail "mirror-agent binary missing. Run: cargo build --release -p mirror-agent"
    fi
    ok "mirror-agent binary present"

    # CLI flags — need user to see these
    log "=== mirror-agent serve --help (top 20) ==="
    "$MIRROR_BIN" serve --help 2>&1 | head -20 || true

    log ""
    log "Precondition check complete. Next: ./scripts/mirror_upgrade.sh bridge"
}

# ----------------------------------------------------------------
# PHASE: bridge
# ----------------------------------------------------------------
phase_bridge() {
    log "=== Phase: bridge ==="

    if [ -f "$BRIDGE_FILE" ]; then
        skip "bridge file already exists — skipping"
        return 0
    fi

    cat > "$BRIDGE_FILE" <<'RUST_EOF'
//! Mirror-agent bridge: delegated synthesis for missions whose
//! cortical-loop write path fails (truncation, turns exhausted,
//! model confusion).
//!
//! Dormant unless `NEXUS_MIRROR_DELEGATION=1` is set. In that case,
//! a failing write is handed to a running mirror-agent sidecar at
//! `http://127.0.0.1:8081` with the accumulated evidence corpus as
//! context. On success, a synthetic `[WRITE_OK] path` marker is
//! pushed into `local_evidence` so downstream verification behaves
//! identically to a normal write.

use anyhow::Result;
use std::time::Duration;

pub struct MirrorBridge {
    endpoint: String,
    timeout: Duration,
}

impl MirrorBridge {
    pub fn new(endpoint: &str) -> Self {
        Self {
            endpoint: endpoint.trim_end_matches('/').to_string(),
            timeout: Duration::from_secs(900),
        }
    }

    /// Ask the sidecar to produce the deliverable at `target_path`.
    /// Returns `Ok(())` iff the file exists with non-trivial content
    /// after the call completes.
    pub async fn delegate_write(
        &self,
        target_path: &str,
        mission: &str,
        evidence_corpus: &str,
    ) -> Result<()> {
        let prompt = format!(
            "You are a subagent in the AthenaFusionX audit pipeline.\n\
             Your ONLY task: write the deliverable file.\n\n\
             MISSION:\n{mission}\n\n\
             TARGET FILE: {target_path}\n\
             (relative to workspace root)\n\n\
             EVIDENCE GATHERED BY THE PARENT AGENT:\n\
             ---\n{evidence_corpus}\n---\n\n\
             INSTRUCTIONS:\n\
             - Use the `write` tool to create {target_path}.\n\
             - Content must be complete prose. No code blocks.\n\
             - Reference functions by name only (backticks ok).\n\
             - Do NOT cite line numbers.\n\
             - Finish every sentence. Close every section.\n\
             - After the write succeeds, respond with exactly: DONE\n"
        );

        let client = reqwest::Client::builder().timeout(self.timeout).build()?;

        let resp = client
            .post(format!("{}/message", self.endpoint))
            .header(reqwest::header::CONTENT_TYPE, "text/plain")
            .body(prompt)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("mirror-agent HTTP: {e}"))?;

        let body = resp.text().await?;
        if !body.contains("DONE") {
            let tail: String = body.chars().rev().take(200).collect::<String>().chars().rev().collect();
            return Err(anyhow::anyhow!(
                "mirror-agent did not signal DONE; response tail: {tail}"
            ));
        }

        // Trust but verify: file must exist and be non-empty.
        let meta = tokio::fs::metadata(target_path).await
            .map_err(|e| anyhow::anyhow!("mirror reported DONE but {target_path} unreadable: {e}"))?;
        if meta.len() < 200 {
            return Err(anyhow::anyhow!(
                "mirror reported DONE but {target_path} is {} bytes (< 200)",
                meta.len()
            ));
        }
        Ok(())
    }
}
RUST_EOF

    ok "created $BRIDGE_FILE"

    # Register in lib.rs
    if ! grep -q "pub mod mirror_bridge;" "$LIB"; then
        backup "$LIB"
        # Insert after a stable anchor — 'pub mod model_config;' is in the dump
        python3 - <<'PY'
from pathlib import Path
p = Path("crates/runtime_core/src/lib.rs")
src = p.read_text()
anchor = "pub mod model_config;"
if anchor not in src:
    raise SystemExit("[!] lib.rs anchor 'pub mod model_config;' not found")
if "pub mod mirror_bridge;" in src:
    print("[=] mirror_bridge already declared")
else:
    src = src.replace(anchor, anchor + "\npub mod mirror_bridge;", 1)
    p.write_text(src)
    print("[+] lib.rs: pub mod mirror_bridge;")
PY
    else
        skip "lib.rs already declares mirror_bridge"
    fi

    log "Bridge phase complete. Next: ./scripts/mirror_upgrade.sh hook"
}

# ----------------------------------------------------------------
# PHASE: hook
# ----------------------------------------------------------------
phase_hook() {
    log "=== Phase: hook ==="

    if grep -q "NEXUS_MIRROR_DELEGATION" "$STRATEGIZE"; then
        skip "strategize.rs already hooks mirror"
        return 0
    fi

    backup "$STRATEGIZE"

    python3 - <<'PY'
from pathlib import Path
p = Path("crates/runtime_core/src/cortical_loop/strategize.rs")
src = p.read_text()

# Anchor: the entire "fail node cleanly" branch after 2 rejects.
# Insert mirror delegation BEFORE the return, gated on env var.
anchor = '''                } else if unclosed_fence {
                    // Never allow an unclosed fence through. A deliverable
                    // that ends mid-sentence is worse than no deliverable.
                    // Fail the node cleanly so the failure is diagnosable.
                    self.log(format!(
                        "\\x1b[1;31m>> [WRITE_TRUNCATION]\\x1b[0m Deliverable `{}` still has an unclosed ``` after {} attempts ({} bytes); failing node.",
                        path, prior, content.len()
                    ));
                    return Ok(CortexState::Fail {'''

if anchor not in src:
    if "NEXUS_MIRROR_DELEGATION" in src:
        print("[=] hook already present")
    else:
        raise SystemExit("[!] truncation-gate fail branch anchor not found")
else:
    replacement = '''                } else if unclosed_fence {
                    // Never allow an unclosed fence through. A deliverable
                    // that ends mid-sentence is worse than no deliverable.
                    self.log(format!(
                        "\\x1b[1;31m>> [WRITE_TRUNCATION]\\x1b[0m Deliverable `{}` still has an unclosed ``` after {} attempts ({} bytes); attempting mirror delegation.",
                        path, prior, content.len()
                    ));
                    // Mirror-agent fallback: if a sidecar is running and the
                    // env var is set, hand off synthesis. On success, synthesize
                    // a normal [WRITE_OK] marker so downstream verification is
                    // unchanged. On failure, fall through to Fail as before.
                    if std::env::var("NEXUS_MIRROR_DELEGATION").is_ok() {
                        let bridge = crate::mirror_bridge::MirrorBridge::new(
                            "http://127.0.0.1:8081",
                        );
                        match bridge
                            .delegate_write(path, mission, &session.local_evidence)
                            .await
                        {
                            Ok(()) => {
                                self.log(
                                    "\\x1b[1;36m>> [MIRROR] Delegated write succeeded.\\x1b[0m"
                                        .to_string(),
                                );
                                session.local_evidence.push_str(&format!(
                                    "\\n[WRITE_OK] {}\\n",
                                    path
                                ));
                                return Ok(CortexState::Strategize);
                            }
                            Err(e) => {
                                self.log(format!(
                                    "\\x1b[1;31m>> [MIRROR FAULT]\\x1b[0m {}",
                                    e
                                ));
                            }
                        }
                    }
                    return Ok(CortexState::Fail {'''
    src = src.replace(anchor, replacement, 1)
    p.write_text(src)
    print("[+] strategize.rs: mirror delegation hooked into truncation gate")
PY

    log "Hook phase complete. Compiling..."
    cargo check -p nexus_runtime_core 2>&1 | tail -3
}

# ----------------------------------------------------------------
# PHASE: launcher
# ----------------------------------------------------------------
phase_launcher() {
    log "=== Phase: launcher ==="

    if grep -q "mirror-agent serve" "$NEXUS_SH"; then
        skip "nexus.sh already launches mirror-agent"
        return 0
    fi

    if [ ! -x "$MIRROR_BIN" ]; then
        fail "mirror-agent binary missing. Build first: cargo build --release -p mirror-agent"
    fi

    # Show CLI flags so the user can correct the launch line if needed
    log "=== mirror-agent serve --help ==="
    "$MIRROR_BIN" serve --help 2>&1 | head -15 || true
    log ""
    log "If flags above differ from the defaults below, edit nexus.sh after this script."
    log ""

    backup "$NEXUS_SH"

    python3 - <<'PY'
from pathlib import Path
p = Path("nexus.sh")
src = p.read_text()

# Insert sidecar launch right before the CLI launch line
anchor = '''# --- 8. Launch Interactive CLI Client ---
echo -e "\\x1b[36m[3/3] Launching Interactive CLI Client...\\x1b[0m"
'''

if "mirror-agent serve" in src:
    print("[=] launcher already patched")
else:
    replacement = '''# --- 7b. Start mirror-agent sidecar (dormant unless NEXUS_MIRROR_DELEGATION=1) ---
MIRROR_PID=""
if [ -x "./mirror/target/release/mirror-agent" ] && [ -n "${NEXUS_MIRROR_DELEGATION:-}" ]; then
    echo -e "\\x1b[36m[2.5/3] Starting mirror-agent sidecar (delegation enabled)...\\x1b[0m"
    stdbuf -oL -eL ./mirror/target/release/mirror-agent serve \\
        --api reflection \\
        --model "${MIRROR_MODEL:-Beam-1.0-501B-A23B}" \\
        --workspace "$PWD" \\
        --bind 127.0.0.1:8081 \\
        --tools read,write \\
        --output /tmp/mirror-trace.jsonl \\
        --max-iterations 10 \\
        --max-exceptions 2 \\
        --turn-timeout-seconds 300 \\
        > /tmp/mirror-agent.log 2>&1 9>&- &
    MIRROR_PID=$!

    MIRROR_READY=0
    for i in $(seq 1 30); do
        if curl -s -o /dev/null --max-time 1 -X GET \\
            "http://127.0.0.1:8081/health"; then
            echo -e "\\x1b[32m[✓] Mirror-agent sidecar ready after ${i}s.\\x1b[0m"
            MIRROR_READY=1
            break
        fi
        if ! kill -0 "$MIRROR_PID" 2>/dev/null; then
            echo -e "\\x1b[31m>> [WARN] mirror-agent exited; delegation disabled.\\x1b[0m"
            MIRROR_PID=""
            break
        fi
        sleep 1
    done
    if [ "$MIRROR_READY" -eq 0 ] && [ -n "$MIRROR_PID" ]; then
        echo -e "\\x1b[31m>> [WARN] mirror-agent did not become ready in 30s; delegation disabled.\\x1b[0m"
        kill -TERM "$MIRROR_PID" 2>/dev/null || true
        MIRROR_PID=""
    fi
fi

# --- 8. Launch Interactive CLI Client ---
echo -e "\\x1b[36m[3/3] Launching Interactive CLI Client...\\x1b[0m"
'''

    src = src.replace(anchor, replacement, 1)

    # Extend cleanup to kill the sidecar
    cleanup_anchor = '''    pkill -f "${TARGET_DIR}/nexus-cli" 2>/dev/null || true
    if [ -n "${BUILD_PID:-}" ] && kill -0 "$BUILD_PID" 2>/dev/null; then'''

    if 'kill -0 "${MIRROR_PID:-0}"' not in src:
        cleanup_new = '''    pkill -f "${TARGET_DIR}/nexus-cli" 2>/dev/null || true
    if [ -n "${MIRROR_PID:-}" ] && kill -0 "$MIRROR_PID" 2>/dev/null; then
        kill -TERM "$MIRROR_PID" 2>/dev/null || true
    fi
    if [ -n "${BUILD_PID:-}" ] && kill -0 "$BUILD_PID" 2>/dev/null; then'''
        if cleanup_anchor in src:
            src = src.replace(cleanup_anchor, cleanup_new, 1)
            print("[+] nexus.sh: cleanup kills mirror sidecar")
        else:
            print("[!] cleanup anchor not found — manually add MIRROR_PID kill")

    p.write_text(src)
    print("[+] nexus.sh: sidecar launch inserted (dormant unless env var set)")
PY

    log "Launcher phase complete."
    log "Verify with: bash -n nexus.sh"
    bash -n "$NEXUS_SH" && ok "nexus.sh syntax OK"
}

# ----------------------------------------------------------------
# PHASE: verify
# ----------------------------------------------------------------
phase_verify() {
    log "=== Phase: verify ==="

    local ok_count=0
    local total=0

    check() {
        local desc="$1"
        local cond="$2"
        total=$((total + 1))
        if eval "$cond"; then
            ok "$desc"
            ok_count=$((ok_count + 1))
        else
            log "MISSING: $desc"
        fi
    }

    check "mirror_bridge.rs exists"          "[ -f $BRIDGE_FILE ]"
    check "lib.rs declares module"           "grep -q 'pub mod mirror_bridge;' $LIB"
    check "strategize.rs hooks mirror"       "grep -q 'NEXUS_MIRROR_DELEGATION' $STRATEGIZE"
    check "nexus.sh launches sidecar"        "grep -q 'mirror-agent serve' $NEXUS_SH"
    check "nexus.sh cleanup kills sidecar"   "grep -q 'MIRROR_PID' $NEXUS_SH"
    check "cargo check compiles"             "cargo check -p nexus_runtime_core 2>&1 | grep -q 'Finished'"

    log ""
    log "Result: $ok_count/$total checks passed"
    if [ "$ok_count" -eq "$total" ]; then
        ok "All checks passed. Integration ready."
        log ""
        log "To enable:   export NEXUS_MIRROR_DELEGATION=1"
        log "To disable:  unset NEXUS_MIRROR_DELEGATION"
        log "Run mission: ./nexus.sh"
    else
        fail "$total - $ok_count checks failed"
    fi
}

# ----------------------------------------------------------------
# PHASE: rollback
# ----------------------------------------------------------------
phase_rollback() {
    log "=== Phase: rollback ==="

    if git diff --quiet && git diff --cached --quiet; then
        if [ ! -f "$BRIDGE_FILE" ]; then
            skip "nothing to roll back"
            return 0
        fi
    fi

    log "Reverting modified files..."
    git checkout -- "$STRATEGIZE" "$LIB" "$NEXUS_SH" 2>/dev/null || true

    if [ -f "$BRIDGE_FILE" ]; then
        rm -f "$BRIDGE_FILE"
        ok "removed $BRIDGE_FILE"
    fi

    # Revert lib.rs module declaration (already handled by git checkout above,
    # but double-check)
    if grep -q "pub mod mirror_bridge;" "$LIB" 2>/dev/null; then
        log "warning: lib.rs still declares mirror_bridge after checkout"
    fi

    cargo check -p nexus_runtime_core 2>&1 | tail -2
    ok "Rollback complete"
}

# ----------------------------------------------------------------
# Dispatcher
# ----------------------------------------------------------------
case "$MODE" in
    discover)  phase_discover ;;
    bridge)    phase_bridge ;;
    hook)      phase_hook ;;
    launcher)  phase_launcher ;;
    verify)    phase_verify ;;
    rollback)  phase_rollback ;;
    all)       phase_discover; phase_bridge; phase_hook; phase_verify ;;
    *)
        echo "Usage: $0 {discover|bridge|hook|launcher|verify|rollback|all}"
        exit 1
        ;;
esac
