#!/bin/bash
# Project Nexus Singularity - Raw Memory Stethoscope

echo "[*] Purging corrupted compiler cache..."
rm -rf .zig-cache zig-out

echo "[*] Attaching Stethoscope to parser loop..."
cat << 'PY_EOF' > stethoscope.py
import os

path = "src/gguf.zig"
with open(path, "r") as f:
    code = f.read()

# Locate the exact moment the ID card reads the key and type
target = "const raw_value_type = readInt(reader, u32) catch return error.TruncatedFile;"
injection = "const raw_value_type = readInt(reader, u32) catch return error.TruncatedFile;\n    std.debug.print(\"[STETHOSCOPE] Key: {s} | TypeID: {d}\\n\", .{key, raw_value_type});"

# Inject only into parseMetadataEntry
if "[STETHOSCOPE]" not in code:
    code = code.replace(target, injection, 1)
    with open(path, "w") as f:
        f.write(code)
    print("[+] Stethoscope successfully wired to the substrate.")
else:
    print("[*] Stethoscope already attached.")
PY_EOF

python3 stethoscope.py

echo "[*] Forging clean Debug Muscles..."
zig build -Doptimize=Debug

echo "[*] Triggering raw diagnostic pulse..."
./zig-out/bin/ziggy-llm compile -m ../local_dir/pure_gemma4_2b.gguf -o ../local_dir/nexus_reflex_v1.zigy
