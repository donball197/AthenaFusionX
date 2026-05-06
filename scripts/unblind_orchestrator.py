import os

fpath = "src/orchestrator.rs"
with open(fpath, "r") as f:
    code = f.read()

# 1. Strip the unsupported Gemma 4 penalties
code = code.replace('                "frequencyPenalty": 0.4,\n', '')
code = code.replace('                "presencePenalty": 0.2,\n', '')

# 2. Expose the hidden HTTP errors
old_match = "                _ => tokio::time::sleep(Duration::from_millis(1000 * attempts)).await,"
new_match = """                Ok(res) => {
                    let status = res.status();
                    let text = res.text().await.unwrap_or_default();
                    tracing::error!("[ORCH] API REJECTED PAYLOAD: {} - {}", status, text);
                    tokio::time::sleep(Duration::from_millis(1000 * attempts)).await;
                }
                Err(e) => {
                    tracing::error!("[ORCH] NETWORK FAULT: {}", e);
                    tokio::time::sleep(Duration::from_millis(1000 * attempts)).await;
                }"""

if "API REJECTED PAYLOAD" not in code:
    code = code.replace(old_match, new_match)
    with open(fpath, "w") as f:
        f.write(code)
    print(">> [SUTURE] Orchestrator unblinded and penalties stripped.")
else:
    print(">> [WARN] Suture already applied.")
