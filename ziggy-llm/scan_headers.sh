#!/bin/bash
# Project Nexus Singularity - GGUF Telemetry Scan

TARGET="src/gguf.zig"

echo "[*] Injecting Telemetry Pulse into Parser..."
# Inject a print statement right after the key is read
sed -i 's/const key = try readOwnedString(allocator, reader);/const key = try readOwnedString(allocator, reader);\n    std.debug.print("[DEBUG] Scanning Key: {s}\\n", .{key});/g' "$TARGET"

echo "[*] Forging Debug Muscles..."
# We build without ReleaseFast so we get full tracebacks
zig build

echo "[*] Running ZIGY Compile Scan..."
./zig-out/bin/ziggy-llm compile -m ../local_dir/pure_gemma4_2b.gguf -o ../local_dir/nexus_reflex_v1.zigy
