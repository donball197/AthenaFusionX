import os

def patch_file(filepath, search, replace):
    if os.path.exists(filepath):
        with open(filepath, "r") as f:
            content = f.read()
        if search in content and replace not in content:
            with open(filepath, "w") as f:
                f.write(content.replace(search, replace))
            print(f"\x1b[32m    ✔ Patched {filepath}\x1b[0m")

# 1. Preserve Scheduler Signature
patch_file("bins/ide_backend/src/scheduler.rs", 
           "pub fn ignite(", 
           "#[allow(clippy::too_many_arguments)]\npub fn ignite(")

# 2 & 3. Server Polish (Path slicing & Print Literals)
patch_file("bins/ide_backend/src/server.rs", 
           "workspace_root: &PathBuf", 
           "workspace_root: &std::path::Path")
patch_file("bins/ide_backend/src/server.rs", 
           'println!("{}>> [SERVER] Listening on http://0.0.0.0:3000{}", "\\x1b[32m", "\\x1b[0m");', 
           'println!("\\x1b[32m>> [SERVER] Listening on http://0.0.0.0:3000\\x1b[0m");')

# 4. WebSocket Bridge Ownership
patch_file("bins/ide_backend/src/ws_bridge.rs", 
           "Message::Text(msg.into())", 
           "Message::Text(msg)")
