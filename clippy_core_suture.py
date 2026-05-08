import os

def patch_file(filepath, search, replace):
    if os.path.exists(filepath):
        with open(filepath, "r") as f:
            content = f.read()
        if search in content and replace not in content:
            with open(filepath, "w") as f:
                f.write(content.replace(search, replace))
            print(f"\x1b[32m    ✔ Patched {filepath}\x1b[0m")

# 1. Structural Default Implementations
defaults = {
    "crates/runtime_core/src/agents/scout_agent.rs": "ScoutAgent",
    "crates/runtime_core/src/cluster.rs": "ClusterMesh",
    "crates/runtime_core/src/orchestrator.rs": "Orchestrator",
    "crates/runtime_core/src/orchestrator_v2/task_graph.rs": "TaskGraph",
    "crates/runtime_core/src/orchestrator_v2/mod.rs": "OrchestratorV2",
    "crates/runtime_core/src/runtime_metrics.rs": "RuntimeMetrics"
}

for path, struct_name in defaults.items():
    if os.path.exists(path):
        with open(path, "r") as f:
            data = f.read()
        if f"impl Default for {struct_name}" not in data:
            data += f"\nimpl Default for {struct_name} {{\n    fn default() -> Self {{\n        Self::new()\n    }}\n}}\n"
            with open(path, "w") as f:
                f.write(data)
            print(f"\x1b[32m    ✔ Appended Default trait to {struct_name}\x1b[0m")

# 2. Preserve API Signatures (Silence too_many_arguments)
patch_file("crates/runtime_core/src/conductor.rs", "pub async fn run_turn(", "#[allow(clippy::too_many_arguments)]\n    pub async fn run_turn(")
patch_file("crates/runtime_core/src/task_manager.rs", "pub async fn send_task(", "#[allow(clippy::too_many_arguments)]\npub async fn send_task(")

# 3. Idiomatic Ranges and Iterators
patch_file("crates/runtime_core/src/agents/scribe_agent.rs", "b >= 32 && b <= 126", "(32..=126).contains(&b)")
patch_file("crates/runtime_core/src/executor.rs", "split('>').last().unwrap_or", "split('>').next_back().unwrap_or")
patch_file("crates/runtime_core/src/lazarus_vault.rs", "|s| s.path == path).last()", "|s| s.path == path).next_back()")

# 4. Math Semantics
patch_file("crates/runtime_core/src/governance/protocol.rs", "self.history.len() % 10 == 0", "self.history.len().is_multiple_of(10)")
