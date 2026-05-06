import os

fpath = "src/executor.rs"
with open(fpath, "r") as f:
    code = f.read()

target = """.map(|c| format!("[READ_OK] {} ({} bytes)", path, c.len()))"""

replacement = """.map(|c| {
                        let truncated: String = c.chars().take(8192).collect();
                        format!("[READ_OK] {} ({} bytes)\\n--- CONTENT ---\\n{}\\n--- END CONTENT ---", path, c.len(), truncated)
                    })"""

if target in code:
    code = code.replace(target, replacement)
    with open(fpath, "w") as f:
        f.write(code)
    print("\x1b[32m>> [SUTURE] RoboticExecutor restored sight to the Cortex.\x1b[0m")
else:
    print("\x1b[31m>> [ERROR] Could not find target in executor.rs\x1b[0m")
