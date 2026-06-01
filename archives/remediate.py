import os, sys

def patch_file(path, search_pattern, replacement):
    with open(path, 'r') as f:
        content = f.read()
    
    if search_pattern in content:
        new_content = content.replace(search_pattern, replacement)
        with open(path, 'w') as f:
            f.write(new_content)
        print(f"[OK] Patched {path}")
    else:
        print(f"[WARN] Pattern not found in {path}")

# Fix 1: Add --whole-archive to build.rs to force linker to see Zig symbols
build_rs = "crates/nexus_substrate/build.rs"
patch_file(build_rs, 
    'println!("cargo:rustc-link-lib=static=muscle");',
    'println!("cargo:rustc-link-arg=-Wl,--whole-archive");\n    println!("cargo:rustc-link-lib=static=muscle");\n    println!("cargo:rustc-link-arg=-Wl,--no-whole-archive");')

# Fix 2: Remove unused imports in executor.rs
executor_rs = "crates/runtime_core/src/executor.rs"
patch_file(executor_rs, 'use crate::arbiter_agent::ArbiterAgent;', '')

# Fix 3: Fix unused variable in task_manager.rs
tm_rs = "crates/runtime_core/src/task_manager.rs"
patch_file(tm_rs, 'let execution_law =', 'let _execution_law =')
patch_file(tm_rs, 'execution_law.clone()', '_execution_law.clone()')

# Fix 4: Fix unused imports in router.rs
router_rs = "crates/runtime_core/src/router.rs"
patch_file(router_rs, 'use serde::{Deserialize, Serialize};', '// use serde::{Deserialize, Serialize};')

print(">> [SYS] Structural remediation complete.")
