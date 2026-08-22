import sys

path = "apps/nexus_ast_tool/src/main.rs"

old = "findings.sort_by(|a, b| b.start_line.cmp(&a.start_line));"
new = "findings.sort_by_key(|b| std::cmp::Reverse(b.start_line));"

with open(path, "r") as f:
    content = f.read()

count = content.count(old)
if count != 1:
    print(f"ANCHOR MISMATCH: found {count} occurrences, expected 1", file=sys.stderr)
    sys.exit(1)

with open(path, "w") as f:
    f.write(content.replace(old, new))

print("OK: unnecessary_sort_by lint fixed")
