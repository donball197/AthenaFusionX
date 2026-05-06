import os

fpath = "src/task_manager.rs"
if os.path.exists(fpath):
    with open(fpath, "r") as f:
        code = f.read()
    
    # Inject the missing Ok(Some(_)) pattern right before the Err(_) pattern
    if "Ok(Some(" not in code:
        code = code.replace(
            "Err(feedback) => {",
            """Ok(Some(modified_directive)) => {
                            session_obs.push(serde_json::json!({"status": "modified_by_user", "directive": modified_directive}).to_string());
                        }
                        Err(feedback) => {"""
        )
        
        with open(fpath, "w") as f:
            f.write(code)
        print(">> [SUTURE] E0004 Fracture Sealed: Ok(Some(_)) arm added to task_manager.rs")
