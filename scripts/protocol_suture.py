import os

fpath = "src/agent_protocol.rs"
with open(fpath, "r") as f:
    code = f.read()

target = '#[serde(rename = "halt")] Halt { reasoning: String },'
replacement = """#[serde(rename = "halt")] Halt { reasoning: String },
    #[serde(rename = "diff_patch")] DiffPatch { path: String, search: String, replace: String, reasoning: String },"""

if target in code and "DiffPatch" not in code:
    code = code.replace(target, replacement)
    with open(fpath, "w") as f:
        f.write(code)
    print("\x1b[32m>> [SUTURE] AgentAction::DiffPatch injected into agent_protocol.rs\x1b[0m")
else:
    print("\x1b[33m>> [SKIP] Target not found or DiffPatch already exists.\x1b[0m")
