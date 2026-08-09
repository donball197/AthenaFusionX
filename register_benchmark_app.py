import sys

path = "Cargo.toml"
with open(path, "r") as f:
    content = f.read()

if '"apps/benchmark_runner"' in content:
    print("[INFO] apps/benchmark_runner is already listed in Cargo.toml.")
    sys.exit(0)

# Inject apps/benchmark_runner into the workspace members array
if "members = [" in content:
    new_content = content.replace('members = [', 'members = [\n    "apps/benchmark_runner",')
    with open(path, "w") as f:
        f.write(new_content)
    print("[OK] Added apps/benchmark_runner to workspace members in Cargo.toml.")
else:
    print("[FAIL] Could not locate 'members = [' array in Cargo.toml.")
    sys.exit(1)
