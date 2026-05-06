import os

# --- 1. Prune src/executor.rs ---
executor_path = "src/executor.rs"
if os.path.exists(executor_path):
    with open(executor_path, "r") as f:
        lines = f.readlines()
    
    new_lines = []
    for line in lines:
        # Check for the specific unused tracing imports
        if "use tracing::{info, warn};" in line:
            print(f">> [PRUNE] Removing unused tracing macros from {executor_path}")
            continue # Drop the whole line if nothing else is used
        new_lines.append(line)
        
    with open(executor_path, "w") as f:
        f.writelines(new_lines)

# --- 2. Prune src/fs_watcher.rs ---
watcher_path = "src/fs_watcher.rs"
if os.path.exists(watcher_path):
    with open(watcher_path, "r") as f:
        content = f.read()
    
    # Surgically remove Watcher while keeping RecursiveMode
    old_import = "notify::{RecursiveMode, Watcher}"
    new_import = "notify::RecursiveMode"
    
    if old_import in content:
        print(f">> [PRUNE] Removing unused Watcher trait from {watcher_path}")
        content = content.replace(old_import, new_import)
        with open(watcher_path, "w") as f:
            f.write(content)

