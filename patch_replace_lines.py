import sys

file_path = "crates/runtime_core/src/json_parser.rs"
with open(file_path, "r") as f:
    content = f.read()

# Find the start of the buggy replace_lines normalizer
start_idx = content.find("            // Normalize replace_lines model-side field names")
if start_idx == -1:
    print("[FAIL] Could not find replace_lines block in json_parser.rs")
    sys.exit(1)

# Find the end of that block (the closing brace of the if statement)
end_idx = content.find("            }\n        };", start_idx)

if end_idx == -1:
    print("[FAIL] Could not find end of replace_lines block")
    sys.exit(1)

# Comment out the entire block
block_to_comment = content[start_idx:end_idx + 13]
commented_block = "\n".join(["// " + line for line in block_to_comment.split("\n")])

new_content = content[:start_idx] + commented_block + content[end_idx + 13:]

with open(file_path, "w") as f:
    f.write(new_content)

print("[OK] Successfully patched json_parser.rs!")
