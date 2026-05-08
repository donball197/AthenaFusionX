import os

tm_file = "crates/runtime_core/src/task_manager.rs"
if os.path.exists(tm_file):
    with open(tm_file, "r") as f:
        data = f.read()
    
    # We are updating the God-Mode override we placed earlier to include the Reality Anchor
    anchor_rule = " REALITY ANCHOR: If a referenced struct, function, or file cannot be found in the active `crates/` or `src/` directories, assume the reference document is STALE. DO NOT search `.lazarus_vault`, `.nexus_archive`, or `archives/` for active code. Abort the search and state the code no longer exists."
    
    if "authorized and REQUIRED" in data and "REALITY ANCHOR" not in data:
        data = data.replace('DO NOT use execute_command for scraping.", execution_law)', 'DO NOT use execute_command for scraping.' + anchor_rule + '", execution_law)')
        with open(tm_file, "w") as f:
            f.write(data)
        print("\x1b[32m    ✔ Reality Anchor injected. Cortex will no longer chase ghost files.\x1b[0m")
    else:
        print("\x1b[33m    [!] Anchor already present or God-Mode override not found.\x1b[0m")
