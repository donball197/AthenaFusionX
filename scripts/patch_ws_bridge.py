import os

fpath = "src/ws_bridge.rs"
with open(fpath, "r") as f: code = f.read()

old_block = """            client_msg = socket.recv() => {
                if client_msg.is_none() {
                
                    break; // Browser closed the tab/socket
                }
            }"""

new_block = """            client_msg = socket.recv() => {
                if let Some(Ok(axum::extract::ws::Message::Text(txt))) = client_msg {
                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&txt) {
                        if json["type"] == "GHOST_DOM_STATE" {
                            tracing::info!("[GHOST_DOM] Frontend State: {}", json["payload"]);
                        }
                    }
                } else if client_msg.is_none() {
                    break;
                }
            }"""

if "GHOST_DOM_STATE" not in code:
    code = code.replace(old_block, new_block)
    with open(fpath, "w") as f: f.write(code)
    print(">> [SUTURE] WebSocket reflection mirror active in ws_bridge.rs")
