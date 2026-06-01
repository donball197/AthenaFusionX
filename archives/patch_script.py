import sys, re, os

def batch_correct(file_path):
    with open(file_path, 'r') as f:
        content = f.read()
    original = content

    # Fix 1: Remove unresolved dom_bridge import
    content = re.sub(r'use crate::agents::dom_bridge::DomBridge;\n', '', content)

    # Fix 2: Correct SubstrateContext import location
    content = re.sub(
        r'use crate::agents::\{([^\}]*)agentic_runtime::SubstrateContext([^\}]*)\};',
        r'use crate::agents::agentic_runtime::SubstrateContext;\nuse crate::agents::{\1\2};',
        content, flags=re.DOTALL
    )

    # Fix 3: Remove unresolved resource_guard imports
    content = re.sub(r'use crate::agents::resource_guard::[^;]+;\n', '', content)

    # Fix 4: Fix RoboticExecutor import mismatch
    content = content.replace('executor::RoboticExecutor', 'executor::AgenticExecutor')

    if content != original:
        with open(file_path + '.fixed', 'w') as f:
            f.write(content)
        # Generate patch file
        os.system(f"diff -u {file_path} {file_path}.fixed > {file_path}.patch")
        # Apply fix
        os.system(f"mv {file_path}.fixed {file_path}")
        print(f"[OK] Patched {file_path}")

print("Running multiline patcher...")
for root, _, files in os.walk('crates/runtime_core/src'):
    for file in files:
        if file.endswith('.rs'):
            batch_correct(os.path.join(root, file))

