import os

with open('src/server.rs', 'r') as f:
    code = f.read()

# Re-link the route
if 'route("/api/heal"' not in code:
    code = code.replace(
        '.route("/api/terminal", post(crate::server_terminal::terminal_command))', 
        '.route("/api/heal", post(handle_heal))\n        .route("/api/terminal", post(crate::server_terminal::terminal_command))'
    )

# Re-inject the logic
heal_logic = """
async fn handle_heal(State(state): State<Arc<AppState>>, Json(payload): Json<HealRequest>) -> impl IntoResponse {
    match crate::auto_healer::AutoHealer::attempt_auto_repair(&state, &payload.path, &payload.error_log).await {
        Ok((true, msg)) => (StatusCode::OK, Json(serde_json::json!({"status": "healed", "message": msg}))).into_response(),
        _ => (StatusCode::BAD_REQUEST, Json(serde_json::json!({"status": "failed"}))).into_response(),
    }
}

#[derive(serde::Deserialize)] pub struct HealRequest { pub path: String, pub error_log: String }
"""

if "HealRequest" not in code:
    code += heal_logic

with open('src/server.rs', 'w') as f:
    f.write(code)
print(">> [SUCCESS] Neural Healer re-linked.")
