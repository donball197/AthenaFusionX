use std::process::Command;
use std::fs;

pub fn validate_patch(temp_path: &str, code: &str, extension: &str) -> Result<(), String> {
    if fs::write(temp_path, code).is_err() { 
        return Err("FileSystem Error".to_string()); 
    }

    match extension {
        "py" => {
            // 1. Scan for Python imports
            let imports: Vec<&str> = vec!["requests", "bs4", "pandas", "numpy", "ccxt"];
            for lib in imports {
                if code.contains(lib) {
                    println!(">> [LIBRARIAN] Missing dependency detected: {}. Installing...", lib);
                    let _ = Command::new("pip").args(&["install", lib]).status();
                }
            }
            // 2. Validate Syntax
            let out = Command::new("python3").args(&["-m", "py_compile", temp_path]).output().map_err(|e| e.to_string())?;
            if !out.status.success() {
                return Err(String::from_utf8_lossy(&out.stderr).to_string());
            }
        },
        "rs" => {
            // Rust is trickier; it usually needs entries in Cargo.toml.
            // For now, we ensure the syntax is sound.
            let out = Command::new("rustc").args(&["--crate-type", "lib", "--emit=mir", "-o", "/dev/null", temp_path]).output().map_err(|e| e.to_string())?;
            if !out.status.success() {
                return Err(String::from_utf8_lossy(&out.stderr).to_string());
            }
        },
        _ => {}
    }

    Ok(())
}
