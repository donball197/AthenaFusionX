#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")"

W=bins/ide_backend/src/ws_bridge.rs
M=bins/ide_backend/src/main.rs
S=bins/ide_backend/src/server.rs
T=crates/runtime_core/src/task_manager.rs
B=crates/runtime_core/src/orchestrator_v2/planner.rs.bak.1793921733

for f in "$W" "$M" "$S" "$T"; do
  [ -f "$f" ] || { echo "missing $f"; exit 1; }
  [ -f "$f.prepatch" ] || cp "$f" "$f.prepatch"
done

# 1. ws_bridge.rs: token check + lag handling
cat > "$W" <<'RS'
use crate::server::AppState;
use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::broadcast::error::RecvError;

pub async fn ws_telemetry_handler(
    ws: WebSocketUpgrade,
    Query(q): Query<HashMap<String, String>>,
    State(state): State<Arc<AppState>>,
) -> Response {
    let authorized = q
        .get("token")
        .map(|t| {
            matches!(
                crate::auth::verify_token_detailed(t),
                Ok(crate::auth::TokenVerdict::Valid)
            )
        })
        .unwrap_or(false);

    if !authorized {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(mut socket: WebSocket, state: Arc<AppState>) {
    let mut rx = state.tx.subscribe();
    loop {
        match rx.recv().await {
            Ok(msg) => {
                if socket.send(Message::Text(msg)).await.is_err() {
                    break;
                }
            }
            Err(RecvError::Lagged(n)) => {
                tracing::warn!("[WS] telemetry client lagged, skipped {} messages", n);
                continue;
            }
            Err(RecvError::Closed) => break,
        }
    }
}
RS
echo "[ok] ws_bridge.rs"

# 2-4. exact-match edits (fail loudly if anchor missing or ambiguous)
python3 - "$M" "$S" "$T" <<'PY'
import sys
main_p, srv_p, task_p = sys.argv[1:4]

def patch(path, old, new, marker):
    s = open(path).read()
    if marker in s:
        print(f"[skip] {path}: already patched"); return
    if s.count(old) != 1:
        sys.exit(f"[FAIL] {path}: anchor found {s.count(old)} times, expected 1:\n{old}")
    open(path, "w").write(s.replace(old, new))
    print(f"[ok] {path}")

patch(main_p,
  "let _daemon_lock = acquire_daemon_lock()?;\n",
  "let _daemon_lock = acquire_daemon_lock()?;\n    let _auth_key = auth::load_key_at_startup()?; // fail fast if NEXUS_AUTH_KEY missing/short\n",
  "load_key_at_startup()?")

patch(srv_p,
  '    let listener = TcpListener::bind("0.0.0.0:3000").await.unwrap();\n    axum::serve(listener, app).await.unwrap();\n',
  '''    let addr = std::env::var("NEXUS_BIND").unwrap_or_else(|_| "127.0.0.1:3000".into());
    let listener = match TcpListener::bind(&addr).await {
        Ok(l) => l,
        Err(e) => {
            tracing::error!("[BOOT] bind {} failed: {}", addr, e);
            std::process::exit(1);
        }
    };
    tracing::info!("[BOOT] API listening on {}", addr);
    if let Err(e) = axum::serve(listener, app).await {
        tracing::error!("[SERVER] serve failed: {}", e);
    }
''',
  "NEXUS_BIND")

patch(task_p,
  '&out[..1500]',
  'out.chars().take(1500).collect::<String>()',
  "out.chars().take(1500)")
PY

# 5. stale backup
[ -f "$B" ] && rm -v "$B" || echo "[skip] backup already gone"

# 6. verify
touch "$M"
CARGO_TERM_COLOR=never cargo +nightly check --workspace > audit_out/cargo_after.txt 2>&1 \
  && echo "[ok] cargo check passed" || { echo "[FAIL] cargo check"; grep -E '^error' -A6 audit_out/cargo_after.txt | head -40; exit 1; }
grep -E '^warning: .*generated' audit_out/cargo_after.txt || true
echo "Rollback: for f in $W $M $S $T; do mv -f \$f.prepatch \$f; done"
