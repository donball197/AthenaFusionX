import os

# 1. Fix src/cli.rs: Add missing Write trait
cli_path = "src/cli.rs"
if os.path.exists(cli_path):
    with open(cli_path, "r") as f:
        lines = f.readlines()
    
    # Insert the import at the top
    if "use std::io::Write;" not in "".join(lines):
        lines.insert(0, "use std::io::Write;\n")
        with open(cli_path, "w") as f:
            f.writelines(lines)
        print("\x1b[32m>> [FIXED] src/cli.rs: Added missing std::io::Write import.\x1b[0m")

# 2. Fix src/orchestrator.rs: Remove unused error import
orch_path = "src/orchestrator.rs"
if os.path.exists(orch_path):
    with open(orch_path, "r") as f:
        code = f.read()
    
    # Replace the unused import
    new_code = code.replace("use tracing::{info, error};", "use tracing::info;")
    if new_code != code:
        with open(orch_path, "w") as f:
            f.write(new_code)
        print("\x1b[32m>> [FIXED] src/orchestrator.rs: Removed unused tracing::error.\x1b[0m")
