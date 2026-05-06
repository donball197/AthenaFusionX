import os

path = "src/server.rs"
if os.path.exists(path):
    with open(path, "r") as f:
        code = f.read()

    old_route = '.route("/fs/list", get(|Query(q): Query<FsQuery>| async move {'
    new_route = """.route("/fs/read", get(|Query(q): Query<FsQuery>| async move {
            let path = q.path.unwrap_or_default();
            match tokio::fs::read_to_string(&path).await {
                Ok(content) => Json(serde_json::json!({ "content": content })),
                Err(e) => Json(serde_json::json!({ "error": e.to_string() }))
            }
        }))
        .route("/fs/write", post(|Json(payload): Json<serde_json::Value>| async move {
            let path = payload["path"].as_str().unwrap_or_default();
            let content = payload["content"].as_str().unwrap_or_default();
            match tokio::fs::write(path, content).await {
                Ok(_) => Json(serde_json::json!({ "status": "success" })),
                Err(e) => Json(serde_json::json!({ "error": e.to_string() }))
            }
        }))
        .route("/fs/list", get(|Query(q): Query<FsQuery>| async move {"""

    if old_route in code and "/fs/read" not in code:
        code = code.replace(old_route, new_route)
        with open(path, "w") as f:
            f.write(code)
        print("\x1b[32m>> [SUTURE] src/server.rs I/O endpoints injected.\x1b[0m")
    else:
        print("\x1b[33m>> [SKIP] Endpoints already exist or pattern not found.\x1b[0m")
