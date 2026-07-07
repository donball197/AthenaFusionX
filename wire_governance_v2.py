import sys

FILES = {}

FILES["crates/runtime_core/src/governance/protocol.rs"] = {
    "anchor": "use std::sync::Arc;",
    "replacement": "use std::sync::{Arc, LazyLock};",
}
FILES["crates/runtime_core/src/governance/protocol.rs"]["anchor_2"] = '''    pub fn is_empty(&self) -> bool {
        self.history.is_empty()
    }
}'''
FILES["crates/runtime_core/src/governance/protocol.rs"]["replacement_2"] = '''    pub fn is_empty(&self) -> bool {
        self.history.is_empty()
    }
}

// Process-wide persistent governance log. Callers should use this
// instead of constructing their own GovernanceLog, or records get
// discarded along with the throwaway instance.
pub static GOVERNANCE: LazyLock<Mutex<GovernanceLog>> =
    LazyLock::new(|| Mutex::new(GovernanceLog::init()));'''

FILES["crates/runtime_core/src/executor.rs"] = {
    "anchor": '''    pub async fn execute_single_action(
        &mut self,
        action: AgentAction,
        _memory: Arc<Mutex<MemoryPalace>>,
        _vmo_state: Arc<Mutex<VmoState>>,
        _dist: Arc<DistributedScheduler>,
    ) -> Result<String> {
        match action {''',
    "replacement": '''    pub async fn execute_single_action(
        &mut self,
        action: AgentAction,
        _memory: Arc<Mutex<MemoryPalace>>,
        _vmo_state: Arc<Mutex<VmoState>>,
        _dist: Arc<DistributedScheduler>,
    ) -> Result<String> {
        let ledger_action = action.clone();
        let result: Result<String> = async {
        match action {''',
}
FILES["crates/runtime_core/src/executor.rs"]["anchor_2"] = '''            AgentAction::Halt { reasoning } => Ok(format!("[HALT] {}", reasoning)),
        }
    }
}'''
FILES["crates/runtime_core/src/executor.rs"]["replacement_2"] = '''            AgentAction::Halt { reasoning } => Ok(format!("[HALT] {}", reasoning)),
        }
        }.await;

        if result.is_ok() {
            crate::governance::protocol::GOVERNANCE.lock().await.record(ledger_action, "robotic_executor");
        }

        result
    }
}'''


def check_anchor(path, key, anchor):
    try:
        with open(path, "r", encoding="utf-8") as f:
            content = f.read()
    except FileNotFoundError:
        print("FATAL: " + path + " not found.", file=sys.stderr)
        sys.exit(1)
    count = content.count(anchor)
    if count == 0:
        print("FATAL: anchor '" + key + "' not found in " + path + ".", file=sys.stderr)
        sys.exit(1)
    if count > 1:
        print("FATAL: anchor '" + key + "' matched " + str(count) + " times in " + path + ", expected 1.", file=sys.stderr)
        sys.exit(1)
    return content


def main():
    loaded = {}
    for path, spec in FILES.items():
        content = check_anchor(path, "primary", spec["anchor"])
        if "anchor_2" in spec:
            content = check_anchor(path, "anchor_2", spec["anchor_2"])
        loaded[path] = content

    for path, spec in FILES.items():
        content = loaded[path]
        content = content.replace(spec["anchor"], spec["replacement"])
        if "anchor_2" in spec:
            content = content.replace(spec["anchor_2"], spec["replacement_2"])
        with open(path, "w", encoding="utf-8") as f:
            f.write(content)
        print("OK: patched " + path)

    print()
    print("Next: cargo check -p nexus_runtime_core -j 2")


if __name__ == "__main__":
    main()
