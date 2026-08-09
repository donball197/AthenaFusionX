import json
import subprocess
import sys

def main():
    result = subprocess.run(["cargo", "metadata", "--format-version", "1"], capture_output=True, text=True, check=True)
    data = json.loads(result.stdout)

    packages = {p["id"]: p["name"] for p in data["packages"]}
    edges = set()

    for node in data.get("resolve", {}).get("nodes", []):
        source = packages.get(node["id"])
        if not source:
            continue
        for dep_id in node.get("dependencies", []):
            target = packages.get(dep_id)
            if target:
                edges.add(f"    \"{source}\" --> \"{target}\"")

    mmd = ["graph TD"] + sorted(list(edges))
    with open("dependency_graph.mmd", "w") as f:
        f.write("\n".join(mmd) + "\n")
    print("Successfully generated dependency_graph.mmd")

if __name__ == "__main__":
    main()
