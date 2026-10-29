import os
import json
import toml
from pathlib import Path

def analyze_dependencies():
    graph = {}
    crates_dir = Path('./crates')
    
    if crates_dir.exists():
        for crate in crates_dir.iterdir():
            cargo_toml = crate / 'Cargo.toml'
            if cargo_toml.exists():
                data = toml.load(cargo_toml)
                deps = list(data.get('dependencies', {}).keys())
                graph[crate.name] = deps
    
    with open('dependency_graph.json', 'w') as f:
        json.dump(graph, f, indent=4)
    print('Dependency graph generated in dependency_graph.json')

if __name__ == '__main__':
    analyze_dependencies()