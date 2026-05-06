import os

fpath = "src/task_manager.rs"
if os.path.exists(fpath):
    with open(fpath, "r") as f:
        code = f.read()

    # 1. Remove the "Loop Killer"
    code = code.replace('if actions.len() <= 3 { break; }', '')

    # 2. Add explicit Halt detection
    if 'if halted { break; }' not in code:
        # Detect the end of the action loop
        injection = """                let mut halted = false;
                for (i, action) in actions.iter().enumerate() {
                    if let AgentAction::Halt { .. } = action { halted = true; }
                    info!("Executing action {}/{}: {:?}", i + 1, actions.len(), action);"""
        
        code = code.replace('for (i, action) in actions.iter().enumerate() {', injection)
        code = code.replace('}\n            }\n            Ok(_) => session_obs.push("EMPTY_ACTIONS".to_string()),', 
                            '}\n                if halted { break; }\n            }\n            Ok(_) => { info!("Agent returned empty actions. Mission complete."); break; }')

    with open(fpath, "w") as f:
        f.write(code)
    print("\x1b[32m>> [SUTURE] Autonomic Loop stabilized in task_manager.rs\x1b[0m")
