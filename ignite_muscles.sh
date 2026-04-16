#!/bin/bash
echo "[*] Compiling Ziggy-LLM ReleaseFast..."
zig build -Doptimize=ReleaseFast

if [ -f "./zig-out/bin/ziggy-llm" ]; then
    echo "[+] Muscle seated at ./zig-out/bin/ziggy-llm"
    # Create a symlink in the root dev folder for the Brain to find
    ln -sf $(pwd)/zig-out/bin/ziggy-llm ../ziggy-reflex
else
    echo "[!] Build failed. Check src/main.zig for linker errors."
fi
