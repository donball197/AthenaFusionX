import os

print(">> Applying Phase 3 (Item #9) Remediation Fix...")

path = "crates/runtime_core/src/world_model.rs"
if os.path.exists(path):
    with open(path, "r") as f:
        content = f.read()

    # The exact block we want to replace
    old_func = """    pub async fn semantic_query(&self, _mission: &str) -> Vec<String> {
        let mut chunks = Vec::new();

        // 1. Inject Repository File Tree Context
        if let Ok(out) = Command::new("find")
            .args([
                ".",
                "-maxdepth",
                "4",
                "-type",
                "f",
                "-not",
                "-path",
                "*/.git/*",
                "-not",
                "-path",
                "*/target/*",
                "-not",
                "-path",
                "*/node_modules/*",
                "-not",
                "-path",
                "*/.cold_storage/*",
            ])
            .output()
        {
            if out.status.success() {
                let files = String::from_utf8_lossy(&out.stdout);
                chunks.push("[REPOSITORY FILE TREE (Depth 4)]".to_string());
                chunks.push(files.chars().take(6000).collect());
            }
        }"""

    # The new memory-safe implementation using walkdir
    new_func = """    pub async fn semantic_query(&self, _mission: &str) -> Vec<String> {
        let mut chunks = Vec::new();

        // 1. Inject Repository File Tree Context (Safe Rust Iteration)
        let mut files = String::new();
        for entry in walkdir::WalkDir::new(".")
            .max_depth(4)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            if entry.file_type().is_file() {
                let path_str = entry.path().to_string_lossy();
                if !path_str.contains("/.git/")
                    && !path_str.contains("/target/")
                    && !path_str.contains("/node_modules/")
                    && !path_str.contains("/.cold_storage/")
                {
                    files.push_str(&format!("{}\\n", path_str));
                }
            }
        }

        if !files.is_empty() {
            chunks.push("[REPOSITORY FILE TREE (Depth 4)]".to_string());
            chunks.push(files.chars().take(6000).collect());
        }"""

    if old_func in content:
        content = content.replace(old_func, new_func)
        with open(path, "w") as f:
            f.write(content)
        print(f"[OK] Patched {path} - Replaced dangerous 'find' shell call with walkdir.")
    else:
        print(f"[!] Could not find target signature in {path}. It may already be patched.")
else:
    print(f"[ERROR] File not found: {path}")

