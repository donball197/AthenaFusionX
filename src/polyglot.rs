use std::path::Path;
use std::process::Command;
use serde::Serialize;
use walkdir::WalkDir;

#[derive(Serialize)]
pub struct BuildResult {
    pub success: bool,
    pub log: String,
    pub detected_stack: String,
}

/// detect_and_build: Orchestrates hardware-specific builds based on detected manifest files.
pub fn detect_and_build(path: &str) -> BuildResult {
    let p = Path::new(path);

    let (cmd, args, stack) = if p.join("Cargo.toml").exists() {
        ("cargo", vec!["build", "--release"], "RUST")
    } else if p.join("package.json").exists() {
        ("npm", vec!["run", "build"], "NODEJS")
    } else if p.join("requirements.txt").exists() {
        ("pip", vec!["install", "-r", "requirements.txt"], "PYTHON")
    } else if p.join("Makefile").exists() {
        ("make", vec![], "MAKEFILE")
    } else {
        return BuildResult {
            success: false,
            log: ">> [POLYGLOT] UNKNOWN STACK: No manifest detected.".to_string(),
            detected_stack: "UNKNOWN".into(),
        };
    };

    println!(">> [POLYGLOT] Building {} project at {}...", stack, path);
    let output = Command::new(cmd).args(&args).current_dir(path).output();

    match output {
        Ok(out) => {
            // FIXED: Using 'out' (the unwrapped Output) instead of 'output' (the Result)
            let log = format!(
                "STDOUT:\n{}\nSTDERR:\n{}",
                String::from_utf8_lossy(&out.stdout),
                String::from_utf8_lossy(&out.stderr)
            );
            BuildResult {
                success: out.status.success(),
                log,
                detected_stack: stack.into(),
            }
        }
        Err(e) => BuildResult {
            success: false,
            log: format!(">> [POLYGLOT] Execution Fault: {}", e),
            detected_stack: stack.into(),
        },
    }
}

/// generate_project_tree: Constructs a high-fidelity visual map of the workspace topology.
pub fn generate_project_tree() -> String {
    let mut tree = String::from("PROJECT STRUCTURE MAP [ZENITH]:\n");
    
    let walker = WalkDir::new(".")
        .max_depth(5)
        .into_iter()
        .filter_entry(|e| {
            let name = e.file_name().to_string_lossy();
            !name.starts_with('.') && 
            name != "target" && 
            name != "node_modules" && 
            name != "dist" &&
            name != ".lazarus_vault"
        })
        .filter_map(|e| e.ok());

    for entry in walker {
        let depth = entry.depth();
        if depth == 0 { continue; }

        let indent = "  ".repeat(depth - 1);
        let prefix = if entry.file_type().is_dir() { "📁 " } else { "📄 " };
        let name = entry.file_name().to_string_lossy();
        
        tree.push_str(&format!("{}{}{}\n", indent, prefix, name));
    }
    tree
}
