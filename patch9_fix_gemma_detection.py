import sys

path = "apps/nexus_ast_tool/src/client.rs"

old = 'let is_gemini = model.to_lowercase().contains("gemini");'
new = '''let model_lower = model.to_lowercase();
        // "gemma" and "gemini" are different strings but both live under the
        // Google generativelanguage API. NVIDIA NIM model names in this repo's
        // .env are all "provider/model" (nvidia/..., z-ai/..., meta/...), so
        // presence of '/' is also a strong signal it's NOT a Google model.
        let is_gemini = model_lower.contains("gemini") || model_lower.contains("gemma");'''

with open(path, "r") as f:
    content = f.read()

count = content.count(old)
if count != 3:
    print(f"ANCHOR MISMATCH: found {count} occurrences, expected 3", file=sys.stderr)
    sys.exit(1)

with open(path, "w") as f:
    f.write(content.replace(old, new))

print(f"OK: is_gemini detection fixed at all {count} call sites")
