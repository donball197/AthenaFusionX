import os

fpath = "src/task_manager.rs"
if os.path.exists(fpath):
    with open(fpath, "r") as f:
        code = f.read()

    # SUTURE: Inject a raw response log before the JSON parser runs
    old_line = 'let response = match result {'
    new_line = '        let response = match result {'
    
    # We add a tracing info call to see the raw output
    log_injection = '        info!("--- RAW NEURAL PAYLOAD ---\\n{}\\n--- END PAYLOAD ---", resp);'
    
    if 'info!("--- RAW NEURAL PAYLOAD' not in code:
        # Find where resp is defined in the Ok match arm
        code = code.replace(
            'Ok(resp) => resp,',
            'Ok(resp) => {\n                info!("\\x1b[34m[THOUGHT_STREAM] >>\\x1b[0m\\n{} ", resp);\n                resp\n            },'
        )
        with open(fpath, "w") as f:
            f.write(code)
        print(">> [SUTURE] Cognitive transparency enabled in task_manager.rs")
