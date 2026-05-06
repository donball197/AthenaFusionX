import os

fpath = "src/executor.rs"
with open(fpath, "r") as f:
    code = f.read()

# Inject the check for Complete/Fault state after each action execution
old_block = """                Ok(obs) => {
                    observations.push(obs.clone());
                    info!("Action observation: {}" , obs);
                }"""

new_block = """                Ok(obs) => {
                    observations.push(obs.clone());
                    info!("Action observation: {}", obs);
                    // Check if an action (like Halt) terminated the cycle
                    if self.current_state == FsmState::Complete || self.current_state == FsmState::Fault {
                        break;
                    }
                }"""

if "if self.current_state == FsmState::Complete" not in code:
    code = code.replace(old_block, new_block)
    with open(fpath, "w") as f:
        f.write(code)
    print(">> [SUTURE] Executor now honors early termination (Halt).")
