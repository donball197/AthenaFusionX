import os

fpath = "src/cli.rs"
if os.path.exists(fpath):
    with open(fpath, "r") as f:
        code = f.read()

    # SUTURE: Inject a stream listener into the CLI loop
    if "let mut rx = state.tx.subscribe();" not in code:
        code = code.replace(
            "task_manager::send_task(",
            "let mut rx = state.tx.subscribe();\n        tokio::spawn(async move {\n            while let Ok(msg) = rx.recv().await {\n                if let Ok(val) = serde_json::from_str::<serde_json::Value>(&msg) {\n                    if val[\"type\"] == \"NEURAL_STREAM\" {\n                        if let Some(token) = val[\"payload\"].as_str() {\n                            print!(\"\\x1b[34m{}\\x1b[0m\", token);\n                            std::io::Write::flush(&mut std::io::stdout()).ok();\n                        }\n                    }\n                }\n            }\n        });\n        task_manager::send_task("
        )
        with open(fpath, "w") as f:
            f.write(code)
        print("\x1b[32m>> [SUTURE] Live Cognitive Streaming enabled in src/cli.rs\x1b[0m")
