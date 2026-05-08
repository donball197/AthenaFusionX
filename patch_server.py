import os

path = "bins/ide_backend/src/server.rs"
if not os.path.exists(path):
    print("\x1b[31m[FAULT]\x1b[0m server.rs not found.")
    exit(1)

with open(path, "r") as f:
    data = f.read()

start_sig = "pub async fn start_server(state: Arc<AppState>) {"
start_idx = data.find(start_sig)

if start_idx != -1:
    depth = 0
    end_idx = -1
    for i in range(start_idx + len(start_sig) - 1, len(data)):
        if data[i] == '{': depth += 1
        elif data[i] == '}':
            depth -= 1
            if depth == 0:
                end_idx = i + 1
                break
    
    if end_idx != -1:
        suture = """pub async fn start_server(state: Arc<AppState>) {
    use axum::middleware::from_fn;
    use crate::security::sovereign_auth_guard;

    let authed = Router::new()
        .route("/api/task", post(unified_gateway))
        .route("/api/fs/write", post(fs_write_handler))
        .layer(from_fn(sovereign_auth_guard));

    let app = Router::new()
        .merge(authed)
        .route("/api/metrics", get(metrics_handler))
        .route("/api/fs/read", get(fs_read_handler))
        .route("/api/fs/list", get(fs_list_handler))
        .route("/api/vitals", get(vitals_handler::handle_vitals))
        .route("/api/cluster/nodes", get(cluster_api::list_nodes))
        .route("/ws/telemetry", get(ws_bridge::ws_telemetry_handler))
        .fallback(static_handler)
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    println!("\\x1b[32m>> [SERVER] Listening on http://0.0.0.0:3000\\x1b[0m");
    axum::serve(listener, app).await.unwrap();
}"""
        new_data = data[:start_idx] + suture + data[end_idx:]
        with open(path, "w") as f:
            f.write(new_data)
        print("\x1b[32m[OK]\x1b[0m Precision Suture applied to server.rs")
    else:
        print("\x1b[31m[FAULT]\x1b[0m Could not find closing brace.")
else:
    print("\x1b[31m[FAULT]\x1b[0m start_server signature not found.")
