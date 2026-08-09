import os

print(">> Applying Phase 5 (Item #13) Remediation Fix...")

path = "crates/runtime_core/src/orchestrator_v2/planner.rs"
if os.path.exists(path):
    with open(path, "r") as f:
        content = f.read()

    # The exact block we want to replace
    old_code = """                let id = item.get("id").and_then(|v| v.as_str()).map(ToString::to_string)
                    .unwrap_or_else(|| format!("task_{}", idx + 1));"""

    # The new memory-safe implementation using tracing::warn!
    new_code = """                let id = item.get("id").and_then(|v| v.as_str()).map(ToString::to_string)
                    .unwrap_or_else(|| {
                        tracing::warn!("Missing task ID in graph, auto-generating fallback: task_{}", idx + 1);
                        format!("task_{}", idx + 1)
                    });"""

    if old_code in content:
        content = content.replace(old_code, new_code)
        with open(path, "w") as f:
            f.write(content)
        print(f"[OK] Patched {path} - Replaced silent unwrap fallback with proper tracing warning.")
    else:
        print(f"[!] Could not find target signature in {path}. It may already be patched.")
else:
    print(f"[ERROR] File not found: {path}")

