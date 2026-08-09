import os

print(">> Linking wasm_tester to Workspace...")

# 1. Remove the stray [workspace] tag from the child
path1 = "apps/wasm_tester/Cargo.toml"
if os.path.exists(path1):
    with open(path1, "r") as f:
        content1 = f.read()
    content1 = content1.replace("[workspace]", "")
    with open(path1, "w") as f:
        f.write(content1)
    print(f"[OK] Cleaned {path1}")

# 2. Add it to the root Cargo.toml members array
path2 = "Cargo.toml"
if os.path.exists(path2):
    with open(path2, "r") as f:
        content2 = f.read()
    
    if '"apps/wasm_tester"' not in content2:
        content2 = content2.replace('"apps/math_utils"', '"apps/math_utils",\n    "apps/wasm_tester"')
        with open(path2, "w") as f:
            f.write(content2)
        print(f"[OK] Added wasm_tester to root workspace.")
    else:
        print("[!] wasm_tester is already in the root workspace.")

print(">> Link Complete.")
