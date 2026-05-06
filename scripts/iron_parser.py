import os

# --- 1. SUTURE: Make 'reasoning' optional in agent_protocol.rs ---
fpath_proto = "src/agent_protocol.rs"
if os.path.exists(fpath_proto):
    with open(fpath_proto, "r") as f:
        code = f.read()
    
    # Inject #[serde(default)] into the enum to prevent missing field panics
    if "#[serde(default)]\npub enum AgentAction" not in code:
        code = code.replace(
            "pub enum AgentAction {",
            "// ⚡ SUTURE: Prevent panics when Gemma forgets the reasoning field\n#[serde(default)]\npub enum AgentAction {"
        )
        # Also need to implement Default for the enum if we use #[serde(default)]
        if "impl Default for AgentAction" not in code:
            code += """\n
impl Default for AgentAction {
    fn default() -> Self {
        AgentAction::Halt { reasoning: "FALLBACK: Parser triggered default enum state.".to_string() }
    }
}
"""
        with open(fpath_proto, "w") as f:
            f.write(code)
    print("\x1b[32m>> [SUTURE] agent_protocol.rs fortified against missing fields.\x1b[0m")

# --- 2. SUTURE: Aggressive String Replacement in json_parser.rs ---
fpath_parser = "src/json_parser.rs"
if os.path.exists(fpath_parser):
    with open(fpath_parser, "r") as f:
        code = f.read()

    if 'text.replace("\\"action\\"", "\\"type\\"")' not in code:
        injection = """    // ⚡ THE IRON PARSER: Intercept and remap Gemma's bad muscle memory
    text = text.replace("\\"action\\":", "\\"type\\":");
    text = text.replace("\\"file_path\\":", "\\"path\\":");
"""
        code = code.replace(
            'let mut text = response.trim().to_string();',
            'let mut text = response.trim().to_string();\n' + injection
        )
        with open(fpath_parser, "w") as f:
            f.write(code)
    print("\x1b[32m>> [SUTURE] json_parser.rs interceptor active.\x1b[0m")

