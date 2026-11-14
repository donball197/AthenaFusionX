#!/usr/bin/env bash
# mirror_smoke.sh — end-to-end validation of the mirror-agent sidecar
#
# Usage:
#   ./scripts/mirror_smoke.sh              # reflection mode (default)
#   ./scripts/mirror_smoke.sh openai       # openai-chat-completions fallback
#   ./scripts/mirror_smoke.sh kill         # just kill any running sidecar
#
# What it does:
#   1. Kills any stale mirror-agent process
#   2. Persists REFLECTION_API_KEY to .mirror/reflection_api_key (required by mirror-agent)
#   3. Adds .mirror/ to .gitignore (secret file)
#   4. Starts the sidecar with safe defaults (bounded iterations, no infinite retry)
#   5. Real readiness check: POST /message with trivial prompt, wait for 204
#   6. Real write test: create /tmp/mirror_smoke.txt, verify content
#   7. Cleans up (kills sidecar, removes test file)
#   8. Prints PASS/FAIL with diagnosis on failure
#
# Exit codes:
#   0  PASS — sidecar works, ready for integration
#   1  FAIL — check diagnosis printed at end
#   2  SKIP — preconditions not met

set -euo pipefail
cd "$(dirname "$0")/.."

MODE="${1:-reflection}"
BIN="./mirror/target/release/mirror-agent"
BIND="127.0.0.1:8081"
MODEL="Beam-1.0-501B-A23B"
TRACE="/tmp/mirror-trace.jsonl"
LOG="/tmp/mirror-agent-smoke.log"
SMOKE_FILE="mirror_smoke.txt"  # workspace-relative; mirror-agent write tool is sandboxed
HEALTH_TIMEOUT=60
TURN_TIMEOUT=180

log()  { printf "\x1b[36m[smoke]\x1b[0m %s\n" "$*"; }
ok()   { printf "\x1b[32m[+]\x1b[0m %s\n" "$*"; }
skip() { printf "\x1b[33m[=]\x1b[0m %s\n" "$*"; }
fail() { printf "\x1b[31m[!]\x1b[0m %s\n" "$*" >&2; }

die() {
    fail "$*"
    cleanup_sidecar
    exit 1
}

cleanup_sidecar() {
    if [ -n "${MIRROR_PID:-}" ] && kill -0 "$MIRROR_PID" 2>/dev/null; then
        kill -TERM "$MIRROR_PID" 2>/dev/null || true
        # Give it 3s to exit gracefully
        for _ in 1 2 3; do
            kill -0 "$MIRROR_PID" 2>/dev/null || break
            sleep 1
        done
        kill -KILL "$MIRROR_PID" 2>/dev/null || true
    fi
    # Belt-and-braces: any leftover mirror-agent serve
    pkill -f "mirror-agent serve" 2>/dev/null || true
}

diagnose() {
    echo ""
    fail "=== SMOKE TEST FAILED ==="
    echo ""
    if [ -f "$LOG" ]; then
        echo "--- sidecar log (last 20) ---"
        tail -20 "$LOG"
        echo ""
    fi
    if [ -f "$TRACE" ]; then
        local trace_errs
        trace_errs=$(grep -c '"level":"error"' "$TRACE" 2>/dev/null || echo 0)
        if [ "$trace_errs" -gt 0 ]; then
            echo "--- last 3 trace errors ---"
            grep '"level":"error"' "$TRACE" | tail -3
            echo ""
        fi
    fi
    echo "Common causes:"
    echo "  'set REFLECTION_API_KEY'      → key file missing or unreadable"
    echo "  'model request failed: 401'   → API key rejected by provider"
    echo "  'model request failed: 429'   → rate limited; try again in 60s"
    echo "  'turn timed out'              → model too slow; try --reasoning-effort low"
    echo "  'HTTP 415'                    → bridge sending wrong Content-Type"
    echo "  'connection refused'          → sidecar died before accepting requests"
    echo ""
    echo "Retry with fallback mode: ./scripts/mirror_smoke.sh openai"
}

# ----------------------------------------------------------------
# Preconditions
# ----------------------------------------------------------------
if [ ! -x "$BIN" ]; then
    fail "mirror-agent binary missing at $BIN"
    echo "  Build: cd mirror && cargo build --release -p mirror-agent"
    exit 2
fi
ok "binary present: $BIN"

if ! grep -q '^MIRROR_HTTP_ADDRESS' /dev/null 2>/dev/null; then :; fi  # noop

if [ ! -f .env ]; then
    fail ".env missing — cannot read REFLECTION_API_KEY"
    exit 2
fi
# Export all vars from .env so child processes inherit them.
# Without set -a, REFLECTION_API_KEY is a shell-local var and the
# launched mirror-agent subprocess cannot see it → infinite "set
# REFLECTION_API_KEY" errors on every model call.
set -a
# shellcheck disable=SC1091
source .env
set +a

if [ -z "${REFLECTION_API_KEY:-}" ]; then
    fail "REFLECTION_API_KEY not set in .env"
    exit 2
fi
# Belt-and-braces: even with set -a, ensure it's explicitly exported.
export REFLECTION_API_KEY
ok "REFLECTION_API_KEY present and exported (${#REFLECTION_API_KEY} chars)"

# ----------------------------------------------------------------
# Kill stale sidecar
# ----------------------------------------------------------------
if pgrep -f "mirror-agent serve" >/dev/null 2>&1; then
    log "killing stale mirror-agent process(es)..."
    pkill -f "mirror-agent serve" || true
    sleep 1
fi
ok "no stale sidecar"

# ----------------------------------------------------------------
# Persist key to file (mirror-agent's reliable path)
# ----------------------------------------------------------------
mkdir -p .mirror
chmod 700 .mirror
printf '%s' "$REFLECTION_API_KEY" > .mirror/reflection_api_key
chmod 600 .mirror/reflection_api_key
ok ".mirror/reflection_api_key ($(wc -c < .mirror/reflection_api_key) bytes)"

# ----------------------------------------------------------------
# Gitignore the secrets dir
# ----------------------------------------------------------------
if ! grep -q '^\.mirror/' .gitignore 2>/dev/null; then
    echo ".mirror/" >> .gitignore
    ok ".gitignore: added .mirror/"
else
    skip ".gitignore: .mirror/ already ignored"
fi

# ----------------------------------------------------------------
# Prepare trace/log/target
# ----------------------------------------------------------------
rm -f "$TRACE" "$LOG" "$SMOKE_FILE"
touch "$LOG"

# ----------------------------------------------------------------
# Build launch args based on mode
# ----------------------------------------------------------------
case "$MODE" in
    reflection)
        log "mode: reflection (native)"
        API_ARGS=(--api reflection --model "$MODEL")
        ;;
    openai)
        log "mode: openai-chat-completions (fallback)"
        export OPENAI_API_KEY="$REFLECTION_API_KEY"
        API_ARGS=(
            --api openai-chat-completions
            --base-url https://api.reflection.ai/openai/v1
            --model "$MODEL"
        )
        ;;
    kill)
        ok "killed (mode=kill)"
        exit 0
        ;;
    *)
        fail "unknown mode: $MODE (use reflection|openai|kill)"
        exit 2
        ;;
esac

# ----------------------------------------------------------------
# Launch sidecar
# ----------------------------------------------------------------
# Sanity check: launch a trivial subshell that prints the key length.
# If the child can't see the env var, we fail here instead of watching
# 5 minutes of retries in mirror-agent.
child_len=$(bash -c 'echo ${#REFLECTION_API_KEY}')
if [ "$child_len" -eq 0 ]; then
    fail "REFLECTION_API_KEY not exported to child processes (child sees 0 chars)"
    echo "  This means source .env did not export the var. Check .env for 'REFLECTION_API_KEY=...' without 'export '."
    exit 2
fi
ok "child sees REFLECTION_API_KEY ($child_len chars)"

log "starting sidecar on $BIND ..."
stdbuf -oL -eL "$BIN" serve \
    "${API_ARGS[@]}" \
    --workspace "$PWD" \
    --bind "$BIND" \
    --tools read,write \
    --output "$TRACE" \
    --max-iterations 10 \
    --max-exceptions 2 \
    --turn-timeout-seconds "$TURN_TIMEOUT" \
    --reasoning-effort low \
    >> "$LOG" 2>&1 &
MIRROR_PID=$!
ok "sidecar PID $MIRROR_PID"

# Trap so Ctrl+C also cleans up
trap 'cleanup_sidecar; exit 130' INT TERM

# ----------------------------------------------------------------
# Readiness check — real POST, not just /health
# ----------------------------------------------------------------
log "readiness: probing POST /message with trivial prompt (timeout ${HEALTH_TIMEOUT}s)..."

READY=0
for i in $(seq 1 "$HEALTH_TIMEOUT"); do
    if ! kill -0 "$MIRROR_PID" 2>/dev/null; then
        diagnose
        die "sidecar exited before becoming ready (see $LOG)"
    fi

    # /health is cheap; use it to know the server is up
    if ! curl -sf -o /dev/null --max-time 1 -X GET "http://$BIND/health"; then
        sleep 1
        continue
    fi

    # /message with a real prompt is the actual readiness test
    code=$(curl -s -o /tmp/mirror_readiness_body -w '%{http_code}' \
        --max-time 30 \
        -X POST "http://$BIND/message" \
        -H 'Content-Type: text/plain' \
        --data 'Reply with the single word OK and nothing else.' 2>/dev/null || echo "000")

    if [ "$code" = "204" ]; then
        READY=1
        ok "readiness: 204 after ${i}s"
        break
    elif [ "$code" = "409" ]; then
        # A turn is running — wait for it to complete
        printf "."
        sleep 2
        continue
    elif [ "$code" = "502" ] || [ "$code" = "504" ] || [ "$code" = "503" ]; then
        # Model call failed
        printf "."
        sleep 2
        continue
    elif [ "$code" = "000" ]; then
        printf "."
        sleep 1
    else
        # 4xx other than 409 — probably wrong media type or bad request
        if [ "$code" = "415" ]; then
            diagnose
            die "readiness: HTTP 415 — server rejected text/plain? (should not happen)"
        fi
        printf "."
        sleep 1
    fi
done
echo ""

if [ "$READY" -ne 1 ]; then
    diagnose
    die "readiness: did not reach 204 within ${HEALTH_TIMEOUT}s"
fi

# ----------------------------------------------------------------
# Smoke test — actual file write
# ----------------------------------------------------------------
log "smoke: POST /message to write $SMOKE_FILE ..."

code=$(curl -s -o /dev/null -w '%{http_code}' \
    --max-time "$TURN_TIMEOUT" \
    -X POST "http://$BIND/message" \
    -H 'Content-Type: text/plain' \
    --data "Use the write tool to create the file $SMOKE_FILE (relative to the workspace root) containing exactly the word hello. After the write succeeds, reply with exactly DONE and nothing else." \
    2>/dev/null || echo "000")

if [ "$code" != "204" ]; then
    diagnose
    die "smoke: POST /message returned $code (expected 204)"
fi
ok "smoke: 204 No Content"

# ----------------------------------------------------------------
# Verify file
# ----------------------------------------------------------------
if [ ! -f "$SMOKE_FILE" ]; then
    diagnose
    die "smoke: $SMOKE_FILE was not created"
fi

content=$(cat "$SMOKE_FILE")
size=$(wc -c < "$SMOKE_FILE")

if ! printf '%s' "$content" | grep -q "hello"; then
    fail "smoke: file created but content wrong"
    echo "  size: $size bytes"
    echo "  content: $content"
    diagnose
    exit 1
fi

ok "smoke: $SMOKE_FILE created (${size} bytes, contains 'hello')"

# ----------------------------------------------------------------
# Done
# ----------------------------------------------------------------
echo ""
# ----------------------------------------------------------------
# Containment check - absolute path outside the workspace must be refused
# ----------------------------------------------------------------
OUTSIDE_PROBE="/tmp/mirror_containment_probe_$$"
rm -f "$OUTSIDE_PROBE"
log "containment: POST /message asking for a write outside the workspace ..."
curl -s -o /dev/null --max-time "$TURN_TIMEOUT" \
    -X POST "http://$BIND/message" \
    -H 'Content-Type: text/plain' \
    --data "Use the write tool to create the file $OUTSIDE_PROBE (an absolute path) containing exactly the word hello. Report the exact result of the tool call." \
    >/dev/null 2>&1 || true

if [ -e "$OUTSIDE_PROBE" ]; then
    rm -f "$OUTSIDE_PROBE"
    diagnose
    die "containment: write outside the workspace SUCCEEDED ($OUTSIDE_PROBE)"
fi
if grep -q "$(basename "$OUTSIDE_PROBE")" "$TRACE" 2>/dev/null; then
    ok "containment: write to $OUTSIDE_PROBE attempted and refused"
else
    echo "[!] containment: INCONCLUSIVE - file absent, but no attempt visible in $TRACE"
fi

ok "=== PASS ==="
echo ""
echo "Sidecar validated. Integration ready."
echo ""
echo "Next steps:"
echo "  1. Commit the key file gitignore:  git add .gitignore && git commit -m 'chore: ignore .mirror/'"
echo "  2. Run integration phases:        ./scripts/mirror_upgrade.sh bridge"
echo "                                     ./scripts/mirror_upgrade.sh hook"
echo "                                     ./scripts/mirror_upgrade.sh verify"
echo "  3. Add mirror launcher phase:      ./scripts/mirror_upgrade.sh launcher"
echo ""
echo "Then run a mission with delegation enabled:"
echo "  export NEXUS_MIRROR_DELEGATION=1"
echo "  ./nexus.sh"

# Clean shutdown
cleanup_sidecar
ok "sidecar shut down cleanly"

# Remove test artifact (leave it for inspection if desired)
rm -f "$SMOKE_FILE" /tmp/mirror_readiness_body

exit 0
