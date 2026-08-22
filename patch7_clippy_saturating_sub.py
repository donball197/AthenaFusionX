import sys

path = "apps/nexus_ast_tool/src/polyglot.rs"

old = "let context_start = if start_line > 7 { start_line - 7 } else { 0 };"
new = "let context_start = start_line.saturating_sub(7);"

with open(path, "r") as f:
    content = f.read()

count = content.count(old)
if count != 1:
    print(f"ANCHOR MISMATCH: found {count} occurrences, expected 1", file=sys.stderr)
    sys.exit(1)

with open(path, "w") as f:
    f.write(content.replace(old, new))

print("OK: implicit_saturating_sub lint fixed")
