import os

print(">> Applying Phase 2 (Item #8) Remediation Fix...")

path = "crates/runtime_core/src/epistemic_engine.rs"
if os.path.exists(path):
    with open(path, "r") as f:
        content = f.read()

    old_json = "let body: Value = res.json().await.unwrap_or_default();"
    new_json = """let body: Value = match res.json().await {
                    Ok(b) => b,
                    Err(e) => {
                        last_err = anyhow::anyhow!("JSON Parse Error: {}", e);
                        retries += 1;
                        if retries > 2 { break; }
                        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                        continue;
                    }
                };"""

    if old_json in content:
        content = content.replace(old_json, new_json)
        with open(path, "w") as f:
            f.write(content)
        print(f"[OK] Patched {path}")
    else:
        # If the file was truncated by the LLM, we need to restore the full file
        print("[!] File was truncated by LLM. Restoring via git...")
        os.system(f"git checkout -- {path}")
        with open(path, "r") as f:
            content = f.read()
        content = content.replace(old_json, new_json)
        with open(path, "w") as f:
            f.write(content)
        print(f"[OK] Restored and Patched {path}")
else:
    print(f"[ERROR] File not found: {path}")

print(">> Fix Applied.")
