import os

path = "src/governance/heartbeat.rs"
if os.path.exists(path):
    with open(path, "r") as f:
        code = f.read()
    
    old_block = """            let handles: Vec<u32> = if let Ok(reg) = kernel::HANDLE_REGISTRY.read() {
                reg.keys().cloned().collect()
            } else { Vec::new() };"""
            
    new_block = """            let handles: Vec<u32> = kernel::HANDLE_REGISTRY.read().keys().cloned().collect();"""
    
    if old_block in code:
        code = code.replace(old_block, new_block)
        with open(path, "w") as f:
            f.write(code)
        print("\x1b[32m>> [SUTURE] src/governance/heartbeat.rs aligned with parking_lot::RwLock.\x1b[0m")
    else:
        print("\x1b[33m>> [SKIP] Lock pattern not found or already fixed.\x1b[0m")
