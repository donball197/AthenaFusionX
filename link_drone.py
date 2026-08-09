import os

print(">> Linking data_scraper to Workspace...")

# 1. Remove any stray [workspace] tag from the child
path1 = "drones/data_scraper/Cargo.toml"
if os.path.exists(path1):
    with open(path1, "r") as f:
        content1 = f.read()
    if "[workspace]" in content1:
        content1 = content1.replace("[workspace]", "")
        with open(path1, "w") as f:
            f.write(content1)
        print(f"[OK] Cleaned {path1}")

# 2. Add it to the root Cargo.toml members array
path2 = "Cargo.toml"
if os.path.exists(path2):
    with open(path2, "r") as f:
        content2 = f.read()
    
    if '"drones/data_scraper"' not in content2:
        content2 = content2.replace('"apps/wasm_tester"', '"apps/wasm_tester",\n    "drones/data_scraper"')
        with open(path2, "w") as f:
            f.write(content2)
        print(f"[OK] Added data_scraper to root workspace.")
    else:
        print("[!] data_scraper is already in the root workspace.")

print(">> Link Complete.")
