import os
fpath = "src/executor.rs"
with open(fpath, "r") as f:
    code = f.read()

# Inject a hard block right before executing commands
target = 'std::process::Command::new("sh")'
if target in code and "rm -rf" not in code:
    code = code.replace(
        target,
        '''if command.contains("rm -rf") || command.contains("mkfs") {
                raw_results.push(format!("AXIOM_BLOCK: Destructive shell commands are strictly forbidden."));
                crate::metrics::METRICS.inc_arbiter_block();
                continue;
            }
            std::process::Command::new("sh")'''
    )
    with open(fpath, "w") as f:
        f.write(code)
    print(">> [SHIELD] Executor hardened against destructive shell commands.")
