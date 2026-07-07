import sys

FILES = {}

FILES["crates/runtime_core/src/executor.rs"] = {
    "anchor": '''pub fn sanitize(path: &str) -> Result<PathBuf> {
    // \u26a1 Allow /tmp/ prefix, block all other absolute paths
    if (path.starts_with('/') && !path.starts_with("/tmp/")) || path.contains("..") {
        return Err(anyhow!("PATH_TRAVERSAL_DENIED: {}", path));
    }''',
    "replacement": '''pub fn sanitize(path: &str) -> Result<PathBuf> {
    // No legitimate path in this repo contains a literal colon. Guards
    // against the LLM mistaking a cargo/clippy diagnostic locator
    // (e.g. "conductor.rs:38:29") for a real file path and writing to it.
    if path.contains(':') {
        return Err(anyhow!("PATH_LOOKS_LIKE_DIAGNOSTIC_LOCATOR: {}", path));
    }
    // Allow /tmp/ prefix, block all other absolute paths
    if (path.starts_with('/') && !path.starts_with("/tmp/")) || path.contains("..") {
        return Err(anyhow!("PATH_TRAVERSAL_DENIED: {}", path));
    }''',
}

FILES["crates/runtime_core/src/epistemic_engine.rs"] = {
    "anchor": '''             2. [{{ \"type\": \"read_file\", \"path\": \"<filepath>\", \"reasoning\": \"...\" }}]\\n\\
             3. [{{ \"type\": \"write_file\", \"path\": \"<filepath>\", \"content\": \"<data>\", \"reasoning\": \"...\" }}]\\n\\
             4. [{{ \"type\": \"diff_patch\", \"path\": \"<filepath>\", \"search\": \"<search>\", \"replace\": \"<replace>\", \"reasoning\": \"...\" }}]\\n\\
             5. [{{ \"type\": \"patch_function\", \"path\": \"<filepath>\", \"function_name\": \"<name>\", \"new_body\": \"<full source>\", \"reasoning\": \"...\" }}]\\n\\
             6. [{{ \"type\": \"halt\", \"reasoning\": \"...\" }}]\\n\\''',
    "replacement": '''             2. [{{ \"type\": \"read_file\", \"path\": \"<filepath>\", \"reasoning\": \"...\" }}]\\n\\
             3. [{{ \"type\": \"read_function\", \"path\": \"<filepath>\", \"function_name\": \"<name>\", \"reasoning\": \"...\" }}]\\n\\
             4. [{{ \"type\": \"write_file\", \"path\": \"<filepath>\", \"content\": \"<data>\", \"reasoning\": \"...\" }}]\\n\\
             5. [{{ \"type\": \"diff_patch\", \"path\": \"<filepath>\", \"search\": \"<search>\", \"replace\": \"<replace>\", \"reasoning\": \"...\" }}]\\n\\
             6. [{{ \"type\": \"patch_function\", \"path\": \"<filepath>\", \"function_name\": \"<name>\", \"new_body\": \"<full source>\", \"reasoning\": \"...\" }}]\\n\\
             7. [{{ \"type\": \"git_rollback\", \"reasoning\": \"...\" }}]\\n\\
             8. [{{ \"type\": \"halt\", \"reasoning\": \"...\" }}]\\n\\
             NOTE: git_rollback discards ALL uncommitted changes via 'git reset --hard HEAD'. Only use it if the strategy explicitly calls for reverting a bad edit.\\n\\''',
}
FILES["crates/runtime_core/src/epistemic_engine.rs"]["anchor_2"] = '''             NOTE: Do NOT use types like 'find_file', 'ls_tree', or 'grep_code'. If you need to search or list, use 'execute_command' (e.g. command: 'ls -F' or command: 'find .').\\n\\'''
FILES["crates/runtime_core/src/epistemic_engine.rs"]["replacement_2"] = '''             NOTE: Do NOT use types like 'find_file', 'ls_tree', or 'grep_code'. If you need to search or list, use 'execute_command' (e.g. command: 'ls -F' or command: 'find .').\\n\\
             NOTE: A path like 'file.rs:38:29' from a compiler/clippy diagnostic is NOT a real file path -- it is 'file.rs' at line 38, column 29. Strip the ':line:col' suffix before using it as a 'path' value.\\n\\'''


def check_anchor(path, key, anchor):
    try:
        with open(path, "r", encoding="utf-8") as f:
            content = f.read()
    except FileNotFoundError:
        print("FATAL: " + path + " not found.", file=sys.stderr)
        sys.exit(1)
    count = content.count(anchor)
    if count == 0:
        print("FATAL: anchor '" + key + "' not found in " + path + ".", file=sys.stderr)
        sys.exit(1)
    if count > 1:
        print("FATAL: anchor '" + key + "' matched " + str(count) + " times in " + path + ", expected 1.", file=sys.stderr)
        sys.exit(1)
    return content


def main():
    loaded = {}
    for path, spec in FILES.items():
        content = check_anchor(path, "primary", spec["anchor"])
        if "anchor_2" in spec:
            content = check_anchor(path, "anchor_2", spec["anchor_2"])
        loaded[path] = content

    for path, spec in FILES.items():
        content = loaded[path]
        content = content.replace(spec["anchor"], spec["replacement"])
        if "anchor_2" in spec:
            content = content.replace(spec["anchor_2"], spec["replacement_2"])
        with open(path, "w", encoding="utf-8") as f:
            f.write(content)
        print("OK: patched " + path)

    print()
    print("Next: cargo check -p nexus_runtime_core -j 2")


if __name__ == "__main__":
    main()
