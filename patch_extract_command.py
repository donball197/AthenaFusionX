#!/usr/bin/env python3
"""Patch crates/runtime_core/src/completion_engine.rs: strip trailing prose clauses
(", and record ...") from raw-shell objectives. Idempotent; aborts if text not found.
Usage: python3 patch_extract_command.py [path]"""
import sys, shutil
p = sys.argv[1] if len(sys.argv) > 1 else "crates/runtime_core/src/completion_engine.rs"
src = open(p, encoding="utf-8").read()

old = """        if RAW_LEADERS.iter().any(|b| first == *b) {
            let line = take_balanced_lines(body);
            let line = line.trim();
            if !line.is_empty() {
                return Some(line.to_string());
            }
        }"""
new = """        if RAW_LEADERS.iter().any(|b| first == *b) {
            let line = take_balanced_lines(body);
            // Trailing prose ("cargo check --workspace, and record ...") is an
            // instruction about the report, not part of the command. The cut is
            // quote-aware, so `grep "a, and b" f` is untouched.
            const RAW_PROSE_STOPPERS: &[&str] = &[
                ", and ", ", then ", ", record ", ", report ", ", capture ",
                " and record ", " and report ", " and then ",
            ];
            let line = truncate_at_unquoted_stopper(&line, RAW_PROSE_STOPPERS);
            let line = line.trim();
            if !line.is_empty() {
                return Some(line.to_string());
            }
        }"""
if "RAW_PROSE_STOPPERS" in src:
    print("already patched"); sys.exit(0)
if src.count(old) != 1:
    sys.exit(f"ABORT: expected exactly 1 match, found {src.count(old)}")
shutil.copy(p, p + ".bak")
open(p, "w", encoding="utf-8").write(src.replace(old, new))
print("patched; backup at", p + ".bak")
