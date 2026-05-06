import os

# 1. Fix src/orchestrator.rs (Lifetime Error)
fpath = "src/orchestrator.rs"
if os.path.exists(fpath):
    with open(fpath, "r") as f:
        code = f.read()
    
    # Fix the temporary Vec lifetime issue
    old_line = 'let line = String::from_utf8_lossy(&buf.drain(..=pos).collect::<Vec<_>>());'
    new_line = 'let line_bytes: Vec<u8> = buf.drain(..=pos).collect();\n                            let line = String::from_utf8_lossy(&line_bytes);'
    
    if old_line in code:
        code = code.replace(old_line, new_line)
        with open(fpath, "w") as f:
            f.write(code)
        print(">> [SUTURE] Fixed lifetime error in src/orchestrator.rs")

# 2. Fix src/self_healing_engine.rs (Struct Variant Error)
fpath = "src/self_healing_engine.rs"
if os.path.exists(fpath):
    with open(fpath, "r") as f:
        code = f.read()
    
    # Fix the Halt variant instantiation
    old_halt = "AgentAction::Halt"
    new_halt = 'AgentAction::Halt { reasoning: "Autonomic state transition".to_string() }'
    
    # Only replace if it's not already corrected
    if old_halt in code and "reasoning:" not in code[code.find(old_halt):code.find(old_halt)+50]:
        code = code.replace(old_halt, new_halt)
        with open(fpath, "w") as f:
            f.write(code)
        print(">> [SUTURE] Fixed Halt variant in src/self_healing_engine.rs")

