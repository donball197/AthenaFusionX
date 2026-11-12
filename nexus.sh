#!/usr/bin/env bash
set -e

# --- 1. Target & Profile Configuration ---
BUILD_PROFILE="debug"
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

cargo ${CARGO_TOOLCHAIN} build ${RELEASE_FLAG} --bin nexus-ide-backend --bin nexus-cli > /tmp/nexus_build.log 2>&1 &
BUILD_PID=$!

spinner $BUILD_PID "Verifying & building backend and CLI"

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
if [ -f daemon.log ]; then mv daemon.log "daemon.$(date +%s).log"; fi
"${TARGET_DIR}/nexus-ide-backend" 9>&- > daemon.log 2>&1 &
DAEMON_PID=$!

cleanup() {
    tput cnorm 2>/dev/null || true
    echo -e "\n\x1b[31m>> Shutting down Background Daemon (PID: $DAEMON_PID)...\x1b[0m"
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

# --- 8. Launch Interactive CLI Client ---
echo -e "\x1b[36m[3/3] Launching Interactive CLI Client...\x1b[0m"
"${TARGET_DIR}/nexus-cli" 9>&-
