import os, re

def append_if_missing(filepath, content):
    if not os.path.exists(filepath): return
    with open(filepath, "r") as f: data = f.read()
    if content.strip() not in data:
        with open(filepath, "a") as f: f.write(content)
        print(f"    ✔ Wired into {filepath}")

# 1. Wire runtime_core
append_if_missing("crates/runtime_core/src/lib.rs", """
pub mod event_bus;
pub mod memory_actor;
pub mod graph_validator;
pub mod wasm_executor;
pub mod runtime_metrics;
""")

# 2. Wire nexus_substrate
append_if_missing("crates/nexus_substrate/src/lib.rs", "\npub mod sqlite_store;\n")
append_if_missing("crates/nexus_substrate/src/kernel/mod.rs", "\npub mod safe_kernel;\n")

# 3. Clean ide_backend/src/main.rs phantom limb
ide_main = "bins/ide_backend/src/main.rs"
if os.path.exists(ide_main):
    with open(ide_main, "r") as f: data = f.read()
    
    # Excise governance module declaration
    data = re.sub(r"pub\s+mod\s+governance;\n?", "", data)
    # Excise governance initialization
    data = re.sub(r"let\s+_governance_log\s*=\s*Arc::new\(parking_lot::Mutex::new\(governance::protocol::GovernanceLog::init\(\)\)\);\n?", "", data)
    
    with open(ide_main, "w") as f: f.write(data)
    print(f"    ✔ Amputated dead governance initialization from {ide_main}")

print("\x1b[32m>> V4 MODULE TREE ALIGNED.\x1b[0m")
