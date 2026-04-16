import os

path = "src/llama_cpu.zig"
with open(path, "r") as f:
    lines = f.readlines()

cleaned_lines = []
for line in lines:
    # Strip the dangling strings that sed left behind
    stripped = line.strip()
    if stripped == "[!!!] GGUF PARSER FAILURE":
        continue
    if "[!] Unknown Metadata Type ID" in stripped and "std.debug.print" not in line:
        continue
    if stripped == "\", .{raw_value_type}); return err; };":
        continue
    cleaned_lines.append(line)

with open(path, "w") as f:
    f.writelines(cleaned_lines)

print("[+] Floating syntax errors excised from llama_cpu.zig")
