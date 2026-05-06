import os
import re

main_path = "src/main.rs"
if os.path.exists(main_path):
    with open(main_path, "r") as f:
        code = f.read()
    
    # 1. Update the function call to the new NexusWatcher::ignite pattern
    # We use let _watcher to ensure the debouncer isn't dropped immediately.
    old_pattern = r"nexus_singularity::fs_watcher::ignite_nervous_system\(tx\.clone\(\), .*?\);"
    new_call = "let _watcher = crate::fs_watcher::NexusWatcher::ignite(tx.clone()).expect(\">> [REPAIR] Failed to ignite FS_WATCHER\");"
    
    if re.search(old_pattern, code):
        code = re.sub(old_pattern, new_call, code)
        with open(main_path, "w") as f:
            f.write(code)
        print("\x1b[32m>> [SUTURE] src/main.rs re-aligned with NexusWatcher::ignite.\x1b[0m")
    else:
        print("\x1b[33m>> [SKIP] Ignition call not found or already updated.\x1b[0m")
