use std::path::Path;
use std::process::Command;
use serde::Serialize;

#[derive(Serialize)]
pub struct BuildResult {
    pub success: bool,
    pub log: String,
    pub detected_stack: String,
}

pub fn detect_and_build(path: &str) -> BuildResult {
    let p = Path::new(path);
    
    let (cmd, args, stack) = if p.join("Cargo.toml").exists() {
        ("cargo", vec!["build", "--release"], "RUST")
    } else if p.join("package.json").exists() {
        ("npm", vec!["run", "build"], "NODEJS")
    } else if p.join("requirements.txt").exists() {
        ("pip", vec!["install", "-r", "requirements.txt"], "PYTHON")
    } else if p.join("go.mod").exists() {
        ("go", vec!["build", "."], "GOLANG")
    } else if p.join("Makefile").exists() {
        ("make", vec![], "MAKEFILE")
    } else {
        return BuildResult { 
            success: false, 
            log: ">> NO BUILD SYSTEM DETECTED (Cargo.toml, package.json, etc missing)".to_string(), 
            detected_stack: "UNKNOWN".into() 
        };
    };

    println!(">> [POLYGLOT] Building {} project in {}", stack, path);

    let output = Command::new(cmd).args(&args).current_dir(path).output();

    match output {
        Ok(out) => {
            let combined_log = format!("STDOUT:\n{}\nSTDERR:\n{}", 
                String::from_utf8_lossy(&out.stdout), 
                String::from_utf8_lossy(&out.stderr)
            );
            BuildResult { success: out.status.success(), log: combined_log, detected_stack: stack.into() }
        },
        Err(e) => BuildResult { success: false, log: format!("Execution Error: {}", e), detected_stack: stack.into() }
    }
}
