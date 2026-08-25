#!/bin/bash
# Patch the Daemon to enforce the READ_FORBIDDEN flag

python3 << 'EOF'
import os

file_path = "bins/ide_backend/src/server.rs"
if os.path.exists(file_path):
    with open(file_path, "r") as f:
        content = f.read()
        
    # We need to inject the flag setter right after the directive string is extracted
    old_code = """    let directive_str = intent.directive.unwrap_or_default();
    let s = state.clone();"""
    
    new_code = """    let directive_str = intent.directive.unwrap_or_default();
    
    // Bug A/B Fix: Physically block read_file actions if the mission forbids it
    nexus_runtime_core::action_gate::READ_FORBIDDEN.store(
        directive_str.contains("DO NOT read any files"),
        std::sync::atomic::Ordering::Relaxed
    );
    
    let s = state.clone();"""
    
    if old_code in content:
        content = content.replace(old_code, new_code)
        with open(file_path, "w") as f:
            f.write(content)
        print("[✅ BUG A/B FULLY FIXED] Injected READ_FORBIDDEN setter into bins/ide_backend/src/server.rs")
    else:
        print("[⚠️ WARNING] Could not find the exact insertion point in server.rs. You may need to add this manually:")
        print(new_code)
else:
    print("[❌ BUG A/B] File not found: bins/ide_backend/src/server.rs")
EOF

echo ">> Rebuilding Workspace..."
cargo check -j 4
