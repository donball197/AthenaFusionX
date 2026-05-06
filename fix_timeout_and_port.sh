#!/usr/bin/env bash
# =============================================================================
# FIX 3: reqwest timeout + graceful port-in-use handling
#
# Two infrastructure fixes:
#   A) Add a 60s timeout to every reqwest::Client so uplink failures are
#      detected quickly instead of hanging until the OS kills the connection.
#
#   B) Replace the .unwrap() on TcpListener::bind in server.rs so a port
#      conflict gives a clear error message instead of a panic.
# =============================================================================
set -euo pipefail

REPO_ROOT="${1:-.}"
cd "$REPO_ROOT"

echo ">> [FIX 3A] Adding reqwest timeout to client builders..."

# Patch every `reqwest::Client::new()` that is used for API calls.
# We target the ones in neural_agent.rs and orchestrator.rs (the uplink clients).
# Safe to be conservative — only patch files that actually make Gemini calls.

for file in src/neural_agent.rs src/orchestrator.rs src/bin/cli.rs src/bin/upgrade.rs src/bin/quantum_ingestor.rs src/bin/global_audit.rs; do
    if [[ ! -f "$file" ]]; then
        continue
    fi

    # Only patch files that contain the Gemini API URL — these are the uplink clients
    if grep -q "generativelanguage.googleapis.com\|gemma-4\|gemini-" "$file" 2>/dev/null; then
        cp "$file" "${file}.bak_timeout"
        
        python3 - "$file" <<'PYEOF'
import sys
path = sys.argv[1]
with open(path) as f:
    src = f.read()

old = "let client = reqwest::Client::new();"
new = """\
let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .connect_timeout(std::time::Duration::from_secs(20))
        .build()
        .expect("Failed to build HTTP client");"""

if old in src:
    src = src.replace(old, new)
    with open(path, "w") as f:
        f.write(src)
    print(f"   Patched: {path}")
else:
    print(f"   Skipped (pattern not found): {path}")
PYEOF
    fi
done

echo ">> [FIX 3B] Fixing port-in-use panic in src/server.rs..."

if [[ ! -f "src/server.rs" ]]; then
    echo "!! [WARN] src/server.rs not found, skipping."
else
    cp src/server.rs src/server.rs.bak_port

    python3 - <<'PYEOF'
path = "src/server.rs"
with open(path) as f:
    src = f.read()

# Pattern: TcpListener::bind(...).unwrap() or similar
# Replace with a helpful error message pointing to the fix command
import re

# Match the bind+unwrap pattern (handles various whitespace/formatting)
old_patterns = [
    'TcpListener::bind(addr).unwrap()',
    'TcpListener::bind(&addr).unwrap()',
    'TcpListener::bind(listen_addr).unwrap()',
    'TcpListener::bind(socket_addr).unwrap()',
]

replacement = '''\
TcpListener::bind(addr).unwrap_or_else(|e| {{
        eprintln!(
            "!! [FATAL] Cannot bind to {{addr}}: {{e}}\\n\
             Is another nexus-dev process already running?\\n\
             Fix: fuser -k {{port}}/tcp",
            addr = addr, e = e, port = addr.port()
        );
        std::process::exit(1);
    }})'''

patched = False
for old in old_patterns:
    if old in src:
        # Use a simpler replacement that works regardless of addr variable name
        src = src.replace(old, old.replace(
            '.unwrap()',
            '''.unwrap_or_else(|e| {
            eprintln!("!! [FATAL] Port already in use: {}. Kill the old process with: fuser -k 3000/tcp", e);
            std::process::exit(1);
        })'''
        ), 1)
        patched = True
        print(f"   Patched bind().unwrap() in src/server.rs")
        break

if not patched:
    print("   [INFO] Could not find exact bind().unwrap() pattern.")
    print("   Search src/server.rs for 'TcpListener::bind' and replace .unwrap()")
    print("   with .unwrap_or_else(|e| { eprintln!(\"Port in use: {}\", e); std::process::exit(1); })")

with open(path, "w") as f:
    f.write(src)
PYEOF
fi

echo ">> [FIX 3] Done."
