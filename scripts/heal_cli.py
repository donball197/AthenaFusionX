import os

with open("src/cli.rs", "r") as f:
    content = f.read()

# Find where the corrupted send_task call starts
idx = content.find("task_manager::send_task")
if idx != -1:
    let_idx = content.rfind("let ", 0, idx)
    
    clean_content = content[:let_idx] + """let result = task_manager::send_task(
            state.orchestrator.clone(),
            &state.client,
            &state.api_key,
            &dna,
            state.memory.clone(),
            state.vmo_state.clone(),
            state.tx.clone(),
            state.dist.clone(),
        ).await;

        println!("\\n\\x1b[32m>> [COMPLETE] {}\\x1b[0m\\n", result);
    }

    Ok(())
}
"""
    with open("src/cli.rs", "w") as f:
        f.write(clean_content)
    print(">> [SUTURE] cli.rs surgically repaired. Corrupted AIS logic purged.")
else:
    print(">> [FAULT] Could not locate task_manager::send_task in cli.rs")
