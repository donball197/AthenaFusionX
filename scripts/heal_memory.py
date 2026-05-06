import os

fpath = "src/task_manager.rs"
if os.path.exists(fpath):
    with open(fpath, "r") as f:
        code = f.read()

    # Inject the missing memory variable into the ContextPacket
    if "recent_logs: session_obs.clone()," not in code:
        code = code.replace(
            "model_override: Some(\"gemma-4-26b-a4b-it\".to_string()),",
            "model_override: Some(\"gemma-4-26b-a4b-it\".to_string()),\n            recent_logs: session_obs.clone(),"
        )
        with open(fpath, "w") as f:
            f.write(code)
        print("\x1b[32m>> [SUTURE] Short-term memory linked in task_manager.rs\x1b[0m")
    else:
        print("\x1b[33m>> [SKIP] Memory already linked.\x1b[0m")
