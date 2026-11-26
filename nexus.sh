#!/usr/bin/env bash
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

# --- 1. Target & Profile Configuration ---
BUILD_PROFILE="debug"
BUILD_PID=""
DAEMON_PID=""
_CLEANUP_DONE=0
CARGO_TOOLCHAIN="+nightly"
RELEASE_FLAG=""
TARGET_DIR="./target/debug"

if [[ "$1" == "--release" || "$1" == "-r" ]]; then
    BUILD_PROFILE="release"
    CARGO_TOOLCHAIN="+stable"
    RELEASE_FLAG="--release"
    TARGET_DIR="./target/release"
fi

echo "=================================================="
echo ">> Booting AthenaFusionX Distributed Architecture"
echo ">> Toolchain: ${CARGO_TOOLCHAIN} | Profile: ${BUILD_PROFILE^^}"
echo "=================================================="

# --- 2. Environment & Linker Settings ---
if [ -f .env ]; then
    set -a
    source .env
    set +a
fi

export OPENSSL_NO_VENDOR=1
export CARGO_UNSTABLE_CARGO_LINTS=true

# Defensive: clear any RUSTFLAGS inherited from the calling shell. A
# previous debugging session may have manually `export`ed a fragile
# linker workaround (e.g. forcing --whole-archive on
# libc_nonshared.a to fix an earlier symbol-resolution issue), and
# `export` persists for the life of the terminal regardless of what
# any script does afterward. That stale flag surviving into a later,
# unrelated build produced a genuine duplicate-symbol linker error
# (__pthread_atfork defined twice) since the workaround is no longer
# needed. This script's own build should never depend on flags left
# behind by an interactive shell.
unset RUSTFLAGS

# --- 3. Exclusive Launcher Ownership ---
# Prevent concurrent launcher invocations.
# The lock file is intentionally persistent; never delete it.
mkdir -p target
LOCK_FILE="target/.nexus-launcher.lock"
exec 9>"$LOCK_FILE"

if ! flock -n 9; then
    echo "[FATAL] Another AthenaFusionX launcher owns the launcher lock."
    exit 1
fi

# --- 4. Smooth Spinner Function ---
spinner() {
    local pid=$1
    local msg="$2"
    local spin_chars=('⠋' '⠙' '⠹' '⠸' '⠼' '⠴' '⠦' '⠧' '⠇' '⠏')
    local i=0

    tput civis 2>/dev/null || true

    while kill -0 "$pid" 2>/dev/null; do
        printf "\r\x1b[36m[%s]\x1b[0m %s... " "${spin_chars[$i]}" "$msg"
        i=$(( (i + 1) % ${#spin_chars[@]} ))
        sleep 0.08
    done

    tput cnorm 2>/dev/null || true
    printf "\r\x1b[K"
}

# --- 5. Quiet Compilation with Live Animation ---
echo -e "\x1b[36m[1/3] Compiling binaries (${BUILD_PROFILE})...\x1b[0m"

cargo ${CARGO_TOOLCHAIN} build ${RELEASE_FLAG} --bin nexus-ide-backend --bin nexus-cli > /tmp/nexus_build.log 2>&1 9>&- &
BUILD_PID=$!

spinner $BUILD_PID "Verifying & building backend and CLI"

BUILD_STATUS=0
wait $BUILD_PID || BUILD_STATUS=$?
if [[ ${BUILD_STATUS:-0} -ne 0 ]]; then
    echo -e "\x1b[31m[✗] Build FAILED. Compiler error log:\x1b[0m"
    cat /tmp/nexus_build.log
    exit 1
fi

echo -e "\x1b[32m[✓] Binaries compiled successfully (${BUILD_PROFILE}).\x1b[0m"

if [[ ! -f "${TARGET_DIR}/nexus-ide-backend" || ! -f "${TARGET_DIR}/nexus-cli" ]]; then
    echo -e "\x1b[31m>> [FATAL] Expected binaries not found in ${TARGET_DIR}.\x1b[0m"
    exit 1
fi

# --- 6. Start Background Daemon ---
echo -e "\x1b[36m[2/3] Initializing Background Daemon (Port 3000)...\x1b[0m"
if curl -s -o /dev/null --max-time 1 "http://127.0.0.1:3000/api/vitals"; then
    echo -e "\x1b[31m>> [FATAL] Port 3000 is already responding before launch (stale daemon running). Stop it first.\x1b[0m"
    exit 1
fi
ls -lh --time-style=full-iso "${TARGET_DIR}/nexus-ide-backend" "${TARGET_DIR}/nexus-cli"
sha256sum "${TARGET_DIR}/nexus-ide-backend" "${TARGET_DIR}/nexus-cli"
if [ -f daemon.log ]; then mv daemon.log "daemon.$(date +%s).log"; fi
stdbuf -oL -eL "${TARGET_DIR}/nexus-ide-backend" 9>&- > daemon.log 2>&1 &
DAEMON_PID=$!

cleanup() {
    [ "${_CLEANUP_DONE:-}" = "1" ] && return
    _CLEANUP_DONE=1
    tput cnorm 2>/dev/null || true
    echo -e "\n\x1b[31m>> Shutting down Background Daemon (PID: $DAEMON_PID)...\x1b[0m"
    pkill -f "${TARGET_DIR}/nexus-cli" 2>/dev/null || true
    if [ -n "${MIRROR_PID:-}" ] && kill -0 "$MIRROR_PID" 2>/dev/null; then
        kill -TERM "$MIRROR_PID" 2>/dev/null || true
    fi
    if [ -n "${BUILD_PID:-}" ] && kill -0 "$BUILD_PID" 2>/dev/null; then
        kill -TERM "$BUILD_PID" 2>/dev/null || true
    fi
    if kill -0 "$DAEMON_PID" 2>/dev/null; then
        kill -TERM "$DAEMON_PID" 2>/dev/null || true
        wait "$DAEMON_PID" 2>/dev/null || true
    fi
}
trap cleanup SIGINT SIGTERM EXIT

# --- 7. Daemon Health Check Animation ---
printf "\x1b[33m[~] Connecting to Daemon vitals\x1b[0m"
READY=0
for i in $(seq 1 300); do
    if ! kill -0 "$DAEMON_PID" 2>/dev/null; then
        echo -e "\n\x1b[31m>> [FATAL] Daemon (PID $DAEMON_PID) exited before becoming ready. daemon.log:\x1b[0m"
        tail -n 30 daemon.log
        exit 1
    fi
    if curl -s -o /dev/null --max-time 1 "http://127.0.0.1:3000/api/vitals"; then
        printf "\r\x1b[K\x1b[32m[✓] Daemon online and verified ready after ${i}s.\x1b[0m\n"
        READY=1
        break
    fi
    printf "."
    sleep 1
done

if [ "$READY" -eq 0 ]; then
    echo -e "\n\x1b[31m>> [FATAL] Daemon did not respond within 300s. Checking daemon.log:\x1b[0m"
    tail -n 25 daemon.log
    exit 1
fi

# --- 7b. Start mirror-agent sidecar (dormant unless NEXUS_MIRROR_DELEGATION=1) ---
MIRROR_PID=""
if [ -x "./mirror/target/release/mirror-agent" ] && [ -n "${NEXUS_MIRROR_DELEGATION:-}" ]; then
    echo -e "\x1b[36m[2.5/3] Starting mirror-agent sidecar (delegation enabled)...\x1b[0m"
    stdbuf -oL -eL ./mirror/target/release/mirror-agent serve \
        --api reflection \
        --model "${MIRROR_MODEL:-Beam-1.0-501B-A23B}" \
        --workspace "$PWD" \
        --bind 127.0.0.1:8081 \
        --tools read,write \
        --output /tmp/mirror-trace.jsonl \
        --max-iterations 10 \
        --max-exceptions 2 \
        --turn-timeout-seconds 300 \
        > /tmp/mirror-agent.log 2>&1 9>&- &
    MIRROR_PID=$!

    MIRROR_READY=0
    for i in $(seq 1 30); do
        if curl -s -o /dev/null --max-time 1 -X GET \
            "http://127.0.0.1:8081/health"; then
            echo -e "\x1b[32m[✓] Mirror-agent sidecar ready after ${i}s.\x1b[0m"
            MIRROR_READY=1
            break
        fi
        if ! kill -0 "$MIRROR_PID" 2>/dev/null; then
            echo -e "\x1b[31m>> [WARN] mirror-agent exited; delegation disabled.\x1b[0m"
            MIRROR_PID=""
            break
        fi
        sleep 1
    done
    if [ "$MIRROR_READY" -eq 0 ] && [ -n "$MIRROR_PID" ]; then
        echo -e "\x1b[31m>> [WARN] mirror-agent did not become ready in 30s; delegation disabled.\x1b[0m"
        kill -TERM "$MIRROR_PID" 2>/dev/null || true
        MIRROR_PID=""
    fi
fi

# --- 8. Launch Interactive CLI Client ---
echo -e "\x1b[36m[3/3] Launching Interactive CLI Client...\x1b[0m"
"${TARGET_DIR}/nexus-cli" 9>&- | tee -a mission.log
