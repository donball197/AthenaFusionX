import sys

path = "apps/nexus_ast_tool/src/main.rs"

old = "if let Ok(new_source) = nexus_ast_tool::mutation::PolyglotSplicer::splice(&file_source, finding.start_line, finding.end_line, &patch) {"
new = "if let Ok(new_source) = nexus_ast_tool::mutation::PolyglotSplicer::splice(&file_source, finding.patch_start_line, finding.end_line, &patch) {"

with open(path, "r") as f:
    content = f.read()

count = content.count(old)
if count != 1:
    print(f"ANCHOR MISMATCH: found {count} occurrences, expected 1", file=sys.stderr)
    sys.exit(1)

with open(path, "w") as f:
    f.write(content.replace(old, new))

print("OK: splice now uses patch_start_line so its range matches what the LLM saw")
