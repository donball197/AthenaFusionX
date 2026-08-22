import sys

path = "apps/nexus_ast_tool/src/main.rs"

old = "match nexus_ast_tool::client::ApiClient::generate_security_patch(&finding.pattern_name, &finding.source_snippet, &lang_str) {"
new = "match nexus_ast_tool::client::ApiClient::generate_security_patch(&finding.pattern_name, &finding.patch_snippet, &lang_str) {"

with open(path, "r") as f:
    content = f.read()

count = content.count(old)
if count != 1:
    print(f"ANCHOR MISMATCH: found {count} occurrences, expected 1", file=sys.stderr)
    sys.exit(1)

with open(path, "w") as f:
    f.write(content.replace(old, new))

print("OK: generate_security_patch now receives the narrow patch_snippet")
