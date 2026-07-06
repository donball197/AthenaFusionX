#!/usr/bin/env python3
import os

PODMAN_PATH = "./crates/runtime_core/src/podman.rs"
ROUTER_PATH = "./crates/runtime_core/src/router.rs"
EXECUTOR_PATH = "./crates/runtime_core/src/executor.rs"

def update_file(path, old_text, new_text):
    if not os.path.exists(path):
        print(f"[!] Could not find {path}")
        return
    with open(path, "r") as f:
        content = f.read()
    if old_text in content:
        with open(path, "w") as f:
            f.write(content.replace(old_text, new_text))
        print(f"[✓] Updated {path}")
    else:
        print(f"[?] Text block not found in {path} (already updated?)")

def apply_fixes():
    print(">> Expanding Sandbox Allowlist and UI Scraper Schema...")

    old_podman = """        let host_allowed = [
            "cargo", "ls", "cat", "python3", "python", "grep", "find",
            "git", "pwd", "echo",
        ];"""
    new_podman = """        let host_allowed = [
            "cargo", "ls", "cat", "python3", "python", "grep", "find",
            "git", "pwd", "echo", "curl", "wget"
        ];"""
    update_file(PODMAN_PATH, old_podman, new_podman)

    old_router = """MANDATORY SCHEMA:
[
  { "type": "patch_function", "path": "...", "function_name": "...", "new_body": "...", "reasoning": "..." },
  { "type": "diff_patch", "path": "...", "search": "...", "replace": "...", "reasoning": "..." },
  { "type": "write_file", "path": "...", "content": "...", "reasoning": "..." },
  { "type": "read_file", "path": "...", "reasoning": "..." },
  { "type": "execute_command", "command": "...", "reasoning": "..." },
  { "type": "search_memory", "query": "...", "reasoning": "..." },
  { "type": "halt", "reasoning": "..." }
]"""
    new_router = """MANDATORY SCHEMA:
[
  { "type": "patch_function", "path": "...", "function_name": "...", "new_body": "...", "reasoning": "..." },
  { "type": "diff_patch", "path": "...", "search": "...", "replace": "...", "reasoning": "..." },
  { "type": "write_file", "path": "...", "content": "...", "reasoning": "..." },
  { "type": "read_file", "path": "...", "reasoning": "..." },
  { "type": "execute_command", "command": "...", "reasoning": "..." },
  { "type": "search_memory", "query": "...", "reasoning": "..." },
  { "type": "scrape_url", "url": "...", "reasoning": "..." },
  { "type": "halt", "reasoning": "..." }
]"""
    update_file(ROUTER_PATH, old_router, new_router)

    old_executor = """            AgentAction::PatchFunction { path, function_name, new_body, .. } => {
                let patched = crate::ast_editor::AstEditor::update_function(path, function_name, new_body)?;
                tokio::fs::write(path, patched).await?;
                Ok(format!("[PATCH_FUNCTION_OK] Updated function '{}' in {}", function_name, path))
            }
            AgentAction::Halt { reasoning } => Ok(format!("[HALT] {}", reasoning)),"""
            
    new_executor = """            AgentAction::PatchFunction { path, function_name, new_body, .. } => {
                let patched = crate::ast_editor::AstEditor::update_function(path, function_name, new_body)?;
                tokio::fs::write(path, patched).await?;
                Ok(format!("[PATCH_FUNCTION_OK] Updated function '{}' in {}", function_name, path))
            }
            AgentAction::ScrapeUrl { url, .. } => {
                let client = reqwest::Client::builder().timeout(std::time::Duration::from_secs(15)).build()?;
                let res = client.get(url).send().await?.text().await?;
                let content: String = res.chars().take(4000).collect(); // Prevent context explosion
                Ok(format!("[URL_SCRAPED] Output (Truncated 4K chars):\\n{}", content))
            }
            AgentAction::Halt { reasoning } => Ok(format!("[HALT] {}", reasoning)),"""
    update_file(EXECUTOR_PATH, old_executor, new_executor)

if __name__ == "__main__":
    apply_fixes()
