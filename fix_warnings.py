import os

# 1. token_ledger.rs - prefix unused variable
p1 = "crates/nexus_substrate/src/token_ledger.rs"
if os.path.exists(p1):
    with open(p1, "r") as f: d = f.read()
    d = d.replace("for attempt in 0..MAX_ATTEMPTS", "for _attempt in 0..MAX_ATTEMPTS")
    with open(p1, "w") as f: f.write(d)
    print("    ✔ token_ledger.rs silenced.")

# 2. vector_bridge.rs - consume unused Result
p2 = "crates/nexus_substrate/src/vector_bridge.rs"
if os.path.exists(p2):
    with open(p2, "r") as f: d = f.read()
    d = d.replace("crate::token_ledger::wait_for_quota", "let _ = crate::token_ledger::wait_for_quota")
    with open(p2, "w") as f: f.write(d)
    print("    ✔ vector_bridge.rs silenced.")

# 3. task_manager.rs - remove unused tracking variables
p3 = "crates/runtime_core/src/task_manager.rs"
if os.path.exists(p3):
    with open(p3, "r") as f: d = f.read()
    d = d.replace("let mut error_count = 0usize;", "")
    d = d.replace("error_count += 1;", "")
    d = d.replace("Err(e)   => {  break 'mission; }", "Err(_e)   => { break 'mission; }")
    with open(p3, "w") as f: f.write(d)
    print("    ✔ task_manager.rs tracking variables pruned.")
