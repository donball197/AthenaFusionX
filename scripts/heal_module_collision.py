import os

# 1. Deduplicate pub mod json_parser in src/lib.rs
fpath = "src/lib.rs"
if os.path.exists(fpath):
    with open(fpath, "r") as f:
        lines = f.readlines()
    
    new_lines = []
    seen_json_parser = False
    for line in lines:
        if "pub mod json_parser;" in line:
            if not seen_json_parser:
                seen_json_parser = True
                new_lines.append(line)
        else:
            new_lines.append(line)
            
    with open(fpath, "w") as f:
        f.writelines(new_lines)
    print(">> [SUTURE] Deduplicated 'json_parser' module in src/lib.rs")

# 2. Prune unused imports in src/task_manager.rs
fpath = "src/task_manager.rs"
if os.path.exists(fpath):
    with open(fpath, "r") as f:
        code = f.read()
    
    # Prune anyhow
    code = code.replace("use anyhow::{anyhow, Result};\n", "")
    # Prune MicroAgent and DiffEngine
    code = code.replace("neural_agent::MicroAgent,", "")
    code = code.replace("diff::DiffEngine,", "")
    # Clean up any trailing commas from the macro block
    code = code.replace("distributed_scheduler::DistributedScheduler, \n", "distributed_scheduler::DistributedScheduler\n")
    
    with open(fpath, "w") as f:
        f.write(code)
    print(">> [SUTURE] Pruned unused imports in src/task_manager.rs")
