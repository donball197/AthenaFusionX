#!/usr/bin/env bash
# =============================================================================
# NEXUS — server.rs auth guard patch (standalone, replaces Fix 8)
# Fixes: re.error: bad escape \x at position 975
# Root cause: \x1b in the replacement string was parsed as a regex escape.
# Fix: replacement is built as a list of lines joined in Python, keeping
#      the ANSI escape out of the re.sub() template entirely.
# Run from project root: bash nexus_patch_server.sh
# =============================================================================
set -euo pipefail

if [[ ! -f "bins/ide_backend/src/server.rs" ]]; then
    echo "[FAULT] bins/ide_backend/src/server.rs not found."
    echo "        Run from the nexus_singularity-dev project root."
    exit 1
fi

echo "[ FIX ] server.rs — injecting auth guard via Python (escape-safe)..."

python3 << 'PYEOF'
import re
import sys

path = "bins/ide_backend/src/server.rs"

try:
    with open(path, "r") as f:
        data = f.read()
except FileNotFoundError:
    print(f"    [FAULT] {path} not found.")
    sys.exit(1)

# Match start_server() from its signature to the first closing brace
# that sits alone on its own line (end of function body).
pattern = re.compile(
    r'pub async fn start_server\(state: Arc<AppState>\)\s*\{.*?\n\}',
    re.DOTALL
)

if not pattern.search(data):
    print("    [FAULT] Could not locate start_server() — verify the function signature.")
    sys.exit(1)

# Build the replacement as a list of plain strings joined with newlines.
# This keeps ALL special characters (backslashes, ANSI escapes, braces)
# out of the re.sub() template, which would misinterpret them.
lines = [
    "pub async fn start_server(state: Arc<AppState>) {",
    "    use axum::middleware::from_fn;",
    "    use crate::security::sovereign_auth_guard;",
    "",
    "    // State-mutating routes require a valid JWT bearer token.",
    "    let authed = Router::new()",
    '        .route("/api/task",     post(unified_gateway))',
    '        .route("/api/fs/write", post(fs_write_handler))',
    "        .layer(from_fn(sovereign_auth_guard));",
    "",
    "    let app = Router::new()",
    "        .merge(authed)",
    '        .route("/api/metrics",       get(metrics_handler))',
    '        .route("/api/fs/read",       get(fs_read_handler))',
    '        .route("/api/fs/list",       get(fs_list_handler))',
    '        .route("/api/vitals",        get(vitals_handler::handle_vitals))',
    '        .route("/api/cluster/nodes", get(cluster_api::list_nodes))',
    '        .route("/ws/telemetry",      get(ws_bridge::ws_telemetry_handler))',
    "        .fallback(static_handler)",
    "        .with_state(state);",
    "",
    '    let listener = TcpListener::bind("0.0.0.0:3000").await.unwrap();',
    # ANSI green escape stored as a plain Python string — never touches re.sub template.
    '    println!("{}>> [SERVER] Listening on http://0.0.0.0:3000{}", "\\x1b[32m", "\\x1b[0m");',
    "    axum::serve(listener, app).await.unwrap();",
    "}",
]

replacement = "\n".join(lines)

# re.sub with a callable avoids ALL template escape processing.
# The lambda returns the replacement string literally — no \1, \x, \n
# substitution happens inside re.sub itself.
new_data = pattern.sub(lambda _: replacement, data, count=1)

with open(path, "w") as f:
    f.write(new_data)

print("    ✔ server.rs patched — /api/task + /api/fs/write now behind sovereign_auth_guard.")
print("    Auth bypass: /api/auth/login (handled inside sovereign_auth_guard itself).")
PYEOF
