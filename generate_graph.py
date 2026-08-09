import json, subprocess

print(">> Fetching workspace metadata...")
res = subprocess.run(["cargo", "metadata", "--format-version=1", "--no-deps"], capture_output=True, text=True)
data = json.loads(res.stdout)

local_crates = {p['name'] for p in data['packages']}
edges = set()

for p in data['packages']:
    src = p['name']
    for dep in p['dependencies']:
        dst = dep['name']
        if dst in local_crates and src != dst:
            edges.add((src, dst))

print("\n================ MERMAID.JS DEPENDENCY GRAPH ================\n")
print("graph TD;")
for s, d in sorted(edges):
    print(f"    {s.replace('-', '_')} --> {d.replace('-', '_')};")
print("\n=============================================================\n")
