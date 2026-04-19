use std::fs;
use std::path::{Path, PathBuf};

pub struct VoidWalker;

impl VoidWalker {
    pub fn identify_orphans() -> Vec<String> {
        let mut orphans = Vec::new();
        let mut declared_mods = Vec::new();

        let root_files = ["src/main.rs", "src/lib.rs"];
        for root in &root_files {
            if let Ok(content) = fs::read_to_string(root) {
                Self::trace_module_tree(Path::new(root), &content, &mut declared_mods);
            }
        }

        let mut all_files = Vec::new();
        let _ = Self::visit_dirs(Path::new("src"), &mut all_files);

        // ⚡ SUTURE: Ignore common build directories to prevent endless snapshot spam
        let ignore_dirs = ["target", "ziggy-llm", "node_modules", ".lazarus_vault", "ui/dist"];

        for file_path in all_files {
            let path_str = file_path.to_string_lossy().to_string();
            
            if ignore_dirs.iter().any(|d| path_str.contains(d)) {
                continue;
            }
            if path_str.ends_with("mod.rs") || path_str.contains("/bin/") {
                continue;
            }
            if !declared_mods.iter().any(|m| path_str.ends_with(m)) {
                orphans.push(path_str);
            }
        }
        orphans
    }

    fn trace_module_tree(current_file: &Path, content: &str, whitelist: &mut Vec<String>) {
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("mod ") || trimmed.starts_with("pub mod ") {
                if let Some(mod_name) = trimmed.split_whitespace().nth(if trimmed.starts_with("pub") { 2 } else { 1 }) {
                    let name = mod_name.trim_end_matches(';');
                    let parent = current_file.parent().unwrap_or(Path::new(""));
                    let possible_paths = vec![
                        parent.join(format!("{}.rs", name)),
                        parent.join(name).join("mod.rs"),
                        parent.join(current_file.file_stem().unwrap_or_default()).join(format!("{}.rs", name)),
                    ];

                    for path in possible_paths {
                        if path.exists() {
                            let path_key = path.to_string_lossy().to_string();
                            if !whitelist.contains(&path_key) {
                                whitelist.push(path_key);
                                if let Ok(sub_content) = fs::read_to_string(&path) {
                                    Self::trace_module_tree(&path, &sub_content, whitelist);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    fn visit_dirs(dir: &Path, files: &mut Vec<PathBuf>) -> std::io::Result<()> {
        if dir.is_dir() {
            for entry in fs::read_dir(dir)? {
                let entry = entry?;
                let path = entry.path();
                if path.is_dir() {
                    Self::visit_dirs(&path, files)?;
                } else if path.extension().and_then(|s| s.to_str()) == Some("rs") {
                    files.push(path);
                }
            }
        }
        Ok(())
    }
}
