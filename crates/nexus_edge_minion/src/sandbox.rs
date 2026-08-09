use std::process::Command;

pub struct LocalSandbox;

impl LocalSandbox {
    pub fn execute(command: &str) -> String {
        println!("    [Sandbox] Intercepted command. Executing on Host: `{}`", command);
        
        match Command::new("sh").arg("-c").arg(command).output() {
            Ok(output) => {
                let mut res = String::new();
                let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
                let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();

                if !stdout.is_empty() { res.push_str(&stdout); }
                if !stderr.is_empty() {
                    if !res.is_empty() { res.push_str("\n"); }
                    res.push_str(&format!("[STDERR] {}", stderr));
                }
                
                if res.is_empty() {
                    "[Command executed successfully with no output]".to_string()
                } else {
                    res
                }
            }
            Err(e) => format!("[SANDBOX FATAL] Failed to spawn shell: {}", e),
        }
    }
}
