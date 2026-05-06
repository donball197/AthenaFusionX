import os

main_path = "src/main.rs"
if os.path.exists(main_path):
    with open(main_path, "r") as f:
        code = f.read()
    
    # Correct the path from crate::fs_watcher to nexus_singularity::fs_watcher
    bad_call = "let _watcher = crate::fs_watcher::NexusWatcher::ignite(tx.clone())"
    good_call = "let _watcher = nexus_singularity::fs_watcher::NexusWatcher::ignite(tx.clone())"
    
    if bad_call in code:
        code = code.replace(bad_call, good_call)
        with open(main_path, "w") as f:
            f.write(code)
        print("\x1b[32m>> [REPAIR] src/main.rs pathing aligned with library DNA.\x1b[0m")
    else:
        print("\x1b[33m>> [SKIP] Path already correct or not found.\x1b[0m")
