#!/bin/bash
# AthenaFusionX Bug Patch Script (4 Bugs)

python3 << 'EOF'
import os

# ---------------------------------------------------------
# Bug 1: Fix search_code_semantic read-only mode (executor.rs)
# ---------------------------------------------------------
file_path = "crates/runtime_core/src/executor.rs"
if os.path.exists(file_path):
    with open(file_path, "r") as f:
        content = f.read()
    
    old_str = 'let db_path = "nexus_daemon.db";\n                match sqlx::sqlite::SqlitePoolOptions::new().max_connections(1).connect(&format!("sqlite://{}?mode=ro", db_path)).await {'
    new_str = 'let db_path = "nexus_daemon.db";\n                match sqlx::sqlite::SqlitePoolOptions::new().max_connections(1).connect(&format!("sqlite://{}", db_path)).await {'
    
    if old_str in content:
        content = content.replace(old_str, new_str)
        with open(file_path, "w") as f:
            f.write(content)
        print("[✅ FIXED] Removed read-only mode from search_code_semantic DB connection")
    else:
        print("[⏭️] Bug 1 already patched or pattern not found")

# ---------------------------------------------------------
# Bug 2: Fix OMNI-GUARD premature completion (completion_engine.rs)
# ---------------------------------------------------------
file_path = "crates/runtime_core/src/completion_engine.rs"
if os.path.exists(file_path):
    with open(file_path, "r") as f:
        content = f.read()
    
    old_str = '''        let has_patch = any_word_unnegated(classify_text, &["patch", "apply", "fix", "remediate", "mutate"]);
        let has_write = any_word_unnegated(classify_text, &["write", "export", "save", "implement", "integrate", "add", "create"]);'''
    
    new_str = '''        let has_patch = any_word_unnegated(classify_text, &["patch", "apply", "fix", "remediate", "mutate"]);
        let has_write = any_word_unnegated(classify_text, &["write", "export", "save", "implement", "integrate", "add", "create"]);
        let has_read = any_word_unnegated(classify_text, &["read", "examine", "inspect", "analyze"]);'''
    
    if old_str in content:
        content = content.replace(old_str, new_str)
        with open(file_path, "w") as f:
            f.write(content)
        print("[✅ FIXED] Added has_read detection to completion_engine.rs")
    else:
        print("[⏭️] Bug 2 pattern not found")

# ---------------------------------------------------------
# Bug 3: Add Tool Compliance rule to Strategist prompt
# ---------------------------------------------------------
file_path = "crates/runtime_core/src/epistemic_engine.rs"
if os.path.exists(file_path):
    with open(file_path, "r") as f:
        content = f.read()
    
    old_str = '"CRITICAL RULE 6: [PATCH RESILIENCE] If a `diff_patch` fails because the search string was not found, DO NOT attempt the same `diff_patch` again. You MUST fall back to `replace_lines` (if you know the line numbers) or `write_file` (to rewrite the entire function/file safely) to apply the fix.",'
    new_str = '"CRITICAL RULE 6: [PATCH RESILIENCE] If a `diff_patch` fails because the search string was not found, DO NOT attempt the same `diff_patch` again. You MUST fall back to `replace_lines` (if you know the line numbers) or `write_file` (to rewrite the entire function/file safely) to apply the fix.",\n            "CRITICAL RULE 7: [TOOL COMPLIANCE] If the directive explicitly names a tool (e.g., `synthesize_report`), you MUST use that exact tool. Do not substitute `write_file` for `synthesize_report`.",\n            "CRITICAL RULE 8: [INDEXING] If you are targeting a directory that has not been indexed, you MUST call `index_codebase` before using `search_code_semantic`.",'
    
    if old_str in content:
        content = content.replace(old_str, new_str)
        with open(file_path, "w") as f:
            f.write(content)
        print("[✅ FIXED] Added Tool Compliance and Indexing rules to Strategist prompt")
    else:
        print("[⏭️] Bug 3 pattern not found")

# ---------------------------------------------------------
# Bug 4: Fix Critic Context Bleed (epistemic_engine.rs)
# ---------------------------------------------------------
file_path = "crates/runtime_core/src/epistemic_engine.rs"
if os.path.exists(file_path):
    with open(file_path, "r") as f:
        content = f.read()
    
    old_str = 'CRITICAL RULE 3: THE EVIDENCE VERIFICATION TAXONOMY. If the proposed action contains a quantitative claim, architectural assumption, or performance metric , you MUST check the EVIDENCE HISTORY. If there is no explicit [Measured] execution log proving the claim, you MUST VETO the proposal (provide a negative confidence_modifier) and cite: \\"VETO: Unverified quantitative claim. Downgrade to [Hypothesis] or provide [Measured] evidence.\\"'
    new_str = 'CRITICAL RULE 3: THE EVIDENCE VERIFICATION TAXONOMY. If the *Strategist Proposal* contains a quantitative claim, architectural assumption, or performance metric, you MUST check the EVIDENCE HISTORY. If there is no explicit [Measured] execution log proving the claim, you MUST VETO the proposal (provide a negative confidence_modifier) and cite: \\"VETO: Unverified quantitative claim. Downgrade to [Hypothesis] or provide [Measured] evidence.\\" NOTE: Do NOT apply this rule to code snippets or variable names found *inside* the Evidence History; it applies only to claims made *by the Strategist* in the proposal.'
    
    if old_str in content:
        content = content.replace(old_str, new_str)
        with open(file_path, "w") as f:
            f.write(content)
        print("[✅ FIXED] Clarified Critic Rule 3 to prevent hallucinated vetoes")
    else:
        print("[⏭️] Bug 4 pattern not found")

EOF

echo ">> Rebuilding Workspace..."
cargo check -j 4
