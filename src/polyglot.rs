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
            log: ">> UNKNOWN STACK".to_string(),
            detected_stack: "UNKNOWN".into(),
        };
    };

    println!(">> [POLYGLOT] Building {} project at {}...", stack, path);
    let output = Command::new(cmd).args(&args).current_dir(path).output();

    match output {
        Ok(out) => {
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
            log: e.to_string(),
            detected_stack: stack.into(),
        },
    }
}

pub fn generate_project_tree() -> String {
    let mut tree = String::from("PROJECT STRUCTURE:\n");
    for entry in WalkDir::new(".")
        .max_depth(2)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        let display = path.display().to_string();

        if display.contains("/target")
            || display.contains("/.git")
            || display.contains("/node_modules")
            || display.contains("/dist")
        {
            continue;
        }

        let depth = display.matches('/').count();
        let indent = "  ".repeat(depth);
        tree.push_str(&format!(
            "{}{}\n",
            indent,
            path.file_name().unwrap_or_default().to_string_lossy()
        ));
    }
    tree
}
