#!/bin/bash
# Project Nexus Singularity - Shadow Parser Detonation

TARGET="src/llama_cpu.zig"

echo "[*] Wiring Panic Traps into the Shadow Parser ($TARGET)..."

# Convert silent returns into loud panics
sed -i 's/return error.InvalidMetadataType;/@panic("FRACTURE_LLAMA_RETURN");/g' "$TARGET"
sed -i 's/else => error.InvalidMetadataType,/@panic("FRACTURE_LLAMA_ELSE"),/g' "$TARGET"

echo "[*] Purging cache to force true compilation..."
rm -rf .zig-cache zig-out

echo "[*] Forging clean Debug Muscles..."
zig build -Doptimize=Debug

echo "[*] Triggering detonation..."
./zig-out/bin/ziggy-llm compile -m ../local_dir/pure_gemma4_2b.gguf -o ../local_dir/nexus_reflex_v1.zigy
